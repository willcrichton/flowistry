use log::{debug, warn};
use maplit::hashset;
use rustc_data_structures::graph::scc::Sccs;
use rustc_middle::{
  mir::{
    borrows::BorrowSet,
    regions::{ConstraintSccIndex, OutlivesConstraint},
    visit::{PlaceContext, Visitor},
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
  recurse_refs: bool, // TODO: this feels like a hack
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
        if self.recurse_refs {
          self.place_stack.push(self.tcx.mk_place_deref(last_place));
          self.visit_ty(elem_ty);
          self.place_stack.pop();
        }
      }

      TyKind::Closure(_, substs) => {
        self.visit_ty(substs.as_closure().tupled_upvars_ty());
      }

      TyKind::RawPtr(_) => {}
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
    recurse_refs: true,
  };
  region_collector.visit_ty(ty);
  region_collector.regions
}

#[derive(Debug, PartialEq, Eq)]
pub struct Aliases<'tcx> {
  pub borrow_aliases: HashMap<Place<'tcx>, PlaceSet<'tcx>>,
  pub(super) synthetic_aliases: HashMap<Place<'tcx>, PlaceSet<'tcx>>,
}

impl Aliases<'tcx> {
  pub fn normalize(&self, place: Place<'tcx>, tcx: TyCtxt<'tcx>) -> PlaceSet<'tcx> {
    let place = tcx.erase_regions(place);
    let init_set = hashset! {Place {
      local: place.local,
      projection: tcx.intern_place_elems(&[]),
    }};

    place
      .projection
      .iter()
      .fold(init_set, |places, projection_elem| {
        let projection_elem = tcx.erase_regions(projection_elem);
        let projection_elem = match projection_elem {
          ProjectionElem::Index(_) => ProjectionElem::Index(Local::from_usize(0)),
          _ => projection_elem,
        };

        places
          .into_iter()
          .map(|place| match projection_elem {
            ProjectionElem::Deref => {
              // TODO: when should get return None? do we need to handle this?
              // has to do with order of execution of .process
              self
                .borrow_aliases
                .get(&place)
                .cloned()
                .unwrap_or_else(HashSet::new)
                .into_iter()
            }
            _ => {
              let mut projection = place.projection.to_vec();
              projection.push(projection_elem);
              hashset!(Place {
                local: place.local,
                projection: tcx.intern_place_elems(&projection),
              })
              .into_iter()
            }
          })
          .flatten()
          .collect::<HashSet<_>>()
      })
  }

  // pub fn aliases<'a>(&'a self, borrow: Place<'tcx>) -> impl Iterator<Item = Place<'tcx>> + 'a {
  //   self
  //     .borrow_aliases
  //     .iter()
  //     .filter_map(move |(place, borrows)| {
  //       if borrows.contains(borrow) {
  //         Some(place)
  //       } else {
  //         None
  //       }
  //     })
  // }

  // pub fn synthetic_aliases<'a>(
  //   &'a self,
  //   place: Place<'tcx>,
  // ) -> Box<dyn Iterator<Item = Place<'tcx>> + 'a> {
  //   match self.synthetic_aliases.get(&place) {
  //     Some(s) => Box::new(s.iter().cloned()),
  //     None => Box::new(vec![].into_iter()),
  //   }
  // }
}

// impl DebugWithContext<(&'_ BorrowSet<'tcx>, &'_ &'_ mut PlaceIndices<'tcx>)> for Aliases {
//   fn fmt_with(
//     &self,
//     (borrow_set, places): &(&BorrowSet<'tcx>, &&mut PlaceIndices<'tcx>),
//     f: &mut fmt::Formatter<'_>,
//   ) -> fmt::Result {
//     for (place, borrows) in self.borrow_aliases.iter_enumerated() {
//       write!(f, "{:?}: ", places.lookup(place))?;
//       for borrow in borrows.iter() {
//         write!(f, "{:?} ", borrow_set[borrow].borrowed_place)?;
//       }
//       write!(f, "\n")?;
//     }
//     Ok(())
//   }
// }

pub struct AliasVisitor<'a, 'mir, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  pub(super) region_ancestors: HashMap<RegionVid, HashSet<ConstraintSccIndex>>,
  pub(super) constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>, // region_to_local: HashMap<RegionVid, Local>,
  pub(super) aliases: Aliases<'tcx>,
  synthetic_local: usize,
}

impl AliasVisitor<'_, '_, 'tcx> {
  fn process(&mut self, local: Local) {
    let ty = self.body.local_decls()[local].ty;
    let mut region_collector = CollectRegions {
      tcx: self.tcx,
      place_stack: vec![Place {
        local,
        projection: self.tcx.intern_place_elems(&[]),
      }],
      ty_stack: Vec::new(),
      regions: HashMap::new(),
      recurse_refs: false,
    };
    region_collector.visit_ty(ty);
    let regions = region_collector.regions;

    debug!("visited {:?} and found regions {:?}", local, regions);

    for (region, (sub_place, _)) in regions {
      self.handle_synthetic_aliases(region, sub_place);

      let ty_borrows = self
        .borrow_set
        .indices()
        .filter(|idx| {
          let borrow = &self.borrow_set[*idx];
          let borrow_scc = self.constraint_sccs.scc(borrow.region);
          self
            .region_ancestors
            .get(&region)
            .map(|ancestors| ancestors.contains(&borrow_scc))
            .unwrap_or(false)
        })
        .map(|idx| {
          self
            .aliases
            .normalize(self.borrow_set[idx].borrowed_place, self.tcx)
        })
        .fold(HashSet::new(), |h1, h2| &h1 | &h2);

      let ty_borrows = if ty_borrows.len() == 0 {
        let synthetic_place = Place {
          local: Local::from_usize(self.synthetic_local),
          projection: self.tcx.intern_place_elems(&[]),
        };
        self.synthetic_local += 1;
        hashset![synthetic_place]
      } else {
        ty_borrows
      };

      debug!(
        "region {:?} in place {:?} has borrows {:?}",
        region, sub_place, ty_borrows
      );
      let sub_place = self
        .aliases
        .normalize(sub_place, self.tcx)
        .into_iter()
        .next()
        .unwrap();
      self.aliases.borrow_aliases.insert(sub_place, ty_borrows);
    }
  }
}

// fn body_inputs<'tcx>(body: &Body<'tcx>, tcx: TyCtxt<'tcx>) -> Vec<Place<'tcx>> {
//   (0..body.arg_count)
//     .map(|i| Place {
//       local: Local::from_usize(i + 1),
//       projection: tcx.intern_place_elems(&[]),
//     })
//     .collect::<Vec<_>>()
// }

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

  let aliases = Aliases {
    borrow_aliases: HashMap::new(),
    synthetic_aliases: HashMap::default(),
  };

  let mut visitor = AliasVisitor {
    tcx,
    body,
    borrow_set,
    region_ancestors,
    constraint_sccs,
    aliases,
    synthetic_local: body.local_decls().len(),
  };
  // visitor.visit_body(body);

  for local in body.local_decls().indices() {
    visitor.process(local);
  }

  debug!("Aliases: {:#?}", visitor.aliases);

  visitor.aliases
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

fn compute_region_ancestors(
  sccs: &Sccs<RegionVid, ConstraintSccIndex>,
  max_region: usize,
  node: ConstraintSccIndex,
) -> HashMap<RegionVid, HashSet<ConstraintSccIndex>> {
  let initial_hash = hashset! {node};

  let regions_in_scc = |idx| {
    (0..max_region)
      .map(|i| RegionVid::from_usize(i))
      .filter(|r| sccs.scc(*r) == idx)
      .collect::<Vec<_>>()
  };

  let mut initial_map = HashMap::new();
  for r in regions_in_scc(node) {
    initial_map.insert(r, initial_hash.clone());
  }

  sccs
    .successors(node)
    .iter()
    .map(|child| {
      let in_child = regions_in_scc(*child)
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
