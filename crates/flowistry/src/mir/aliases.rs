use crate::{
  block_timer,
  extensions::{is_extension_active, PointerMode},
  indexed::{
    impls::{NormalizedPlaces, PlaceDomain, PlaceIndex, PlaceSet},
    IndexMatrix, IndexSetIteratorExt, IndexedDomain, ToIndex,
  },
  mir::utils::{self, PlaceRelation},
};
use log::{debug, info, trace};
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
use std::{cell::RefCell, rc::Rc};

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
  pub aliases: IndexMatrix<Place<'tcx>, Place<'tcx>>,
  pub deps: IndexMatrix<Place<'tcx>, Place<'tcx>>,
  pub subs: IndexMatrix<Place<'tcx>, Place<'tcx>>,
  pub supers: IndexMatrix<Place<'tcx>, Place<'tcx>>,
  pub place_domain: Rc<PlaceDomain<'tcx>>,
}

rustc_index::newtype_index! {
  pub struct ConstraintSccIndex {
      DEBUG_FORMAT = "cs{}"
  }
}

impl Aliases<'tcx> {
  fn process_region_sccs(
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

        let grandchildren = Self::process_region_sccs(sccs, regions_in_scc, *child)
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

  fn compute_region_ancestors(
    body_with_facts: &BodyWithBorrowckFacts<'tcx>,
    region_to_pointers: &HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
  ) -> IndexVec<RegionVid, BitSet<RegionVid>> {
    let outlives_constraints = body_with_facts
      .input_facts
      .subset_base
      .iter()
      .map(|(r1, r2, _)| (*r1, *r2))
      .collect::<HashSet<_>>();
    trace!("outlives_constraints: {:?}", outlives_constraints);

    let max_region = region_to_pointers
      .keys()
      .chain(
        outlives_constraints
          .iter()
          .map(|(r1, r2)| [r1, r2])
          .flatten(),
      )
      .map(|region| region.as_usize())
      .max()
      .unwrap_or(0)
      + 1;

    let body = &body_with_facts.body;

    // All regions for references in function arguments
    let _abstract_regions = body
      .args_iter()
      .map(|local| {
        let arg = utils::local_to_place(local, tcx);
        utils::interior_pointers(arg, tcx, body, def_id).into_keys()
      })
      .flatten()
      .collect::<HashSet<_>>();

    let static_region = RegionVid::from_usize(0);
    let mut processed_constraints = outlives_constraints
      .clone()
      .into_iter()
      //
      // Static region outlives everything, so add static :> r for all r
      .chain((1..max_region).map(|i| (static_region, RegionVid::from_usize(i))))
      //
      // Outlives-constraints on abstract regions are useful for borrow checking but aren't
      // useful for alias-analysis. Eg if self : &'a mut (i32, i32) and x = &'b mut *self.0,
      // then knowing 'a : 'b would naively add self to the loan set of 'b. So for increased
      // precision, we can safely filter any constraints 'a : _ where 'a is abstract.
      // See the interprocedural_field_independence test for an example of where this works
      // and also how it breaks.
      //
      // FIXME: need to figure out the right approximation here for field-sensitivity
      // .filter(|(sup, sub)| !(abstract_regions.contains(sup) && abstract_regions.contains(sub)))
      .collect::<Vec<_>>();

    if is_extension_active(|mode| mode.pointer_mode == PointerMode::Conservative) {
      processed_constraints.extend(generate_conservative_constraints(
        tcx,
        &body_with_facts.body,
        region_to_pointers,
      ));
    }

    let region_graph = VecGraph::new(max_region, processed_constraints);
    let constraint_sccs: Sccs<_, ConstraintSccIndex> = Sccs::new(&region_graph);

    let mut regions_in_scc =
      IndexVec::from_elem_n(BitSet::new_empty(max_region), constraint_sccs.num_sccs());
    {
      let regions_in_constraint = outlives_constraints
        .iter()
        .map(|constraint| [constraint.0, constraint.1])
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
    let region_to_ancestor_sccs =
      Self::process_region_sccs(&constraint_sccs, &regions_in_scc, root_scc);

    let region_ancestors = IndexVec::from_fn_n(
      |region| {
        let mut ancestors = BitSet::new_empty(max_region);
        if let Some(sccs) = region_to_ancestor_sccs.get(&region) {
          for scc in sccs.iter() {
            ancestors.union(&regions_in_scc[scc]);
          }
        }
        ancestors
      },
      max_region,
    );
    debug!(
      "region ancestors: {:?}",
      region_ancestors.iter_enumerated().collect::<Vec<_>>()
    );

    region_ancestors
  }

  fn place_deps(
    place: Place<'tcx>,
    all_aliases: &IndexMatrix<Place<'tcx>, Place<'tcx>>,
    body: &Body<'tcx>,
    tcx: TyCtxt<'tcx>,
  ) -> HashSet<Place<'tcx>> {
    let aliases = all_aliases.row(place).copied();
    let ptr_deps = utils::pointers_in_place(place, tcx)
      .into_iter()
      .map(|ptr| Self::place_deps(ptr, all_aliases, body, tcx))
      .flatten();

    let maybe_place = if utils::is_direct(place, body) {
      vec![place]
    } else {
      vec![]
    };

    maybe_place
      .into_iter()
      .chain(aliases)
      .chain(ptr_deps)
      .collect()
  }

  // TODO: extremely ugly return type
  fn compute_conflicts(
    place_domain: &Rc<PlaceDomain<'tcx>>,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    all_aliases: HashMap<Place<'tcx>, HashSet<Place<'tcx>>>,
  ) -> (
    IndexMatrix<Place<'tcx>, Place<'tcx>>,
    IndexMatrix<Place<'tcx>, Place<'tcx>>,
    IndexMatrix<Place<'tcx>, Place<'tcx>>,
    IndexMatrix<Place<'tcx>, Place<'tcx>>,
  ) {
    let new_mtx = || IndexMatrix::new(place_domain.clone(), place_domain.clone());
    let mut deps_map = new_mtx();
    let mut aliases_map = new_mtx();
    let mut subs_map = new_mtx();
    let mut supers_map = new_mtx();

    for (place, aliases) in all_aliases.into_iter() {
      let direct_aliases = aliases
        .iter()
        .filter(|alias| utils::is_direct(**alias, body))
        .copied()
        .collect_indices(place_domain.clone());
      aliases_map.union_into_row(place, &direct_aliases);
    }

    for place in place_domain.as_vec().iter() {
      let deps = Self::place_deps(*place, &aliases_map, body, tcx)
        .into_iter()
        .collect_indices(place_domain.clone());
      deps_map.union_into_row(place, &deps);
    }

    for place in place_domain.as_vec().iter() {
      let (subs, supers): (Vec<_>, Vec<_>) = place_domain
        .as_vec()
        .iter_enumerated()
        .filter_map(move |(idx, other_place)| {
          let relation = PlaceRelation::of(*other_place, *place);
          (relation.overlaps() && utils::is_direct(*other_place, body))
            .then(move || (relation, idx))
        })
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

      subs_map.union_into_row(place, &to_set(subs));
      supers_map.union_into_row(place, &to_set(supers));
    }

    debug!("aliases_map: {:?}", aliases_map);
    debug!("deps_map: {:?}", deps_map);
    debug!("supers_map: {:?}", supers_map);
    debug!("subs_map: {:?}", subs_map);

    (aliases_map, deps_map, subs_map, supers_map)
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

      if utils::is_direct(place, body) {
        return aliases;
      }

      // If place = (*p).1, then get ptr = p and projection_past_deref = .1
      let (deref_index, _) = place
        .projection
        .iter()
        .enumerate()
        .rev()
        .find(|(_, elem)| matches!(elem, ProjectionElem::Deref))
        .unwrap();

      let ptr = utils::mk_place(place.local, &place.projection[..deref_index], tcx);
      let projection_past_deref = &place.projection[deref_index + 1..];

      let (region, orig_ty) = match ptr.ty(body.local_decls(), tcx).ty.kind() {
        TyKind::Ref(RegionKind::ReVar(region), ty, _) => (*region, ty),
        // ty => unreachable!("{:?} / {:?}", place, ty),
        // TODO: how to deal with box?
        _ => {
          return aliases;
        }
      };

      // For ptr : &w T1 and each loan : T2, if T1 == T2 then add the projection
      // to the loan (so as to make the alias more precise), otherwise leave it alone
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

    // Get every place that explicitly appears within the MIR body
    let mut finder = FindPlaces {
      tcx,
      body,
      def_id,
      places: Vec::new(),
    };
    finder.visit_body(body);

    let mut all_places = finder.places.into_iter().collect::<HashSet<_>>();

    // Add every place that appears in a loan set
    all_places.extend(loans.into_iter().flatten());

    // For every place p = *q, add q
    let all_pointers = all_places
      .iter()
      .map(|place| utils::pointers_in_place(*place, tcx))
      .flatten()
      .collect::<Vec<_>>();
    all_places.extend(all_pointers);

    // Compute aliases for each (normalized) place
    let normalized_places = Rc::new(RefCell::new(NormalizedPlaces::new(tcx, def_id)));
    let all_aliases = all_places
      .iter()
      .map(|place| {
        let norm_place = normalized_places.borrow_mut().normalize(*place);
        // can't compute aliases w/ normalized place b/c that has regions erased
        (norm_place, compute_aliases(*place))
      })
      .collect::<HashMap<_, _>>();

    // Include aliases in the place domain
    all_places.extend(all_aliases.values().flatten().copied());

    debug!("Places: {:?}", {
      let mut v = all_places.iter().collect::<Vec<_>>();
      v.sort();
      v
    });
    info!("Place domain size: {}", all_places.len());

    (
      Rc::new(PlaceDomain::new(all_places, normalized_places)),
      all_aliases,
    )
  }

  fn compute_loans(
    body_with_facts: &BodyWithBorrowckFacts<'tcx>,
    region_to_pointers: HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
  ) -> IndexVec<RegionVid, HashSet<Place<'tcx>>> {
    // Get the graph of which regions outlive which other ones
    let region_ancestors =
      Self::compute_region_ancestors(body_with_facts, &region_to_pointers, tcx, def_id);

    // Initialize the loan set where loan['a] = {*x} if x: &'a T
    let mut loans = IndexVec::from_elem_n(HashSet::default(), region_ancestors.len());
    for (region, places) in region_to_pointers.iter() {
      for (sub_place, _) in places {
        loans[*region].insert(tcx.mk_place_deref(*sub_place));
      }
    }

    // Given expressions e = &'a p, add p to loan['a]
    let mut gather_borrows = GatherBorrows::default();
    gather_borrows.visit_body(&body_with_facts.body);
    for (region, _, place) in gather_borrows.borrows.into_iter() {
      loans[region].insert(place);
    }

    // Propagate all loans where if 'a : 'b, then add loan['a] to loan['b].
    // Iterate to fixpoint.
    loop {
      let mut changed = false;
      let prev_loans = loans.clone();
      for (region, region_loans) in loans.iter_enumerated_mut() {
        let outlives_loans = region_ancestors[region]
          .iter()
          .map(|ancestor| prev_loans[ancestor].iter().copied())
          .flatten()
          .collect::<HashSet<_>>();

        let orig_len = region_loans.len();
        region_loans.extend(&outlives_loans);
        changed |= orig_len != region_loans.len();
      }

      if !changed {
        break;
      }
    }

    loans
  }

  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    block_timer!("aliases");
    let body = &body_with_facts.body;

    // Get a mapping of regions -> references with that region,
    // i.e. 'a -> {p | p : &'a T}
    let all_locals = body.local_decls().indices();
    let all_pointers = all_locals
      .map(|local| {
        let place = utils::local_to_place(local, tcx);
        utils::interior_pointers(place, tcx, body, def_id)
      })
      .flatten();
    let mut region_to_pointers: HashMap<_, Vec<_>> = HashMap::default();
    for (region, places) in all_pointers {
      region_to_pointers.entry(region).or_default().extend(places);
    }

    // Use outlives-constraints to get the loan set for each region
    let loans = Self::compute_loans(body_with_facts, region_to_pointers, tcx, def_id);
    debug!("Loans: {:?}", {
      let mut v = loans.iter_enumerated().collect::<Vec<_>>();
      v.sort_by_key(|(r, _)| *r);
      v
    });

    // Convert loan sets for regions to alias sets for places by specializing
    // loans with projections
    let (place_domain, all_aliases) = Self::compute_place_domain(tcx, body, def_id, &loans);

    // Use alias sets to build derived metadata like the conflicts (#) relation
    let (aliases, deps, subs, supers) =
      Self::compute_conflicts(&place_domain, tcx, body, all_aliases);

    Aliases {
      place_domain,
      aliases,
      deps,
      subs,
      supers,
    }
  }

  pub fn conflicts(&self, place: impl ToIndex<Place<'tcx>>) -> PlaceSet<'tcx> {
    self
      .aliases
      .row(place)
      .map(|alias| {
        let subs = self.subs.row(alias);
        let supers = self.supers.row(alias);
        subs.chain(supers)
      })
      .flatten()
      .copied()
      .collect_indices(self.place_domain.clone())
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
      let regions_with_place = region_to_pointers
        .iter()
        // find other regions that contain a loan matching any type in places
        .filter(|(other_region, other_places)| {
          *region != **other_region
            && places.iter().any(|(place, _)| {
              other_places
                .iter()
                .any(|(other_place, _)| same_ty(*place, *other_place))
            })
        });

      // add 'a : 'b and 'b : 'a to ensure the lifetimes are considered equal
      regions_with_place
        .map(|(other_region, _)| [(*region, *other_region), (*other_region, *region)])
        .flatten()
        .collect::<Vec<_>>()
    })
    .flatten()
    .collect::<Vec<_>>()
}
