use std::{
  collections::hash_map::Entry,
  hash::Hash,
  io::Write,
  ops::ControlFlow,
  path::Path,
  process::{Command, Stdio},
};

use anyhow::{bail, Result};
use log::{trace, warn};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_graphviz as dot;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    pretty::write_mir_fn,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS, TypeFoldable, TypeVisitor},
};
use rustc_mir_dataflow::{fmt::DebugWithContext, graphviz, Analysis, Results};
use rustc_mir_transform::MirPass;
use rustc_span::Symbol;
use rustc_target::abi::VariantIdx;
use smallvec::SmallVec;

use crate::extensions::{is_extension_active, MutabilityMode};

pub trait OperandExt<'tcx> {
  fn to_place(&self) -> Option<Place<'tcx>>;
}

impl OperandExt<'tcx> for Operand<'tcx> {
  fn to_place(&self) -> Option<Place<'tcx>> {
    match self {
      Operand::Copy(place) | Operand::Move(place) => Some(*place),
      Operand::Constant(_) => None,
    }
  }
}

pub fn arg_mut_ptrs<'tcx>(
  args: &[(usize, Place<'tcx>)],
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  def_id: DefId,
) -> Vec<(usize, Place<'tcx>)> {
  let ignore_mut =
    is_extension_active(|mode| mode.mutability_mode == MutabilityMode::IgnoreMut);
  args
    .iter()
    .map(|(i, place)| {
      place
        .interior_pointers(tcx, body, def_id)
        .into_iter()
        .map(|(_, places)| {
          places
            .into_iter()
            .filter_map(|(place, mutability)| match mutability {
              Mutability::Mut => Some(place),
              Mutability::Not => ignore_mut.then(|| place),
            })
        })
        .flatten()
        .map(move |place| (*i, tcx.mk_place_deref(place)))
    })
    .flatten()
    .collect::<Vec<_>>()
}

pub fn arg_places<'tcx>(args: &[Operand<'tcx>]) -> Vec<(usize, Place<'tcx>)> {
  args
    .iter()
    .enumerate()
    .filter_map(|(i, arg)| arg.to_place().map(move |place| (i, place)))
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
      &part_place.projection[whole_place.projection.len() ..]
    } else {
      &whole_place.projection[part_place.projection.len() ..]
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
  fn visit_place(
    &mut self,
    place: &Place<'tcx>,
    _context: PlaceContext,
    _location: Location,
  ) {
    self.places.push(*place);
  }
}

pub fn dump_results<'tcx, A>(
  body: &Body<'tcx>,
  results: &Results<'tcx, A>,
  _def_id: DefId,
  _tcx: TyCtxt<'tcx>,
) -> Result<()>
where
  A: Analysis<'tcx>,
  A::Domain: DebugWithContext<A>,
{
  let graphviz =
    graphviz::Formatter::new(body, results, graphviz::OutputStyle::AfterOnly);
  let mut buf = Vec::new();
  dot::render(&graphviz, &mut buf)?;

  let output_dir = Path::new("target");
  // let fname = tcx.def_path_debug_str(def_id);
  let fname = "results";
  let output_path = output_dir.join(format!("{}.png", fname));

  let mut p = Command::new("dot")
    .args(&["-Tpng", "-o", &output_path.display().to_string()])
    .stdin(Stdio::piped())
    .spawn()?;

  p.stdin.as_mut().unwrap().write_all(&buf)?;
  let status = p.wait()?;

  if !status.success() {
    bail!("dot for {} failed", output_path.display())
  };

  Ok(())
}

pub fn location_to_string(location: Location, body: &Body<'_>) -> String {
  let block = &body.basic_blocks()[location.block];
  if location.statement_index == block.statements.len() {
    format!("{:?}", block.terminator().kind)
  } else {
    format!("{:?}", block.statements[location.statement_index].kind)
  }
}

pub struct SimplifyMir;
impl MirPass<'tcx> for SimplifyMir {
  fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    for block in body.basic_blocks_mut() {
      block.statements.retain(|stmt| {
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

pub trait PlaceExt<'tcx> {
  fn make(local: Local, projection: &[PlaceElem<'tcx>], tcx: TyCtxt<'tcx>) -> Self;
  fn from_ref(place: PlaceRef<'tcx>, tcx: TyCtxt<'tcx>) -> Self;
  fn from_local(local: Local, tcx: TyCtxt<'tcx>) -> Self;
  fn is_arg(&self, body: &Body<'tcx>) -> bool;
  fn is_direct(&self, body: &Body<'tcx>) -> bool;
  fn refs_in_projection(&self) -> SmallVec<[(PlaceRef<'tcx>, &[PlaceElem<'tcx>]); 2]>;
  fn interior_pointers(
    &self,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
  ) -> HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>;
  fn interior_places(
    &self,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
    depth_limit: Option<usize>,
  ) -> Vec<Place<'tcx>>;
}

impl PlaceExt<'tcx> for Place<'tcx> {
  fn make(local: Local, projection: &[PlaceElem<'tcx>], tcx: TyCtxt<'tcx>) -> Self {
    Place {
      local,
      projection: tcx.intern_place_elems(projection),
    }
  }

  fn from_ref(place: PlaceRef<'tcx>, tcx: TyCtxt<'tcx>) -> Self {
    Self::make(place.local, place.projection, tcx)
  }

  fn from_local(local: Local, tcx: TyCtxt<'tcx>) -> Self {
    Place::make(local, &[], tcx)
  }

  fn is_arg(&self, body: &Body<'tcx>) -> bool {
    let i = self.local.as_usize();
    i > 0 && i - 1 < body.arg_count
  }

  fn is_direct(&self, body: &Body<'tcx>) -> bool {
    !self.is_indirect() || self.is_arg(body)
  }

  fn refs_in_projection(&self) -> SmallVec<[(PlaceRef<'tcx>, &[PlaceElem<'tcx>]); 2]> {
    self
      .projection
      .iter()
      .enumerate()
      .filter_map(|(i, elem)| match elem {
        ProjectionElem::Deref => {
          let ptr = PlaceRef {
            local: self.local,
            projection: &self.projection[.. i],
          };
          let after = &self.projection[i + 1 ..];
          Some((ptr, after))
        }
        _ => None,
      })
      .collect()
  }

  fn interior_pointers(
    &self,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
  ) -> HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>> {
    let ty = self.ty(body.local_decls(), tcx).ty;
    let mut region_collector = CollectRegions {
      tcx,
      def_id,
      local: self.local,
      place_stack: self.projection.to_vec(),
      ty_stack: Vec::new(),
      regions: HashMap::default(),
      places: None,
      types: None,
      depth_limit: None,
    };
    region_collector.visit_ty(ty);
    region_collector.regions
  }

  fn interior_places(
    &self,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
    depth_limit: Option<usize>,
  ) -> Vec<Place<'tcx>> {
    let ty = self.ty(body.local_decls(), tcx).ty;
    let mut region_collector = CollectRegions {
      tcx,
      def_id,
      local: self.local,
      place_stack: self.projection.to_vec(),
      ty_stack: Vec::new(),
      regions: HashMap::default(),
      places: Some(HashSet::default()),
      types: None,
      depth_limit,
    };
    region_collector.visit_ty(ty);
    region_collector.places.unwrap().into_iter().collect()
  }
}

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  local: Local,
  place_stack: Vec<PlaceElem<'tcx>>,
  ty_stack: Vec<Ty<'tcx>>,
  places: Option<HashSet<Place<'tcx>>>,
  types: Option<HashSet<Ty<'tcx>>>,
  regions: HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
  depth_limit: Option<usize>,
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

    if let Some(limit) = self.depth_limit {
      if self.place_stack.len() > limit {
        return ControlFlow::Continue(());
      }
    }

    trace!(
      "exploring {:?} with {:?}",
      Place::make(self.local, &self.place_stack, self.tcx),
      ty
    );

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
            if !field.vis.is_accessible_from(self.def_id, self.tcx) {
              continue;
            }

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
      places.insert(Place::make(self.local, &self.place_stack, self.tcx));
    }

    if let Some(types) = self.types.as_mut() {
      types.insert(ty);
    }

    self.ty_stack.pop();
    ControlFlow::Continue(())
  }

  fn visit_region(&mut self, region: ty::Region<'tcx>) -> ControlFlow<Self::BreakTy> {
    trace!("visiting region {:?}", region);
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

        let place = Place::make(self.local, &self.place_stack, self.tcx);

        self
          .regions
          .entry(*region)
          .or_default()
          .push((place, mutability));

        // for initialization setup of Aliases::build
        if let Some(places) = self.places.as_mut() {
          places.insert(self.tcx.mk_place_deref(place));
        }
      }
      RegionKind::ReStatic | RegionKind::ReErased => {}
      _ => unreachable!("{:?}: {:?}", self.ty_stack.first().unwrap(), region),
    };

    ControlFlow::Continue(())
  }
}

pub trait BodyExt<'tcx> {
  type AllReturnsIter<'a>: Iterator<Item = Location>
  where
    Self: 'a;
  fn all_returns(&self) -> Self::AllReturnsIter<'_>;

  type AllLocationsIter<'a>: Iterator<Item = Location>
  where
    Self: 'a;
  fn all_locations(&self) -> Self::AllLocationsIter<'_>;

  type LocationsIter: Iterator<Item = Location>;
  fn locations_in_block(&self, block: BasicBlock) -> Self::LocationsIter;

  fn debug_info_name_map(&self) -> HashMap<Local, Symbol>;

  fn to_string(&self, tcx: TyCtxt<'tcx>) -> Result<String>;
}

impl BodyExt<'tcx> for Body<'tcx> {
  type AllReturnsIter<'a>
  where
    Self: 'a,
  = impl Iterator<Item = Location>;
  fn all_returns(&self) -> Self::AllReturnsIter<'_> {
    self
      .basic_blocks()
      .iter_enumerated()
      .filter_map(|(block, data)| match data.terminator().kind {
        TerminatorKind::Return => Some(Location {
          block,
          statement_index: data.statements.len(),
        }),
        _ => None,
      })
  }

  type AllLocationsIter<'a>
  where
    Self: 'a,
  = impl Iterator<Item = Location>;
  fn all_locations(&self) -> Self::AllLocationsIter<'_> {
    self
      .basic_blocks()
      .iter_enumerated()
      .map(|(block, data)| {
        (0 .. data.statements.len() + 1).map(move |statement_index| Location {
          block,
          statement_index,
        })
      })
      .flatten()
  }

  type LocationsIter = impl Iterator<Item = Location>;
  fn locations_in_block(&self, block: BasicBlock) -> Self::LocationsIter {
    let num_stmts = self.basic_blocks()[block].statements.len();
    (0 ..= num_stmts).map(move |statement_index| Location {
      block,
      statement_index,
    })
  }

  fn debug_info_name_map(&self) -> HashMap<Local, Symbol> {
    self
      .var_debug_info
      .iter()
      .filter_map(|info| match info.value {
        VarDebugInfoContents::Place(place) => Some((place.local, info.name)),
        _ => None,
      })
      .collect()
  }

  fn to_string(&self, tcx: TyCtxt<'tcx>) -> Result<String> {
    let mut buffer = Vec::new();
    write_mir_fn(tcx, self, &mut |_, _| Ok(()), &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
  }
}

// #[derive(Debug)]
// struct Loop {
//   header: Location,
//   body: LocationSet,
// }

// fn find_loops(
//   body: &Body,
//   location_domain: &Rc<LocationDomain>,
// ) -> (Vec<Loop>, LocationSet) {
//   let mut loops = vec![];
//   for node in body.basic_blocks().indices() {
//     for successor in body.successors(node) {
//       if body.dominators().is_dominated_by(node, successor) {
//         let mut loop_body = HashSet::default();
//         loop_body.insert(successor);

//         let mut stack = vec![node];
//         while !stack.is_empty() {
//           let n = stack.pop().unwrap();
//           loop_body.insert(n);
//           stack.extend(
//             body.predecessors()[n]
//               .iter()
//               .filter(|p| **p != successor && !loop_body.contains(p))
//               .copied(),
//           );
//         }

//         loops.push(Loop {
//           header: successor.start_location(),
//           body: loop_body
//             .into_iter()
//             .map(|block| body.locations_in_block(block))
//             .flatten()
//             .collect_indices(location_domain.clone()),
//         });
//       }
//     }
//   }

//   let mut outer = body
//     .all_locations()
//     .collect_indices(location_domain.clone());
//   for l in &loops {
//     outer.subtract(&l.body);
//   }

//   (loops, outer)
// }
