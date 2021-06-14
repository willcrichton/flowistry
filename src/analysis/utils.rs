use log::warn;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS, TypeFoldable, TypeVisitor},
};
use rustc_target::abi::VariantIdx;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::ops::ControlFlow;

pub use super::place_set::{PlaceDomain, PlaceIndex, PlaceSet};

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
  regions: HashMap<RegionVid, (Place<'tcx>, Mutability)>,
}

impl TypeVisitor<'tcx> for CollectRegions<'tcx> {
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
  };
  region_collector.visit_ty(ty);
  region_collector.regions
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

    if locals_match && projections_match {
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
    .find(|(_, elem)| match elem {
      ProjectionElem::Deref => true,
      _ => false,
    })
    .map(|(place_ref, _)| Place {
      local: place_ref.local,
      projection: tcx.intern_place_elems(place_ref.projection),
    })
}
