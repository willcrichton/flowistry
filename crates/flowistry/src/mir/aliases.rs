use std::{cell::RefCell, hash::Hash, rc::Rc};

use datafrog::{Iteration, Relation};
use log::{debug, info};
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{RegionKind, RegionVid, TyCtxt, TyKind, TyS},
};

use crate::{
  block_timer,
  extensions::{is_extension_active, PointerMode},
  indexed::{
    impls::{NormalizedPlaces, PlaceDomain, PlaceIndex, PlaceSet},
    IndexMatrix, IndexSet, IndexSetIteratorExt, IndexedDomain, RefSet, ToIndex,
  },
  mir::utils::{self, PlaceExt, PlaceRelation},
};

#[derive(Default)]
struct GatherBorrows<'tcx> {
  borrows: Vec<(RegionVid, BorrowKind, Place<'tcx>)>,
}

impl Visitor<'tcx> for GatherBorrows<'tcx> {
  fn visit_assign(
    &mut self,
    _place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    _location: Location,
  ) {
    if let Rvalue::Ref(region, kind, borrowed_place) = rvalue {
      let region_vid = match region {
        RegionKind::ReVar(region_vid) => *region_vid,
        _ => unreachable!(),
      };
      self.borrows.push((region_vid, *kind, *borrowed_place));
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
    self.places.push(Place::from_local(local, self.tcx));
  }

  fn visit_place(
    &mut self,
    place: &Place<'tcx>,
    _context: PlaceContext,
    _location: Location,
  ) {
    self.places.push(*place);
  }

  fn visit_assign(
    &mut self,
    place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
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
        let arg_mut_ptrs =
          utils::arg_mut_ptrs(&arg_places, self.tcx, self.body, self.def_id);
        self
          .places
          .extend(arg_mut_ptrs.into_iter().map(|(_, place)| place));
      }

      _ => {}
    }
  }
}

pub struct Aliases<'tcx> {
  /// For each place p, {p' | exists execution s.t. eval(p) # eval(p')}
  pub aliases: IndexMatrix<Place<'tcx>, Place<'tcx>>,

  /// For each place p, {p' | p' = proj[p]}
  pub children: IndexMatrix<Place<'tcx>, Place<'tcx>>,

  /// For each place p, {p' | p' # p}
  pub conflicts: IndexMatrix<Place<'tcx>, Place<'tcx>>,

  /// Every place used during analysis (but not every reachable place!)
  pub place_domain: Rc<PlaceDomain<'tcx>>,
}

rustc_index::newtype_index! {
  pub struct ConstraintSccIndex {
      DEBUG_FORMAT = "cs{}"
  }
}

type LoanMap<'tcx> = HashMap<RegionVid, HashSet<Place<'tcx>>>;

fn group_pairs<K: Hash + Eq, V: Hash + Eq>(
  pairs: impl Iterator<Item = (K, V)>,
) -> HashMap<K, HashSet<V>> {
  let mut map: HashMap<_, HashSet<_>> = HashMap::default();
  for (k, v) in pairs {
    map.entry(k).or_default().insert(v);
  }
  map
}

impl Aliases<'tcx> {
  fn compute_loans(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> LoanMap<'tcx> {
    let body = &body_with_facts.body;
    let mut iteration = Iteration::new();

    let subset = {
      let mut subset = HashSet::default();

      // subset('a, 'b) :- subset_base('a, 'b, _).
      {
        let subset_base = &body_with_facts.input_facts.subset_base;
        subset.extend(subset_base.iter().copied().map(|(o1, o2, _)| (o1, o2)));
      }

      // subset('static, 'a).
      {
        let static_region = RegionVid::from_usize(0);
        let static_outlives = subset
          .iter()
          .map(|(o1, o2)| [o1, o2])
          .flatten()
          .map(|o| (static_region, *o))
          .collect::<Vec<_>>();
        subset.extend(static_outlives);
      }

      if is_extension_active(|mode| mode.pointer_mode == PointerMode::Conservative) {
        // for all p1 : &'a T, p2: &'b T: subset('a, 'b).
        let mut region_to_pointers: HashMap<_, Vec<_>> = HashMap::default();
        for local in body.local_decls().indices() {
          for (k, vs) in
            Place::from_local(local, tcx).interior_pointers(tcx, body, def_id)
          {
            region_to_pointers.entry(k).or_default().extend(vs);
          }
        }

        subset.extend(
          generate_conservative_constraints(
            tcx,
            &body_with_facts.body,
            &region_to_pointers,
          )
          .into_iter(),
        );
      }

      Relation::from_iter(subset)
    };

    let contains = iteration.variable::<(RegionVid, Place<'static>)>("contains");
    let mut definite = HashMap::default();
    let mk_tuple =
      |region: RegionVid, place: Place<'tcx>| -> (RegionVid, Place<'static>) {
        (region, unsafe { std::mem::transmute(place) })
      };

    // For all e = &'a p in body: contains('a, p).
    {
      let mut gather_borrows = GatherBorrows::default();
      gather_borrows.visit_body(&body_with_facts.body);

      for (region, _, place) in gather_borrows.borrows {
        contains.extend([mk_tuple(region, place)]);

        let (ty, projection) = match place.refs_in_projection().last() {
          Some((ptr, proj)) => (
            ptr.ty(body.local_decls(), tcx).ty.peel_refs(),
            proj.to_vec(),
          ),
          None => (
            body.local_decls()[place.local].ty,
            place.projection.to_vec(),
          ),
        };
        definite.insert(region, (ty, projection));
      }
    }

    // For all args p : &'a T where 'a is abstract: contains('a, *p).
    {
      let arg_ptrs = |arg: Local| {
        let place = Place::from_local(arg, tcx);
        place
          .interior_pointers(tcx, body, def_id)
          .into_iter()
          .map(|(region, places)| {
            places
              .into_iter()
              .map(move |(place, _)| mk_tuple(region, tcx.mk_place_deref(place)))
          })
          .flatten()
      };
      contains.extend(body.args_iter().map(arg_ptrs).flatten());
    }

    // reachable is the transitive closure of subset
    let reachable = {
      let mut iteration = Iteration::new();
      let reachable = iteration.variable("reachable");
      reachable.extend(subset.as_ref().iter().copied());
      let reachable_rev = iteration.variable_indistinct("reachable_rev");
      while iteration.changed() {
        reachable_rev.from_map(&reachable, |&(o1, o2)| (o2, o1));
        reachable.from_join(&reachable_rev, &reachable, |_, a, c| (*a, *c));
      }
      group_pairs(reachable.complete().iter().copied())
    };

    while iteration.changed() {
      // Subset implies containment: p ∈ 'a ∧ 'a ⊆ 'b ⇒ p ∈ 'b
      // i.e. contains('b, p) :- contains('a, p), subset('a, 'b).
      //
      // If 'a is from a borrow expression &'a proj[*p'], then we add proj to all inherited aliases.
      // See interprocedural_field_independence for an example where this matters.
      // But we only do this if:
      //   * !subset('b, 'a) since otherwise projections would be added infinitely.
      //   * if p' : &T, then p : T since otherwise proj[p] is not well-typed.
      contains.from_join(&contains, &subset, |a, p, b| {
        let is_reachable = reachable.get(b).map(|set| set.contains(a)).unwrap_or(false);
        let p_ty = p.ty(body.local_decls(), tcx).ty;
        let p_proj = match definite.get(b) {
          Some((ty, proj)) if !is_reachable && TyS::same_type(ty, p_ty) => {
            let mut full_proj = p.projection.to_vec();
            full_proj.extend(proj);
            Place::make(p.local, tcx.intern_place_elems(&full_proj), tcx)
          }
          _ => *p,
        };
        mk_tuple(*b, p_proj)
      });
    }

    group_pairs(contains.complete().iter().copied())
  }

  fn compute_all_places(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
    loans: &LoanMap<'tcx>,
  ) -> HashSet<Place<'tcx>> {
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
    all_places.extend(loans.values().flatten().cloned());

    // For every place p = *q, add q
    let all_pointers = all_places
      .iter()
      .map(|place| {
        place
          .refs_in_projection()
          .into_iter()
          .map(|(ptr, _)| Place::from_ref(ptr, tcx))
      })
      .flatten()
      .collect::<Vec<_>>();
    all_places.extend(all_pointers);

    let all_locals = body.local_decls().indices();
    all_places.extend(
      all_locals
        .map(|local| {
          let place = Place::from_local(local, tcx);
          place
            .interior_pointers(tcx, body, def_id)
            .into_values()
            .map(|places| {
              places
                .into_iter()
                .map(|(p, _)| vec![p, tcx.mk_place_deref(p)].into_iter())
                .flatten()
            })
            .flatten()
        })
        .flatten(),
    );

    debug!("Places: {:?}", {
      let mut v = all_places.iter().collect::<Vec<_>>();
      v.sort();
      v
    });
    info!("Place domain size: {}", all_places.len());

    all_places
  }

  fn aliases_from_loans(
    place: Place<'tcx>,
    loans: &LoanMap<'tcx>,
    body: &Body<'tcx>,
    tcx: TyCtxt<'tcx>,
  ) -> HashSet<Place<'tcx>> {
    let mut aliases = HashSet::default();
    aliases.insert(place);

    // Places with no derefs, or derefs from arguments, have no aliases
    if place.is_direct(body) {
      return aliases;
    }

    // place = after[*ptr]
    let (ptr, after) = *place.refs_in_projection().last().unwrap();

    // ptr : &'region orig_ty
    let (region, orig_ty) = match ptr.ty(body.local_decls(), tcx).ty.kind() {
      TyKind::Ref(RegionKind::ReVar(region), ty, _) => (*region, ty),
      // ty => unreachable!("{:?} / {:?}", place, ty),
      // TODO: how to deal with box?
      _ => {
        return aliases;
      }
    };

    // For each p ∈ loans('region),
    //   if p : orig_ty then add: after[p]
    //   else add: p
    let region_loans = loans
      .get(&region)
      .map(|loans| loans.iter())
      .into_iter()
      .flatten();
    let region_aliases = region_loans.map(|loan| {
      let loan_ty = loan.ty(body.local_decls(), tcx).ty;
      if TyS::same_type(orig_ty, loan_ty) {
        let mut projection = loan.projection.to_vec();
        projection.extend(after.iter().copied());
        Place::make(loan.local, &projection, tcx)
      } else {
        *loan
      }
    });

    aliases.extend(region_aliases);
    aliases
  }

  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    block_timer!("aliases");
    let body = &body_with_facts.body;

    let loans = Self::compute_loans(tcx, def_id, body_with_facts);
    debug!("Loans: {:#?}", loans);

    let mut all_places = Self::compute_all_places(tcx, body, def_id, &loans);

    let normalized_places = Rc::new(RefCell::new(NormalizedPlaces::new(tcx, def_id)));
    let all_aliases = all_places
      .iter()
      .map(|place| {
        (
          normalized_places.borrow_mut().normalize(*place),
          Self::aliases_from_loans(*place, &loans, body, tcx),
        )
      })
      .collect::<HashMap<_, _>>();
    debug!("Aliases: {:#?}", all_aliases);

    all_places.extend(all_aliases.values().map(|s| s.iter().copied()).flatten());

    let place_domain = Rc::new(PlaceDomain::new(all_places, normalized_places));

    Self::compute_conflicts(place_domain, body, all_aliases)
  }

  fn compute_conflicts(
    place_domain: Rc<PlaceDomain<'tcx>>,
    body: &Body<'tcx>,
    aliases_map: HashMap<Place<'tcx>, HashSet<Place<'tcx>>>,
  ) -> Aliases<'tcx> {
    let new_mtx = || IndexMatrix::new(place_domain.clone(), place_domain.clone());
    let mut aliases = new_mtx();
    let mut children = new_mtx();
    let mut conflicts = new_mtx();

    for (place, aliases_hashset) in aliases_map.into_iter() {
      let aliases_indexset = aliases_hashset
        .into_iter()
        .collect_indices(place_domain.clone());
      aliases.union_into_row(place, &aliases_indexset);
    }

    for place in place_domain.as_vec().iter() {
      let (subs, supers): (Vec<_>, Vec<_>) = place_domain
        .as_vec()
        .iter_enumerated()
        .filter_map(move |(idx, other_place)| {
          let relation = PlaceRelation::of(*other_place, *place);
          (relation.overlaps() && other_place.is_direct(body))
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

      let subs = to_set(subs);
      let supers = to_set(supers);

      children.union_into_row(place, &subs);

      conflicts.union_into_row(place, &subs);
      conflicts.union_into_row(place, &supers);
    }

    for place in place_domain.as_vec().iter() {
      for alias in aliases.row(place) {
        children.union_rows(alias, place);
        conflicts.union_rows(alias, place);
      }
    }

    Aliases {
      aliases,
      children,
      conflicts,
      place_domain,
    }
  }

  pub fn conflicts(
    &self,
    place: impl ToIndex<Place<'tcx>>,
  ) -> PlaceSet<'tcx, RefSet<'_, Place<'tcx>>> {
    self.conflicts.row_set(place).unwrap()
  }

  pub fn reachable_values(
    &self,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: DefId,
    place: Place<'tcx>,
  ) -> IndexSet<Place<'tcx>> {
    let interior_pointer_places = place
      .interior_pointers(tcx, body, def_id)
      .into_values()
      .map(|v| v.into_iter().map(|(place, _)| place))
      .flatten();

    interior_pointer_places
      .map(|place| self.aliases.row(tcx.mk_place_deref(place)).copied())
      .flatten()
      .chain(vec![place])
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

#[cfg(test)]
mod test {
  use super::*;
  use crate::{
    mir::utils::{BodyExt, PlaceExt},
    test_utils,
  };

  #[test]
  fn test_sccs() {
    let input = r#"
    fn main() {
      let mut x = 1;
      let y = &mut x;
      *y;
    }
    "#;
    test_utils::compile_body(input, |tcx, body_id, body_with_facts| {
      let body = &body_with_facts.body;
      let def_id = tcx.hir().body_owner_def_id(body_id);
      let aliases = Aliases::build(tcx, def_id.to_def_id(), body_with_facts);
      let name_map = body
        .debug_info_name_map()
        .into_iter()
        .map(|(k, v)| (v.to_string(), k))
        .collect::<HashMap<_, _>>();

      let x = Place::from_local(name_map["x"], tcx);
      let y = Place::from_local(name_map["y"], tcx);
      let y_deref = tcx.mk_place_deref(y);
      assert!(aliases.aliases.row_set(y_deref).unwrap().contains(x));
    })
  }
}
