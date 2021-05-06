use super::intraprocedural::elapsed;
use super::utils::{self, PlaceRelation, PlaceSet};
use crate::config::{Config, MutabilityMode};
use log::debug;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_data_structures::graph::scc::Sccs;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    borrows::BorrowSet,
    regions::{ConstraintSccIndex, OutlivesConstraint},
    *,
  },
  ty::{RegionVid, TyCtxt},
};
use std::cell::RefCell;
use std::time::Instant;

pub struct Aliases<'tcx> {
  loans: IndexVec<RegionVid, PlaceSet<'tcx>>,
  loan_cache: RefCell<HashMap<Place<'tcx>, PlaceSet<'tcx>>>,
}

impl Aliases<'tcx> {
  fn compute_region_ancestors(
    sccs: &Sccs<RegionVid, ConstraintSccIndex>,
    regions_in_scc: &IndexVec<ConstraintSccIndex, BitSet<RegionVid>>,
    node: ConstraintSccIndex,
  ) -> HashMap<RegionVid, BitSet<ConstraintSccIndex>> {
    let new_set = || BitSet::new_empty(sccs.num_sccs());
    let set_merge = |s1: &mut BitSet<ConstraintSccIndex>, s2: BitSet<ConstraintSccIndex>| {
      s1.union(&s2);
    };

    let mut initial_set = new_set();
    initial_set.insert(node);

    let mut initial_map = HashMap::default();
    for r in regions_in_scc[node].iter() {
      initial_map.insert(r, initial_set.clone());
    }

    sccs
      .successors(node)
      .iter()
      .map(|child| {
        let in_child = regions_in_scc[*child]
          .iter()
          .map(|r| (r, initial_set.clone()))
          .collect::<HashMap<_, _>>();
        let grandchildren = Self::compute_region_ancestors(sccs, regions_in_scc, *child)
          .into_iter()
          .map(|(region, mut parents)| {
            parents.insert(node);
            (region, parents)
          })
          .collect::<HashMap<_, _>>();
        utils::hashmap_merge(in_child, grandchildren, set_merge)
      })
      .fold(initial_map, |h1, h2| {
        utils::hashmap_merge(h1, h2, set_merge)
      })
  }

  pub fn loans(&self, place: Place<'tcx>) -> PlaceSet<'tcx> {
    let compute_loans = || {
      self
        .loans
        .iter()
        .filter_map(|loans| {
          let matches_loan = loans
            .iter()
            .any(|loan| PlaceRelation::of(*loan, place).overlaps());
          let is_deref = place.is_indirect();
          (matches_loan && is_deref).then(|| loans.clone().into_iter())
        })
        .flatten()
        .chain(vec![place].into_iter())
        .collect::<HashSet<_>>()
    };

    self
      .loan_cache
      .borrow_mut()
      .entry(place)
      .or_insert_with(compute_loans)
      .clone()
  }

  pub fn build(
    config: &Config,
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    outlives_constraints: &'a Vec<OutlivesConstraint>,
    constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>,
  ) -> Self {
    let all_regions = body
      .local_decls()
      .indices()
      .map(|local| {
        let place = utils::local_to_place(local, tcx);
        utils::interior_pointers(place, tcx, body)
      })
      .fold(HashMap::default(), |mut h1, h2| {
        h1.extend(h2);
        h1
      });

    let start = Instant::now();
    let max_region = all_regions
      .keys()
      .map(|region| region.as_usize())
      .max()
      .unwrap_or(0)
      + 1;

    let mut regions_in_scc =
      IndexVec::from_elem_n(BitSet::new_empty(max_region), constraint_sccs.num_sccs());
    {
      let regions_in_constraint = outlives_constraints
        .iter()
        .map(|constraint| vec![constraint.sup, constraint.sub].into_iter())
        .flatten()
        .collect::<HashSet<_>>();
      for region in 0..max_region {
        let region = RegionVid::from_usize(region);
        if regions_in_constraint.contains(&region) {
          let scc = constraint_sccs.scc(region);
          regions_in_scc[scc].insert(region);
        }
      }
    }

    let root_region = RegionVid::from_usize(0);
    let root_scc = constraint_sccs.scc(root_region);
    let region_ancestors =
      Self::compute_region_ancestors(constraint_sccs, &regions_in_scc, root_scc);
    debug!("region ancestors: {:#?}", region_ancestors);

    let mut aliases = Aliases {
      loans: IndexVec::from_elem_n(HashSet::default(), max_region),
      loan_cache: RefCell::new(HashMap::default()),
    };

    for (region, (sub_place, mutability)) in all_regions {
      if mutability == Mutability::Mut
        || config.eval_mode.mutability_mode == MutabilityMode::IgnoreMut
      {
        aliases.loans[region].insert(tcx.mk_place_deref(sub_place));
      }
    }

    for (_, borrow) in borrow_set.iter_enumerated() {
      let mutability = borrow.kind.to_mutbl_lossy();
      if mutability == Mutability::Mut
        || config.eval_mode.mutability_mode == MutabilityMode::IgnoreMut
      {
        aliases.loans[borrow.region].insert(borrow.borrowed_place);
      }
    }

    elapsed("Alias setup", start);

    debug!("initial aliases {:#?}", aliases.loans);

    let start = Instant::now();
    loop {
      let mut changed = false;
      let prev_aliases = aliases.loans.clone();
      for (region, places) in aliases.loans.iter_enumerated_mut() {
        let alias_places = region_ancestors
          .get(&region)
          .map(|sccs| {
            let alias_regions = sccs
              .iter()
              .map(|scc_index| regions_in_scc[scc_index].iter())
              .flatten();

            alias_regions
              .filter_map(|region| {
                prev_aliases
                  .get(region)
                  .map(|places| places.clone().into_iter())
              })
              .flatten()
              .collect::<HashSet<_>>()
          })
          .unwrap_or_else(HashSet::default);

        let n = places.len();
        places.extend(alias_places.into_iter());
        changed = places.len() > n;
      }

      // TODO: needed for parity with Oxide, but doesn't seem to be needed for actual correctness?
      // for (_, borrow) in borrow_set.iter_enumerated() {
      //   let place = borrow.borrowed_place;
      //   if let Some(ProjectionElem::Deref) = place.projection.first() {
      //     let outer_projection = &place.projection[1..];
      //     let ty = body.local_decls()[place.local].ty;
      //     if let TyKind::Ref(RegionKind::ReVar(region), _, _) = ty.kind() {
      //       let new_loans = aliases.loans[*region].clone().into_iter().map(|loan| {
      //         let mut projection = loan.projection.to_vec();
      //         projection.extend_from_slice(outer_projection);
      //         Place {
      //           local: loan.local,
      //           projection: tcx.intern_place_elems(&projection),
      //         }
      //       });
      //       aliases
      //         .loans
      //         .get_mut(borrow.region)
      //         .unwrap()
      //         .extend(new_loans);
      //     }
      //   }
      // }

      if !changed {
        break;
      }
    }

    for v in aliases.loans.iter_mut() {
      *v = v.iter().map(|place| tcx.erase_regions(*place)).collect();
    }
    elapsed("Alias compute", start);

    debug!(
      "Aliases: {}",
      aliases
        .loans
        .iter_enumerated()
        .map(|(region, places)| format!("{:?}: {:?}", region, places))
        .collect::<Vec<_>>()
        .join(", ")
    );

    aliases
  }
}
