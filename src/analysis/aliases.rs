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
  ty::{self, RegionKind, RegionVid, Ty, TyS, TyCtxt, TyKind,  TypeFoldable, TypeVisitor},
};
use rustc_target::abi::VariantIdx;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::hash::Hash;
use std::ops::ControlFlow;

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  place_stack: Vec<Place<'tcx>>,
  ty_stack: Vec<Ty<'tcx>>,
  regions: HashMap<RegionVid, Place<'tcx>>,
}

impl TypeVisitor<'tcx> for CollectRegions<'tcx> {
  fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
    if self.ty_stack.iter().any(|visited_ty| TyS::same_type(ty, visited_ty)) {
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
    if let RegionKind::ReVar(region) = region {
      self
        .regions
        .insert(*region, *self.place_stack.last().unwrap());
    }

    ControlFlow::Continue(())
  }
}

pub(super) fn interior_pointers<'tcx>(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> HashMap<RegionVid, Place<'tcx>> {
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

#[derive(Debug)]
pub struct Aliases {
  borrow_aliases: IndexVec<PlaceIndex, BitSet<BorrowIndex>>,
  pub(super) synthetic_aliases: HashMap<PlaceIndex, HashSet<PlaceIndex>>,
}

impl Aliases {
  pub fn aliases<'a>(&'a self, borrow: BorrowIndex) -> impl Iterator<Item = PlaceIndex> + 'a {
    self
      .borrow_aliases
      .iter_enumerated()
      .filter_map(move |(place, borrows)| {
        if borrows.contains(borrow) {
          Some(place)
        } else {
          None
        }
      })
  }

  pub fn synthetic_aliases<'a>(
    &'a self,
    place: PlaceIndex,
  ) -> Box<dyn Iterator<Item = PlaceIndex> + 'a> {
    match self.synthetic_aliases.get(&place) {
      Some(s) => Box::new(s.iter().cloned()),
      None => Box::new(vec![].into_iter()),
    }
  }
}

pub struct AliasVisitor<'a, 'mir, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  pub(super) region_ancestors: HashMap<RegionVid, HashSet<ConstraintSccIndex>>,
  pub(super) constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>, // region_to_local: HashMap<RegionVid, Local>,
  pub(super) place_indices: &'a mut PlaceIndices<'tcx>,
  pub(super) aliases: Aliases,
  pub(super) input_regions: HashMap<RegionVid, Place<'tcx>>,
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
    let regions = interior_pointers(place, self.tcx, self.body);
    debug!("visited {:?} and found regions {:?}", place, regions);

    for (region, sub_place) in regions {
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
        .collect::<Vec<_>>();

      let sub_place_deref = self.tcx.mk_place_deref(sub_place);
      let sub_place_idx = self.place_indices.insert(&sub_place_deref);

      for idx in ty_borrows {
        let nborrows = self.borrow_set.len();
        self
          .aliases
          .borrow_aliases
          .ensure_contains_elem(sub_place_idx, || BitSet::new_empty(nborrows));
        debug!("alias {:?} to {:?}", sub_place_deref, idx);
        self.aliases.borrow_aliases[sub_place_idx].insert(idx);
      }
    }
  }
}

fn body_inputs<'tcx>(body: &Body<'tcx>, tcx: TyCtxt<'tcx>) -> Vec<Place<'tcx>> {
  (0..body.arg_count)
    .map(|i| Place {
      local: Local::from_usize(i + 1),
      projection: tcx.intern_place_elems(&[]),
    })
    .collect::<Vec<_>>()
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

  let aliases = Aliases {
    borrow_aliases: IndexVec::from_elem_n(
      BitSet::new_empty(borrow_set.len()),
      place_indices.count(),
    ),
    synthetic_aliases: HashMap::default(),
  };

  let input_regions = body_inputs(body, tcx)
    .into_iter()
    .map(|place| interior_pointers(place, tcx, body))
    .fold(HashMap::new(), |mut h1, h2| {
      h1.extend(h2);
      h1
    });

  let mut visitor = AliasVisitor {
    tcx,
    body,
    borrow_set,
    region_ancestors,
    constraint_sccs,
    place_indices,
    aliases,
    input_regions: input_regions.clone(),
  };
  visitor.visit_body(body);

  for input_place in input_regions.values() {
    visitor.process(*input_place);
  }

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
      merge(in_child, grandchildren, |h1, h2| h1 | h2)
    })
    .fold(initial_map, |h1, h2| merge(h1, h2, |h1, h2| h1 | h2))
}

