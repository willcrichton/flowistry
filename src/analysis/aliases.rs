use super::place_index::{PlaceIndex, PlaceIndices};
use log::{debug, warn};
use rustc_data_structures::graph::scc::Sccs;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    borrows::{BorrowIndex, BorrowSet},
    regions::{ConstraintSccIndex, OutlivesConstraint},
    visit::Visitor,
    *,
  },
  ty::{self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TypeFoldable, TypeVisitor},
};
use rustc_target::abi::VariantIdx;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::hash::Hash;
use std::ops::ControlFlow;
// TODO: aliases no longer needs to be a dataflow pass, can just be a visitor

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  place_stack: Vec<Place<'tcx>>,
  regions: HashMap<RegionVid, Place<'tcx>>,
}

impl TypeVisitor<'tcx> for CollectRegions<'tcx> {
  fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
    let last_place = *self.place_stack.last().unwrap();

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

      _ => {
        warn!("unimplemented {:?}", ty);
      }
    };

    ControlFlow::Continue(())
  }

  fn visit_region(&mut self, region: ty::Region<'tcx>) -> ControlFlow<Self::BreakTy> {
    if let RegionKind::ReVar(region) = region {
      self
        .regions
        .insert(*region, *self.place_stack.last().unwrap());
    }

    ControlFlow::Continue(())
  }
}

#[derive(Debug)]
pub struct Aliases(IndexVec<PlaceIndex, BitSet<BorrowIndex>>);

impl Aliases {
  pub fn aliases<'a>(&'a self, borrow: BorrowIndex) -> impl Iterator<Item = PlaceIndex> + 'a {
    self
      .0
      .iter_enumerated()
      .filter_map(move |(place, borrows)| {
        if borrows.contains(borrow) {
          Some(place)
        } else {
          None
        }
      })
  }
}

pub struct AliasVisitor<'a, 'mir, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  region_ancestors: HashMap<RegionVid, HashSet<ConstraintSccIndex>>,
  constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>, // region_to_local: HashMap<RegionVid, Local>,
  place_indices: &'a mut PlaceIndices<'tcx>,
  aliases: Aliases,
}

impl<'tcx> Visitor<'tcx> for AliasVisitor<'_, '_, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, _rvalue: &Rvalue<'tcx>, _location: Location) {
    self.process(*place);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    match &terminator.kind {
      TerminatorKind::Call { destination, .. } => {
        if let Some((place, _)) = destination {
          self.process(*place);
        }
      }
      _ => {}
    }
  }
}

impl AliasVisitor<'_, '_, 'tcx> {
  fn process(&mut self, place: Place<'tcx>) {
    let ty = place.ty(self.body.local_decls(), self.tcx).ty;

    let mut region_collector = CollectRegions {
      tcx: self.tcx,
      place_stack: vec![place],
      regions: HashMap::new(),
    };
    region_collector.visit_ty(ty);
    debug!(
      "visited {:?} : {:?} and found regions {:?}",
      place, ty, region_collector.regions
    );

    for (region, sub_place) in region_collector.regions {
      let sub_place_deref = self.tcx.mk_place_deref(sub_place);
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
        .collect::<Vec<_>>();

      for idx in ty_borrows {
        let place_idx = self.place_indices.insert(&sub_place_deref);
        let nborrows = self.borrow_set.len();
        self
          .aliases
          .0
          .ensure_contains_elem(place_idx, || BitSet::new_empty(nborrows));
        debug!("alias {:?} to {:?}", sub_place_deref, idx);
        self.aliases.0[place_idx].insert(idx);
      }
    }
  }
}

pub fn compute_aliases(
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  outlives_constraints: &'a Vec<OutlivesConstraint>,
  constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>,
  place_indices: &'a mut PlaceIndices<'tcx>,
) -> Aliases {
  let max_region = outlives_constraints
    .iter()
    .map(|constraint| constraint.sup.as_usize().max(constraint.sub.as_usize()))
    .max()
    .unwrap_or(0)
    + 1;

  let root_region = RegionVid::from_usize(0);
  let root_scc = constraint_sccs.scc(root_region);
  let region_ancestors = compute_region_ancestors(constraint_sccs, max_region, root_scc);

  let aliases = Aliases(IndexVec::from_elem_n(
    BitSet::new_empty(borrow_set.len()),
    place_indices.count(),
  ));

  let mut visitor = AliasVisitor {
    tcx,
    body,
    borrow_set,
    region_ancestors,
    constraint_sccs,
    place_indices,
    aliases,
  };
  visitor.visit_body(body);

  visitor.aliases
}

fn merge<K: Eq + Hash, V>(
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
  let mut initial_hash = HashSet::new();
  initial_hash.insert(node);
  sccs
    .successors(node)
    .iter()
    .map(|child| {
      let in_child = (0..max_region)
        .map(|i| RegionVid::from_usize(i))
        .filter(|r| sccs.scc(*r) == *child)
        .map(|r| (r, initial_hash.clone()))
        .collect::<HashMap<_, _>>();
      let grandchildren = compute_region_ancestors(sccs, max_region, *child)
        .into_iter()
        .map(|(region, mut parents)| {
          parents.insert(node);
          (region, parents)
        })
        .collect::<HashMap<_, _>>();
      merge(in_child, grandchildren, |h1, h2| h1 | h2)
    })
    .fold(HashMap::new(), |h1, h2| merge(h1, h2, |h1, h2| h1 | h2))
}
