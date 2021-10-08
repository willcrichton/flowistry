use super::{
  extensions::{is_extension_active, PointerMode},
  indexed::{IndexSetIteratorExt, IndexedDomain, ToIndex},
  indexed_impls::{NormalizedPlaces, PlaceDomain, PlaceIndex, PlaceSet},
  utils::{self, elapsed, PlaceRelation},
};
use log::{debug, trace};

use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::{
  fx::{FxHashMap as HashMap, FxHashSet as HashSet},
  graph::{scc::Sccs, vec_graph::VecGraph},
};
use rustc_hir::def_id::DefId;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{RegionKind, RegionVid, TyCtxt, TyKind, TyS},
};
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Default)]
struct GatherBorrows<'tcx> {
  borrows: Vec<(RegionVid, BorrowKind, Place<'tcx>)>,
}

impl Visitor<'tcx> for GatherBorrows<'tcx> {
  fn visit_assign(&mut self, _place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, _location: Location) {
    if let Rvalue::Ref(region, kind, borrowed_place) = *rvalue {
      let region_vid = match region {
        RegionKind::ReVar(region_vid) => *region_vid,
        _ => unreachable!(),
      };
      self.borrows.push((region_vid, kind, borrowed_place));
    }
  }
}

#[derive(Debug, Clone)]
pub struct Conflicts<'tcx> {
  pub subs: PlaceSet<'tcx>,
  pub supers: PlaceSet<'tcx>,
  pub single_pointee: bool,
}

impl Conflicts<'tcx> {
  pub fn iter<'a>(&'a self) -> impl Iterator<Item = PlaceIndex> + 'a {
    self.subs.indices().chain(self.supers.indices())
  }
}

struct FindPlaces<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  def_id: DefId,
  places: Vec<Place<'tcx>>,
}

impl Visitor<'tcx> for FindPlaces<'_, 'tcx> {
  // this is needed for eval? not sure why locals wouldn't show up in the body as places,
  // maybe optimized out or something
  fn visit_local_decl(&mut self, local: Local, _local_decl: &LocalDecl<'tcx>) {
    self.places.push(utils::local_to_place(local, self.tcx));
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.places.push(*place);
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let is_borrow = matches!(rvalue, Rvalue::Ref(..));
    if is_borrow {
      self.places.push(self.tcx.mk_place_deref(*place));
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    self.super_terminator(terminator, location);

    match &terminator.kind {
      TerminatorKind::Call { args, .. } => {
        let arg_places = utils::arg_places(args);
        let arg_mut_ptrs = utils::arg_mut_ptrs(&arg_places, self.tcx, self.body, self.def_id);
        self
          .places
          .extend(arg_mut_ptrs.into_iter().map(|(_, place)| place));
      }

      _ => {}
    }
  }
}

pub struct Aliases<'tcx> {
  conflicts: IndexVec<PlaceIndex, Conflicts<'tcx>>,
  pub place_domain: Rc<PlaceDomain<'tcx>>,
}

rustc_index::newtype_index! {
  pub struct ConstraintSccIndex {
      DEBUG_FORMAT = "cs{}"
  }
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

  fn compute_region_info(
    body_with_facts: &BodyWithBorrowckFacts<'_>,
    region_to_pointers: &HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
  ) -> (
    usize,
    IndexVec<ConstraintSccIndex, BitSet<RegionVid>>,
    HashMap<RegionVid, BitSet<ConstraintSccIndex>>,
  ) {
    let outlives_constraints = body_with_facts
      .input_facts
      .subset_base
      .iter()
      .map(|(r1, r2, _)| (*r1, *r2))
      .collect::<Vec<_>>();
    debug!("outlives_constraints: {:?}", outlives_constraints);

    let max_region = region_to_pointers
      .keys()
      .chain(
        outlives_constraints
          .iter()
          .map(|(r1, r2)| vec![r1, r2].into_iter())
          .flatten(),
      )
      .map(|region| region.as_usize())
      .max()
      .unwrap_or(0)
      + 1;

    let static_region = RegionVid::from_usize(0);
    let mut processed_constraints = outlives_constraints
      .clone()
      .into_iter()
      // static region outlives everything
      .chain((1..max_region).map(|i| (static_region, RegionVid::from_usize(i))))
      .collect::<Vec<_>>();

    if is_extension_active(|mode| mode.pointer_mode == PointerMode::Conservative) {
      processed_constraints
        .extend(generate_conservative_constraints(tcx, body, region_to_pointers).into_iter());
    }

    let region_graph = VecGraph::new(max_region, processed_constraints);
    let constraint_sccs: Sccs<_, ConstraintSccIndex> = Sccs::new(&region_graph);

    let mut regions_in_scc =
      IndexVec::from_elem_n(BitSet::new_empty(max_region), constraint_sccs.num_sccs());
    {
      let regions_in_constraint = outlives_constraints
        .iter()
        .map(|constraint| vec![constraint.0, constraint.1].into_iter())
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
    trace!("regions_in_scc: {:?}", regions_in_scc);

    let root_scc = constraint_sccs.scc(static_region);
    let region_ancestors =
      Self::compute_region_ancestors(&constraint_sccs, &regions_in_scc, root_scc);
    trace!("region ancestors: {:?}", region_ancestors);

    (max_region, regions_in_scc, region_ancestors)
  }

  fn compute_conflicts(
    place_domain: &Rc<PlaceDomain<'tcx>>,
    mut all_aliases: HashMap<Place<'tcx>, HashSet<Place<'tcx>>>,
  ) -> IndexVec<PlaceIndex, Conflicts<'tcx>> {
    IndexVec::from_fn_n(
      |place| {
        let aliases = all_aliases
          .remove(place_domain.value(place))
          .unwrap_or_default();
        let single_pointee = {
          // If there is only one pointer at every level of indirection, then
          // there is only one possible place pointed-to
          let deref_counts = aliases
            .iter()
            .map(|place| {
              place
                .projection
                .iter()
                .filter(|elem| *elem == ProjectionElem::Deref)
                .count()
            })
            .collect::<HashSet<_>>();
          deref_counts.len() == aliases.len()
        };

        let (subs, supers): (Vec<_>, Vec<_>) = aliases
          .iter()
          .map(|alias| {
            place_domain
              .as_vec()
              .iter_enumerated()
              .filter_map(move |(idx, place)| {
                let relation = PlaceRelation::of(*place, *alias);
                relation.overlaps().then(move || (relation, idx))
              })
          })
          .flatten()
          .partition(|(relation, _)| match relation {
            PlaceRelation::Sub => true,
            PlaceRelation::Super => false,
            PlaceRelation::Disjoint => unreachable!(),
          });

        let to_set = |v: Vec<(PlaceRelation, PlaceIndex)>| {
          v.into_iter()
            .map(|(_, idx)| idx)
            .collect_indices(place_domain.clone())
        };

        Conflicts {
          subs: to_set(subs),
          supers: to_set(supers),
          single_pointee,
        }
      },
      place_domain.as_vec().len(),
    )
  }

  fn compute_place_domain(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
    loans: &IndexVec<RegionVid, HashSet<Place<'tcx>>>,
  ) -> (
    Rc<PlaceDomain<'tcx>>,
    HashMap<Place<'tcx>, HashSet<Place<'tcx>>>,
  ) {
    let compute_aliases = |place: Place<'tcx>| {
      let mut aliases = HashSet::default();
      aliases.insert(place);

      let (ptr, projection_past_deref) = match utils::split_deref(place, tcx) {
        Some(fields) => fields,
        _ => {
          return aliases;
        }
      };
      let (region, orig_ty) = match ptr.ty(body.local_decls(), tcx).ty.kind() {
        TyKind::Ref(RegionKind::ReVar(region), ty, _) => (*region, ty),
        // ty => unreachable!("{:?} / {:?}", place, ty),
        // TODO: how to deal with box?
        _ => {
          return aliases;
        }
      };

      let region_aliases = loans[region].iter().map(|loan| {
        let loan_ty = loan.ty(body.local_decls(), tcx).ty;
        if TyS::same_type(orig_ty, loan_ty) {
          let mut projection = loan.projection.to_vec();
          projection.extend(projection_past_deref);
          utils::mk_place(loan.local, &projection, tcx)
        } else {
          *loan
        }
      });
      aliases.extend(region_aliases);
      aliases
    };

    let mut finder = FindPlaces {
      tcx,
      body,
      def_id,
      places: Vec::new(),
    };
    finder.visit_body(body);

    let normalized_places = Rc::new(RefCell::new(NormalizedPlaces::new(tcx, def_id)));

    let all_aliases = finder
      .places
      .iter()
      .map(|place| {
        let norm_place = normalized_places.borrow_mut().normalize(*place);
        // can't compute aliases w/ normalized place b/c that has regions erased
        (norm_place, compute_aliases(*place))
      })
      .collect::<HashMap<_, _>>();

    finder.places.extend(
      all_aliases
        .values()
        .map(|aliases| aliases.iter().copied())
        .flatten(),
    );

    let all_ptrs = finder
      .places
      .iter()
      .map(|place| {
        place
          .iter_projections()
          .filter_map(|(place_ref, elem)| match elem {
            ProjectionElem::Deref => {
              Some(utils::mk_place(place_ref.local, place_ref.projection, tcx))
            }
            _ => None,
          })
      })
      .flatten()
      .collect::<HashSet<_>>();

    let all_places = finder
      .places
      .into_iter()
      .chain(all_ptrs.into_iter())
      .collect::<HashSet<_>>();

    trace!("Places: {:#?}", all_places);
    debug!("Place domain size: {}", all_places.len());

    (
      Rc::new(PlaceDomain::new(all_places, normalized_places)),
      all_aliases,
    )
  }

  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    let start = Instant::now();
    let body = &body_with_facts.body;

    // Get a mapping from region -> {set of references with that region}
    let region_to_pointers = body
      .local_decls()
      .indices()
      .map(|local| {
        let place = utils::local_to_place(local, tcx);
        utils::interior_pointers(place, tcx, body, def_id)
      })
      .fold(
        HashMap::default(),
        |mut h1: HashMap<RegionVid, Vec<_>>, h2| {
          for (k, vs) in h2.into_iter() {
            h1.entry(k).or_default().extend(&vs);
          }
          h1
        },
      );

    // Get the graph of which regions outlive which other ones
    let (max_region, regions_in_scc, region_ancestors) =
      Self::compute_region_info(body_with_facts, &region_to_pointers, tcx, body);

    // Initialize the loan set where loan['a] = {*x} if x: &'a T
    let mut loans = IndexVec::from_elem_n(HashSet::default(), max_region);
    for (region, places) in region_to_pointers.iter() {
      for (sub_place, _) in places {
        loans[*region].insert(tcx.mk_place_deref(*sub_place));
      }
    }

    // Given expressions e = &'a p, add p to loan['a]
    let mut gather_borrows = GatherBorrows::default();
    gather_borrows.visit_body(body);
    for (region, _, place) in gather_borrows.borrows.into_iter() {
      loans[region].insert(place);
    }

    elapsed("Alias setup", start);
    trace!("initial aliases {:?}", loans);

    // Propagate all loans where if 'a : 'b, then add loan['a] to loan['b].
    // Iterate to fixpoint.
    let start = Instant::now();
    loop {
      let mut changed = false;
      let prev_aliases = loans.clone();
      for (region, places) in loans.iter_enumerated_mut() {
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
                  .map(|places| places.iter().copied())
              })
              .flatten()
              .collect::<HashSet<_>>()
          })
          .unwrap_or_default();

        let orig_len = places.len();
        places.extend(&alias_places);
        changed |= orig_len != places.len();
      }

      if !changed {
        break;
      }
    }

    // Eagerly materialize every Place we will use in the computation, and generate initial
    // alias sets.
    let (place_domain, all_aliases) = Self::compute_place_domain(tcx, body, def_id, &loans);

    // Extend alias sets to all conflicts.
    let conflicts = Self::compute_conflicts(&place_domain, all_aliases);

    trace!(
      "conflicts: {}",
      conflicts
        .iter_enumerated()
        .map(|(place, conflicts)| { format!("{:?}: {:?}", place_domain.value(place), conflicts) })
        .collect::<Vec<_>>()
        .join("\n")
    );

    elapsed("Alias compute", start);

    Aliases {
      place_domain,
      conflicts,
    }
  }

  pub fn conflicts(&self, place: impl ToIndex<Place<'tcx>>) -> &Conflicts<'tcx> {
    &self.conflicts[place.to_index(&self.place_domain)]
  }
}

pub fn generate_conservative_constraints<'tcx>(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  region_to_pointers: &HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
) -> Vec<(RegionVid, RegionVid)> {
  let get_ty = |p| tcx.mk_place_deref(p).ty(body.local_decls(), tcx).ty;
  let same_ty = |p1, p2| TyS::same_type(get_ty(p1), get_ty(p2));

  region_to_pointers
    .iter()
    .map(|(region, places)| {
      region_to_pointers
        .iter()
        // find other regions that contain a loan matching any type in places
        .filter(|(other_region, other_places)| {
          *region != **other_region
            && places.iter().any(|(place, _)| {
              other_places
                .iter()
                .any(|(other_place, _)| same_ty(*place, *other_place))
            })
        })
        // add 'a : 'b and 'b : 'a to ensure the lifetimes are considered equal
        .map(|(other_region, _)| {
          vec![(*region, *other_region), (*other_region, *region)].into_iter()
        })
        .flatten()
        .collect::<Vec<_>>()
        .into_iter()
    })
    .flatten()
    .collect::<Vec<_>>()
}
