use log::{debug, warn};
use maplit::hashset;
use rustc_data_structures::graph::scc::Sccs;
use rustc_index::vec::IndexVec;
use rustc_middle::{
  mir::{
    borrows::BorrowSet,
    regions::{ConstraintSccIndex, OutlivesConstraint},
    *,
  },
  ty::{self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS, TypeFoldable, TypeVisitor},
};
use rustc_target::abi::VariantIdx;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::hash::Hash;
use std::ops::ControlFlow;

pub type PlaceSet<'tcx> = HashSet<Place<'tcx>>;

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  place_stack: Vec<Place<'tcx>>,
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

    let last_place = *self.place_stack.last().unwrap();
    self.ty_stack.push(ty);

    match ty.kind() {
      TyKind::Tuple(fields) => {
        for (i, field) in fields.iter().enumerate() {
          let place = self
            .tcx
            .mk_place_field(last_place, Field::from_usize(i), field.expect_ty());
          self.place_stack.push(place);
          field.super_visit_with(self);
          self.place_stack.pop();
        }
      }

      TyKind::Adt(adt_def, subst) => match adt_def.adt_kind() {
        ty::AdtKind::Struct => {
          for (i, field) in adt_def.all_fields().enumerate() {
            let ty = field.ty(self.tcx, subst);
            let place = self
              .tcx
              .mk_place_field(last_place, Field::from_usize(i), ty);
            self.place_stack.push(place);
            self.visit_ty(ty);
            self.place_stack.pop();
          }
        }
        ty::AdtKind::Union => {
          warn!("unimplemented {:?}", ty);
        }
        ty::AdtKind::Enum => {
          for (i, variant) in adt_def.variants.iter().enumerate() {
            let cast_place =
              self
                .tcx
                .mk_place_downcast(last_place, adt_def, VariantIdx::from_usize(i));
            for (j, field) in variant.fields.iter().enumerate() {
              let ty = field.ty(self.tcx, subst);
              let place = self
                .tcx
                .mk_place_field(cast_place, Field::from_usize(j), ty);
              self.place_stack.push(place);
              self.visit_ty(ty);
              self.place_stack.pop();
            }
          }
        }
      },

      TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => {
        let place = self.tcx.mk_place_index(last_place, Local::from_usize(0));
        self.place_stack.push(place);
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Ref(region, elem_ty, _) => {
        self.visit_region(region);
        self.place_stack.push(self.tcx.mk_place_deref(last_place));
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Closure(_, substs) => {
        self.visit_ty(substs.as_closure().tupled_upvars_ty());
      }

      TyKind::RawPtr(_) | TyKind::Projection(_) => {}
      _ if ty.is_primitive_ty() => {}

      _ => {
        warn!("unimplemented {:?}", ty);
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
        self
          .regions
          .insert(*region, (*self.place_stack.last().unwrap(), mutability));
      }
      RegionKind::ReStatic => {}
      _ => unreachable!("{:?}: {:?}", self.ty_stack.first().unwrap(), region),
    };

    ControlFlow::Continue(())
  }
}

pub(super) fn interior_pointers<'tcx>(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> HashMap<RegionVid, (Place<'tcx>, Mutability)> {
  let ty = place.ty(body.local_decls(), tcx).ty;
  let mut region_collector = CollectRegions {
    tcx,
    place_stack: vec![place],
    ty_stack: Vec::new(),
    regions: HashMap::new(),
  };
  region_collector.visit_ty(ty);
  region_collector.regions
}

#[derive(Debug, PartialEq, Eq)]
pub struct Aliases<'tcx>(IndexVec<RegionVid, PlaceSet<'tcx>>);

impl Aliases<'tcx> {
  pub fn aliases(
    &self,
    orig_place: Place<'tcx>,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
  ) -> PlaceSet<'tcx> {
    // let mut aliases =
    //   if let Some((ptr_place, ProjectionElem::Deref)) = place.iter_projections().last() {
    //     let ty = ptr_place.ty(body.local_decls(), tcx).ty;
    //     let region = if let TyKind::Ref(RegionKind::ReVar(region), _, _) = ty.kind() {
    //       *region
    //     } else {
    //       unreachable!("{:?}", ty.kind())
    //     };

    //     self
    //       .0
    //       .get(&region)
    //       .cloned()
    //       .unwrap_or_else(HashSet::new)
    //   } else {
    //     HashSet::new()
    //   };
    // aliases.insert(place);
    // aliases

    debug!(
      "initial place {:?} of type {:?}",
      orig_place,
      orig_place.ty(body.local_decls(), tcx).ty
    );

    orig_place.projection.iter().fold(
      hashset!(Place {
        local: orig_place.local,
        projection: tcx.intern_place_elems(&[])
      }),
      |places, projection_elem| {
        places
          .into_iter()
          .filter_map(|place| {
            let ty = place.ty(body.local_decls(), tcx).ty;
            Some(match projection_elem {
              ProjectionElem::Deref => {
                if ty.builtin_deref(false).is_none() {
                  return None;
                }

                let place = tcx.mk_place_deref(place);

                let region = if let TyKind::Ref(RegionKind::ReVar(region), _, _) = ty.kind() {
                  region
                } else {
                  return Some(hashset![place].into_iter());
                };

                self
                  .0
                  .get(*region)
                  .map(|loans| {
                    loans
                      .clone()
                      .into_iter()
                      .chain(vec![place].into_iter())
                      .collect::<HashSet<_>>()
                      .into_iter()
                  })
                  .unwrap_or_else(|| hashset! {place}.into_iter())
              }

              _ => {
                let mut projection = place.projection.to_vec();
                projection.push(projection_elem);
                hashset!(Place {
                  local: place.local,
                  projection: tcx.intern_place_elems(&projection)
                })
                .into_iter()
              }
            })
          })
          .flatten()
          .collect::<HashSet<_>>()
      },
    )
  }
}

pub fn compute_aliases(
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  outlives_constraints: &'a Vec<OutlivesConstraint>,
  constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>,
) -> Aliases<'tcx> {
  let max_region = outlives_constraints
    .iter()
    .map(|constraint| constraint.sup.as_usize().max(constraint.sub.as_usize()))
    .max()
    .unwrap_or(0)
    + 1;

  let root_region = RegionVid::from_usize(0);
  let root_scc = constraint_sccs.scc(root_region);
  let region_ancestors = compute_region_ancestors(constraint_sccs, max_region, root_scc);
  debug!("region ancestors: {:#?}", region_ancestors);

  let mut aliases = Aliases(IndexVec::from_elem_n(HashSet::new(), max_region));

  for (_, borrow) in borrow_set.iter_enumerated() {
    let mutability = borrow.kind.to_mutbl_lossy();
    if mutability != Mutability::Mut {
      continue;
    }

    aliases.0[borrow.region].insert(borrow.borrowed_place);
  }

  for local in body.args_iter() {
    let place = Place {
      local,
      projection: tcx.intern_place_elems(&[]),
    };

    for (region, (sub_place, mutability)) in interior_pointers(place, tcx, body) {
      if mutability != Mutability::Mut {
        continue;
      }

      aliases.0[region].insert(tcx.mk_place_deref(sub_place));
    }
  }

  debug!("initial aliases {:#?}", aliases);

  loop {
    let mut changed = false;
    let prev_aliases = aliases.0.clone();
    for (region, places) in aliases.0.iter_enumerated_mut() {
      let alias_regions = region_ancestors[&region]
        .iter()
        .map(|scc_index| regions_in_scc(constraint_sccs, max_region, *scc_index).into_iter())
        .flatten();

      let alias_places = alias_regions
        .filter_map(|region| {
          prev_aliases
            .get(region)
            .map(|places| places.clone().into_iter())
        })
        .flatten()
        .collect::<HashSet<_>>();

      let n = places.len();
      places.extend(alias_places.into_iter());
      changed = places.len() > n;
    }

    if !changed {
      break;
    }
  }

  for v in aliases.0.iter_mut() {
    *v = v.iter().map(|place| tcx.erase_regions(*place)).collect();
  }

  debug!("Aliases: {:#?}", aliases);

  aliases
}

fn hashmap_merge<K: Eq + Hash, V>(
  mut h1: HashMap<K, V>,
  h2: HashMap<K, V>,
  conflict: impl Fn(&V, &V) -> V,
) -> HashMap<K, V> {
  for (k, v) in h2.into_iter() {
    match h1.entry(k) {
      Entry::Vacant(entry) => {
        entry.insert(v);
      }
      Entry::Occupied(mut entry) => {
        let entry = entry.get_mut();
        *entry = conflict(&*entry, &v);
      }
    }
  }
  h1
}

fn regions_in_scc(
  sccs: &Sccs<RegionVid, ConstraintSccIndex>,
  max_region: usize,
  idx: ConstraintSccIndex,
) -> Vec<RegionVid> {
  (0..max_region)
    .map(|i| RegionVid::from_usize(i))
    .filter(|r| sccs.scc(*r) == idx)
    .collect::<Vec<_>>()
}

fn compute_region_ancestors(
  sccs: &Sccs<RegionVid, ConstraintSccIndex>,
  max_region: usize,
  node: ConstraintSccIndex,
) -> HashMap<RegionVid, HashSet<ConstraintSccIndex>> {
  let initial_hash = hashset! {node};

  let mut initial_map = HashMap::new();
  for r in regions_in_scc(sccs, max_region, node) {
    initial_map.insert(r, initial_hash.clone());
  }

  sccs
    .successors(node)
    .iter()
    .map(|child| {
      let in_child = regions_in_scc(sccs, max_region, *child)
        .into_iter()
        .map(|r| (r, initial_hash.clone()))
        .collect::<HashMap<_, _>>();
      let grandchildren = compute_region_ancestors(sccs, max_region, *child)
        .into_iter()
        .map(|(region, mut parents)| {
          parents.insert(node);
          (region, parents)
        })
        .collect::<HashMap<_, _>>();
      hashmap_merge(in_child, grandchildren, |h1, h2| h1 | h2)
    })
    .fold(initial_map, |h1, h2| {
      hashmap_merge(h1, h2, |h1, h2| h1 | h2)
    })
}
