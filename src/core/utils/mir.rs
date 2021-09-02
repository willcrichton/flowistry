#![allow(dead_code)]

use anyhow::{bail, Result};
use log::warn;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_graphviz as dot;
use rustc_hir::BodyId;
use rustc_middle::{
  mir::{
    visit::{NonUseContext, PlaceContext, Visitor},
    *,
  },
  ty::{
    self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS, TypeFoldable, TypeVisitor,
    WithOptConstParam,
  },
};
use rustc_mir::{
  consumers::BodyWithBorrowckFacts,
  dataflow::{fmt::DebugWithContext, graphviz, Analysis, Results},
  transform::{simplify, MirPass},
  util::write_mir_fn,
};
use rustc_span::Span;
use rustc_target::abi::VariantIdx;
use std::{
  collections::hash_map::Entry, fs::File, hash::Hash, io::Write, ops::ControlFlow, process::Command,
};

pub fn operand_to_place(operand: &Operand<'tcx>) -> Option<Place<'tcx>> {
  match operand {
    Operand::Copy(place) | Operand::Move(place) => Some(*place),
    Operand::Constant(_) => None,
  }
}

pub fn local_to_place(local: Local, tcx: TyCtxt<'tcx>) -> Place<'tcx> {
  Place {
    local,
    projection: tcx.intern_place_elems(&[]),
  }
}

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  local: Local,
  place_stack: Vec<PlaceElem<'tcx>>,
  ty_stack: Vec<Ty<'tcx>>,
  places: Option<HashSet<Place<'tcx>>>,
  regions: HashMap<RegionVid, (Place<'tcx>, Mutability)>,
}

impl TypeVisitor<'tcx> for CollectRegions<'tcx> {
  fn tcx_for_anon_const_substs(&self) -> Option<TyCtxt<'tcx>> {
    Some(self.tcx)
  }

  fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
    if self
      .ty_stack
      .iter()
      .any(|visited_ty| TyS::same_type(ty, visited_ty))
    {
      return ControlFlow::Continue(());
    }

    self.ty_stack.push(ty);

    match ty.kind() {
      _ if ty.is_box() => {
        self.place_stack.push(ProjectionElem::Deref);
        self.visit_ty(ty.boxed_ty());
        self.place_stack.pop();
      }

      TyKind::Tuple(fields) => {
        for (i, field) in fields.iter().enumerate() {
          self.place_stack.push(ProjectionElem::Field(
            Field::from_usize(i),
            field.expect_ty(),
          ));
          field.super_visit_with(self);
          self.place_stack.pop();
        }
      }

      TyKind::Adt(adt_def, subst) => match adt_def.adt_kind() {
        ty::AdtKind::Struct => {
          for (i, field) in adt_def.all_fields().enumerate() {
            let ty = field.ty(self.tcx, subst);
            self
              .place_stack
              .push(ProjectionElem::Field(Field::from_usize(i), ty));
            self.visit_ty(ty);
            self.place_stack.pop();
          }
        }
        ty::AdtKind::Union => {
          // unsafe, so ignore
        }
        ty::AdtKind::Enum => {
          for (i, variant) in adt_def.variants.iter().enumerate() {
            let variant_index = VariantIdx::from_usize(i);
            let cast = PlaceElem::Downcast(
              Some(adt_def.variants[variant_index].ident.name),
              variant_index,
            );
            self.place_stack.push(cast);
            for (j, field) in variant.fields.iter().enumerate() {
              let ty = field.ty(self.tcx, subst);
              let field = ProjectionElem::Field(Field::from_usize(j), ty);
              self.place_stack.push(field);
              self.visit_ty(ty);
              self.place_stack.pop();
            }
            self.place_stack.pop();
          }
        }
      },

      TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => {
        self
          .place_stack
          .push(ProjectionElem::Index(Local::from_usize(0)));
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Ref(region, elem_ty, _) => {
        self.visit_region(region);
        self.place_stack.push(ProjectionElem::Deref);
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Closure(_, substs) => {
        self.visit_ty(substs.as_closure().tupled_upvars_ty());
      }

      TyKind::RawPtr(_)
      | TyKind::Projection(_)
      | TyKind::FnDef(_, _)
      | TyKind::FnPtr(_)
      | TyKind::Opaque(_, _)
      | TyKind::Foreign(_)
      | TyKind::Dynamic(_, _)
      | TyKind::Param(_)
      | TyKind::Never => {}

      _ if ty.is_primitive_ty() => {}

      _ => {
        warn!("unimplemented {:?} ({:?})", ty, ty.kind());
      }
    };

    if let Some(places) = self.places.as_mut() {
      places.insert(Place {
        local: self.local,
        projection: self.tcx.intern_place_elems(&self.place_stack),
      });
    }

    self.ty_stack.pop();
    ControlFlow::Continue(())
  }

  fn visit_region(&mut self, region: ty::Region<'tcx>) -> ControlFlow<Self::BreakTy> {
    match region {
      RegionKind::ReVar(region) => {
        let mutability = if self
          .ty_stack
          .iter()
          .any(|ty| ty.is_ref() && ty.ref_mutability().unwrap() == Mutability::Not)
        {
          Mutability::Not
        } else {
          Mutability::Mut
        };

        let place = Place {
          local: self.local,
          projection: self.tcx.intern_place_elems(&self.place_stack),
        };
        self.regions.insert(*region, (place, mutability));
      }
      RegionKind::ReStatic => {}
      _ => unreachable!("{:?}: {:?}", self.ty_stack.first().unwrap(), region),
    };

    ControlFlow::Continue(())
  }
}

pub fn interior_pointers<'tcx>(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> HashMap<RegionVid, (Place<'tcx>, Mutability)> {
  let ty = place.ty(body.local_decls(), tcx).ty;
  let mut region_collector = CollectRegions {
    tcx,
    local: place.local,
    place_stack: vec![],
    ty_stack: Vec::new(),
    regions: HashMap::default(),
    places: None,
  };
  region_collector.visit_ty(ty);
  region_collector.regions
}

pub fn interior_places<'tcx>(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> Vec<Place<'tcx>> {
  let ty = place.ty(body.local_decls(), tcx).ty;
  let mut region_collector = CollectRegions {
    tcx,
    local: place.local,
    place_stack: vec![],
    ty_stack: Vec::new(),
    regions: HashMap::default(),
    places: Some(HashSet::default()),
  };
  region_collector.visit_ty(ty);
  region_collector.places.unwrap().into_iter().collect()
}

pub fn arg_mut_ptrs<'tcx>(
  args: &[Place<'tcx>],
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> Vec<Place<'tcx>> {
  args
    .iter()
    .map(|place| {
      interior_pointers(*place, tcx, body)
        .into_iter()
        .filter_map(|(_, (place, mutability))| match mutability {
          Mutability::Mut => Some(place),
          Mutability::Not => None,
        })
        .map(|place| tcx.mk_place_deref(place))
    })
    .flatten()
    .collect::<Vec<_>>()
}

pub fn arg_places<'tcx>(args: &[Operand<'tcx>]) -> Vec<Place<'tcx>> {
  args
    .iter()
    .filter_map(|arg| operand_to_place(arg))
    .collect::<Vec<_>>()
}

pub fn hashmap_merge<K: Eq + Hash, V>(
  mut h1: HashMap<K, V>,
  h2: HashMap<K, V>,
  conflict: impl Fn(&mut V, V),
) -> HashMap<K, V> {
  for (k, v) in h2.into_iter() {
    match h1.entry(k) {
      Entry::Vacant(entry) => {
        entry.insert(v);
      }
      Entry::Occupied(mut entry) => {
        let entry = entry.get_mut();
        conflict(entry, v);
      }
    }
  }
  h1
}

#[derive(PartialEq, Eq, Debug)]
pub enum PlaceRelation {
  Super,
  Sub,
  Disjoint,
}

impl PlaceRelation {
  pub fn overlaps(&self) -> bool {
    *self != PlaceRelation::Disjoint
  }

  pub fn of(part_place: Place<'tcx>, whole_place: Place<'tcx>) -> Self {
    let locals_match = part_place.local == whole_place.local;
    if !locals_match {
      return PlaceRelation::Disjoint;
    }

    let projections_match = part_place
      .projection
      .iter()
      .zip(whole_place.projection.iter())
      .all(|(elem1, elem2)| {
        use ProjectionElem::*;
        match (elem1, elem2) {
          (Deref, Deref) => true,
          (Field(f1, _), Field(f2, _)) => f1 == f2,
          (Index(_), Index(_)) => true,
          (ConstantIndex { .. }, ConstantIndex { .. }) => true,
          (Subslice { .. }, Subslice { .. }) => true,
          (Downcast(_, v1), Downcast(_, v2)) => v1 == v2,
          _ => false,
        }
      });

    let is_sub_part = part_place.projection.len() >= whole_place.projection.len();
    let remaining_projection = if is_sub_part {
      &part_place.projection[whole_place.projection.len()..]
    } else {
      &whole_place.projection[part_place.projection.len()..]
    };

    if remaining_projection
      .iter()
      .any(|elem| matches!(elem, ProjectionElem::Deref))
    {
      return PlaceRelation::Disjoint;
    }

    if projections_match {
      if is_sub_part {
        PlaceRelation::Sub
      } else {
        PlaceRelation::Super
      }
    } else {
      PlaceRelation::Disjoint
    }
  }
}

#[derive(Default)]
pub struct PlaceCollector<'tcx> {
  pub places: Vec<Place<'tcx>>,
}

impl Visitor<'tcx> for PlaceCollector<'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.places.push(*place);
  }
}

pub fn pointer_for_place(place: Place<'tcx>, tcx: TyCtxt<'tcx>) -> Option<Place<'tcx>> {
  place
    .iter_projections()
    .rev()
    .find(|(_, elem)| matches!(elem, ProjectionElem::Deref))
    .map(|(place_ref, _)| Place {
      local: place_ref.local,
      projection: tcx.intern_place_elems(place_ref.projection),
    })
}

struct FindSpannedPlaces<'a, 'tcx> {
  body: &'a Body<'tcx>,
  span: Span,
  places: HashSet<(Place<'tcx>, Location)>,
}

impl Visitor<'tcx> for FindSpannedPlaces<'_, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
    // assignments to return value have a span of the entire function, which we want to avoid
    if let Some(local) = place.as_local() {
      if local.as_usize() == 0 {
        return;
      }
    }

    let span = match context {
      PlaceContext::MutatingUse(..) | PlaceContext::NonMutatingUse(..) => {
        let source_info = self.body.source_info(location);
        Some(source_info.span)
      }
      PlaceContext::NonUse(non_use) => match non_use {
        NonUseContext::VarDebugInfo => {
          if self.body.args_iter().any(|local| local == place.local) {
            let source_info = self.body.local_decls()[place.local].source_info;
            Some(source_info.span)
          } else {
            None
          }
        }
        _ => None,
      },
    };

    if let Some(span) = span {
      if self.span.contains(span) || span.contains(self.span) {
        self.places.insert((*place, location));
      }
    }
  }
}

pub fn span_to_places(body: &Body<'tcx>, span: Span) -> Vec<(Place<'tcx>, Location)> {
  let mut visitor = FindSpannedPlaces {
    body,
    span,
    places: HashSet::default(),
  };
  visitor.visit_body(body);
  visitor.places.into_iter().collect::<Vec<_>>()
}

pub fn dump_results<'tcx, A>(
  path: &str,
  body: &Body<'tcx>,
  results: &Results<'tcx, A>,
) -> Result<()>
where
  A: Analysis<'tcx>,
  A::Domain: DebugWithContext<A>,
{
  let graphviz = graphviz::Formatter::new(body, results, graphviz::OutputStyle::AfterOnly);
  let mut buf = Vec::new();
  dot::render(&graphviz, &mut buf)?;
  let mut file = File::create("/tmp/graph.dot")?;
  file.write_all(&buf)?;
  let status = Command::new("dot")
    .args(&["-Tpng", "/tmp/graph.dot", "-o", path])
    .status()?;
  if !status.success() {
    bail!("dot for {} failed", path)
  };
  Ok(())
}

pub fn mir_to_string(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> Result<String> {
  let mut buffer = Vec::new();
  write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut buffer)?;
  Ok(String::from_utf8(buffer)?)
}

pub fn location_to_string(location: Location, body: &Body<'_>) -> String {
  let block = &body.basic_blocks()[location.block];
  if location.statement_index == block.statements.len() {
    format!("{:?}", block.terminator())
  } else {
    format!("{:?}", block.statements[location.statement_index])
  }
}

struct SimplifyMir;
impl MirPass<'tcx> for SimplifyMir {
  fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    for block in body.basic_blocks_mut() {
      block.retain_statements(|stmt| {
        !matches!(
          stmt.kind,
          // TODO: variable_select_lhs test fails if we remove FakeRead
          // StatementKind::FakeRead(..)
          StatementKind::StorageLive(..) | StatementKind::StorageDead(..)
        )
      });

      let terminator = block.terminator_mut();
      terminator.kind = match terminator.kind {
        TerminatorKind::FalseEdge { real_target, .. } => TerminatorKind::Goto {
          target: real_target,
        },
        TerminatorKind::FalseUnwind { real_target, .. } => TerminatorKind::Goto {
          target: real_target,
        },
        _ => continue,
      }
    }
  }
}

pub fn get_body_with_borrowck_facts(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
) -> BodyWithBorrowckFacts<'tcx> {
  let local_def_id = tcx.hir().body_owner_def_id(body_id);
  let mut body_with_facts = rustc_mir::consumers::get_body_with_borrowck_facts(
    tcx,
    WithOptConstParam::unknown(local_def_id),
  );

  let body = &mut body_with_facts.body;
  SimplifyMir.run_pass(tcx, body);
  simplify::SimplifyCfg::new("flowistry").run_pass(tcx, body);

  body_with_facts
}

pub fn split_deref(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
) -> Option<(Place<'tcx>, &'tcx [PlaceElem<'tcx>])> {
  let deref_index = place
    .projection
    .iter()
    .enumerate()
    .rev()
    .find(|(_, elem)| matches!(elem, ProjectionElem::Deref))
    .map(|(i, _)| i)?;

  Some((
    Place {
      local: place.local,
      projection: tcx.intern_place_elems(&place.projection[..deref_index]),
    },
    &place.projection[deref_index + 1..],
  ))
}
