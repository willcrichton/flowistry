use log::debug;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::def_id::DefId;
use rustc_index::bit_set::SparseBitMatrix;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS},
};

use crate::{
  block_timer,
  cached::Cached,
  extensions::{is_extension_active, PointerMode},
  indexed::impls::PlaceSet,
  mir::utils::{self, PlaceExt},
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

    // See PlaceCollector for where this matters
    if let Rvalue::Aggregate(box AggregateKind::Adt(def_id, idx, substs, _, _), _) =
      rvalue
    {
      let adt_def = self.tcx.adt_def(*def_id);
      let variant = &adt_def.variants[*idx];
      let places = variant.fields.iter().enumerate().map(|(i, field)| {
        let mut projection = place.projection.to_vec();
        projection.push(ProjectionElem::Field(
          Field::from_usize(i),
          field.ty(self.tcx, substs),
        ));
        Place::make(place.local, &projection, self.tcx)
      });
      self.places.extend(places);
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

type LoanMap<'tcx> = HashMap<RegionVid, HashSet<Place<'tcx>>>;

pub struct Aliases<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  def_id: DefId,

  loans: LoanMap<'tcx>,

  normalized_cache: Cached<Place<'tcx>, Place<'tcx>>,
  aliases_cache: Cached<Place<'tcx>, PlaceSet<'tcx>>,
  conflicts_cache: Cached<Place<'tcx>, PlaceSet<'tcx>>,
  reachable_cache: Cached<Place<'tcx>, PlaceSet<'tcx>>,
  // // For each place p, {p' | exists execution s.t. eval(p) # eval(p')}
  // pub aliases: HashMap<Place<'tcx>, HashSet<Place<'tcx>>>
}

impl Aliases<'a, 'tcx> {
  fn compute_loans(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> LoanMap<'tcx> {
    let body = &body_with_facts.body;

    let static_region = RegionVid::from_usize(0);
    let subset_base = &body_with_facts.input_facts.subset_base;
    let all_regions = subset_base.iter().copied().flat_map(|(a, b, _)| [a, b]);
    let max_region = all_regions.clone().max().unwrap_or(static_region);

    let mut subset = SparseBitMatrix::new(max_region.as_usize() + 1);

    // subset('a, 'b) :- subset_base('a, 'b, _).
    for (a, b, _) in subset_base {
      subset.insert(*a, *b);
    }

    // subset('static, 'a).
    for a in all_regions {
      subset.insert(static_region, a);
    }

    if is_extension_active(|mode| mode.pointer_mode == PointerMode::Conservative) {
      // for all p1 : &'a T, p2: &'b T: subset('a, 'b).
      let mut region_to_pointers: HashMap<_, Vec<_>> = HashMap::default();
      for local in body.local_decls().indices() {
        for (k, vs) in Place::from_local(local, tcx).interior_pointers(tcx, body, def_id)
        {
          region_to_pointers.entry(k).or_default().extend(vs);
        }
      }

      let constraints = generate_conservative_constraints(
        tcx,
        &body_with_facts.body,
        &region_to_pointers,
      );

      for (a, b) in constraints {
        subset.insert(a, b);
      }
    }

    let mut contains: LoanMap<'tcx> = HashMap::default();
    let mut definite: HashMap<RegionVid, (Ty<'tcx>, Vec<PlaceElem<'tcx>>)> =
      HashMap::default();

    // For all e = &'a x.q in body:
    //   contains('a, p).
    //   If p = p^[* p']: definite('a, ty(p'), p'^[])
    //   Else:            definite('a, ty(p),  p^[]).
    let mut gather_borrows = GatherBorrows::default();
    gather_borrows.visit_body(&body_with_facts.body);
    for (region, _, place) in gather_borrows.borrows {
      contains.entry(region).or_default().insert(place);

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

    // For all args p : &'a T where 'a is abstract: contains('a, *p).
    for local in body.args_iter() {
      let place = Place::from_local(local, tcx);
      for (region, ptrs) in place.interior_pointers(tcx, body, def_id) {
        for (ptr, _) in ptrs {
          contains
            .entry(region)
            .or_default()
            .insert(tcx.mk_place_deref(ptr));
        }
      }
    }
    debug!("Initial contains: {contains:#?}");
    debug!("Definite: {definite:#?}");

    // Subset implies containment: p ∈ 'a ∧ 'a ⊆ 'b ⇒ p ∈ 'b
    // i.e. contains('b, p) :- contains('a, p), subset('a, 'b).
    //
    // If 'a is from a borrow expression &'a proj[*p'], then we add proj to all inherited aliases.
    // See interprocedural_field_independence for an example where this matters.
    // But we only do this if:
    //   * !subset('b, 'a) since otherwise projections would be added infinitely.
    //   * if p' : &T, then p : T since otherwise proj[p] is not well-typed.
    //
    // Note that it's theoretically more efficient to compute the transitive closure of `subset`
    // and then do the pass below in one step rather than to a fixpoint. But this negates the added
    // precision from propagating projections. For example, in the program:
    //   a = &'0 mut (0, 0)
    //   b = &'1 mut a.0
    //   c = &'2 mut *b
    //   *c = 1;
    // then '0 :> '1 :> '2. By propagating projections, then '1 = {a.0}. However if we see '0 :> '2
    // to insert contains('0) into contains('2), then contains('2) = {a, a.0} which defeats the purpose!
    // Then *c = 1 is considered to be a mutation to anything within a.
    loop {
      let mut changed = false;
      for a in subset.rows() {
        for b in subset.iter(a) {
          let cyclic = subset.contains(b, a);

          if let Some(places) = contains.get(&a).cloned() {
            for p in places {
              let p_ty = p.ty(body.local_decls(), tcx).ty;
              let p_proj = match definite.get(&b) {
                Some((ty, proj)) if !cyclic && TyS::same_type(ty, p_ty) => {
                  let mut full_proj = p.projection.to_vec();
                  full_proj.extend(proj);
                  Place::make(p.local, tcx.intern_place_elems(&full_proj), tcx)
                }
                _ => p,
              };

              changed |= contains.entry(b).or_default().insert(p_proj);
            }
          }
        }
      }

      if !changed {
        break;
      }
    }

    contains
  }
  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    block_timer!("aliases");
    let body = &body_with_facts.body;

    let loans = Self::compute_loans(tcx, def_id, body_with_facts);
    debug!("Loans: {loans:?}");

    Aliases {
      loans,
      tcx,
      body,
      def_id,
      aliases_cache: Cached::default(),
      normalized_cache: Cached::default(),
      conflicts_cache: Cached::default(),
      reachable_cache: Cached::default(),
    }
  }

  pub fn normalize(&self, place: Place<'tcx>) -> Place<'tcx> {
    *self
      .normalized_cache
      .get(place, |place| place.normalize(self.tcx, self.def_id))
  }

  pub fn aliases(&self, place: Place<'tcx>) -> &PlaceSet<'tcx> {
    // note: important that aliases are computed on the unnormalized place
    // which contains region information
    self.aliases_cache.get(self.normalize(place), move |_| {
      let mut aliases = HashSet::default();
      aliases.insert(place);

      // Places with no derefs, or derefs from arguments, have no aliases
      if place.is_direct(self.body) {
        return aliases;
      }

      // place = after[*ptr]
      let (ptr, after) = *place.refs_in_projection().last().unwrap();

      // ptr : &'region orig_ty
      let (region, orig_ty) = match ptr.ty(self.body.local_decls(), self.tcx).ty.kind() {
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
      let region_loans = self
        .loans
        .get(&region)
        .map(|loans| loans.iter())
        .into_iter()
        .flatten();
      let region_aliases = region_loans.map(|loan| {
        let loan_ty = loan.ty(self.body.local_decls(), self.tcx).ty;
        if TyS::same_type(orig_ty, loan_ty) {
          let mut projection = loan.projection.to_vec();
          projection.extend(after.iter().copied());
          Place::make(loan.local, &projection, self.tcx)
        } else {
          *loan
        }
      });

      aliases.extend(region_aliases);
      log::trace!("Aliases for place {place:?} are {aliases:?}");
      aliases
    })
  }

  pub fn children(&self, place: Place<'tcx>) -> PlaceSet<'tcx> {
    HashSet::from_iter(place.interior_places(self.tcx, self.body, self.def_id))
  }

  pub fn conflicts(&self, place: Place<'tcx>) -> &PlaceSet<'tcx> {
    self.conflicts_cache.get(place, |place| {
      self
        .aliases(place)
        .iter()
        .flat_map(|alias| {
          let children = self.children(*alias);
          let parents = alias
            .iter_projections()
            .take_while(|(_, elem)| !matches!(elem, PlaceElem::Deref))
            .map(|(place_ref, _)| Place::from_ref(place_ref, self.tcx));
          children.into_iter().chain(parents)
        })
        .collect()
    })
  }

  pub fn reachable_values(&self, place: Place<'tcx>) -> &PlaceSet<'tcx> {
    self.reachable_cache.get(place, |place| {
      let interior_pointer_places = place
        .interior_pointers(self.tcx, self.body, self.def_id)
        .into_values()
        .flat_map(|v| v.into_iter().map(|(place, _)| place));

      interior_pointer_places
        .flat_map(|place| self.aliases(self.tcx.mk_place_deref(place)).iter().copied())
        .chain([place])
        .collect()
    })
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
    .flat_map(|(region, places)| {
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
        .flat_map(|(other_region, _)| {
          [(*region, *other_region), (*other_region, *region)]
        })
        .collect::<Vec<_>>()
    })
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
      assert!(aliases.aliases(y_deref).contains(&x));
    })
  }
}
