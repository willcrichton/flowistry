//! Alias analysis to determine the points-to set of a reference.

use std::{hash::Hash, time::Instant};

use log::{debug, info};
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::{
  fx::{FxHashMap as HashMap, FxHashSet as HashSet},
  graph::{iterate::reverse_post_order, scc::Sccs, vec_graph::VecGraph},
  intern::Interned,
};
use rustc_hir::def_id::DefId;
use rustc_index::{
  bit_set::{HybridBitSet, SparseBitMatrix},
  IndexVec,
};
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::{Region, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TypeAndMut},
};
use rustc_utils::{mir::place::UNKNOWN_REGION, timer::elapsed, PlaceExt};

use crate::{
  extensions::{is_extension_active, PointerMode},
  mir::utils::{AsyncHack, PlaceSet},
};

type BorrowckLocationIndex =
  <rustc_borrowck::consumers::RustcFacts as crate::polonius_engine::FactTypes>::Point;

#[derive(Default)]
struct GatherBorrows<'tcx> {
  borrows: Vec<(RegionVid, BorrowKind, Place<'tcx>)>,
}

macro_rules! region_pat {
  ($name:ident) => {
    Region(Interned(RegionKind::ReVar($name), _))
  };
}

impl<'tcx> Visitor<'tcx> for GatherBorrows<'tcx> {
  fn visit_assign(
    &mut self,
    _place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    _location: Location,
  ) {
    if let Rvalue::Ref(region_pat!(region), kind, borrowed_place) = rvalue {
      self.borrows.push((*region, *kind, *borrowed_place));
    }
  }
}

type LoanSet<'tcx> = HashSet<(Place<'tcx>, Mutability)>;
type LoanMap<'tcx> = HashMap<RegionVid, LoanSet<'tcx>>;

/// Data structure for computing and storing aliases.
pub struct Aliases<'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'tcx Body<'tcx>,
  pub(super) loans: LoanMap<'tcx>,
}

rustc_index::newtype_index! {
  #[debug_format = "rs{}"]
  struct RegionSccIndex {}
}

impl<'tcx> Aliases<'tcx> {
  /// Runs the alias analysis on a given `body_with_facts`.
  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    let loans = Self::compute_loans(tcx, def_id, body_with_facts, |_, _, _| true);
    Aliases {
      tcx,
      body: &body_with_facts.body,
      loans,
    }
  }

  /// Alternative constructor if you need to filter out certain borrowck facts.
  ///
  /// Just use [`Aliases::build`] unless you know what you're doing.
  pub fn build_with_fact_selection(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
    selector: impl Fn(RegionVid, RegionVid, BorrowckLocationIndex) -> bool,
  ) -> Self {
    let loans = Self::compute_loans(tcx, def_id, body_with_facts, selector);
    Aliases {
      tcx,
      body: &body_with_facts.body,
      loans,
    }
  }

  fn compute_loans(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
    constraint_selector: impl Fn(RegionVid, RegionVid, BorrowckLocationIndex) -> bool,
  ) -> LoanMap<'tcx> {
    let start = Instant::now();
    let body = &body_with_facts.body;
    let static_region = RegionVid::from_usize(0);
    let subset_base = &body_with_facts
      .input_facts
      .as_ref()
      .unwrap()
      .subset_base
      .iter()
      .cloned()
      .filter(|(r1, r2, i)| constraint_selector(*r1, *r2, *i))
      .collect::<Vec<_>>();

    let all_pointers = body
      .local_decls()
      .indices()
      .flat_map(|local| {
        Place::from_local(local, tcx).interior_pointers(tcx, body, def_id)
      })
      .collect::<Vec<_>>();
    let max_region = all_pointers
      .iter()
      .map(|(region, _)| *region)
      .chain(subset_base.iter().flat_map(|(r1, r2, _)| [*r1, *r2]))
      .filter(|r| *r != UNKNOWN_REGION)
      .max()
      .unwrap_or(static_region);
    let num_regions = max_region.as_usize() + 1;
    let all_regions = (0 .. num_regions).map(RegionVid::from_usize);

    let mut subset = SparseBitMatrix::new(num_regions);

    let async_hack = AsyncHack::new(tcx, body, def_id);
    let ignore_regions = async_hack.ignore_regions();

    // subset('a, 'b) :- subset_base('a, 'b, _).
    for (a, b, _) in subset_base {
      if ignore_regions.contains(a) || ignore_regions.contains(b) {
        continue;
      }
      subset.insert(*a, *b);
    }

    // subset('static, 'a).
    for a in all_regions.clone() {
      subset.insert(static_region, a);
    }

    if is_extension_active(|mode| mode.pointer_mode == PointerMode::Conservative) {
      // for all p1 : &'a T, p2: &'b T: subset('a, 'b).
      let mut region_to_pointers: HashMap<_, Vec<_>> = HashMap::default();
      for (region, places) in &all_pointers {
        if *region != UNKNOWN_REGION {
          region_to_pointers
            .entry(*region)
            .or_default()
            .extend(places);
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
    //   If p = p^[* p2]: definite('a, ty(p2), p2^[])
    //   Else:            definite('a, ty(p),  p^[]).
    let mut gather_borrows = GatherBorrows::default();
    gather_borrows.visit_body(&body_with_facts.body);
    for (region, kind, place) in gather_borrows.borrows {
      if place.is_direct(body) {
        contains
          .entry(region)
          .or_default()
          .insert((place, kind.to_mutbl_lossy()));
      }

      let def = match place.refs_in_projection().next() {
        Some((ptr, proj)) => {
          let ptr_ty = ptr.ty(body.local_decls(), tcx).ty;
          (ptr_ty.builtin_deref(true).unwrap().ty, proj.to_vec())
        }
        None => (
          body.local_decls()[place.local].ty,
          place.projection.to_vec(),
        ),
      };
      definite.insert(region, def);
    }

    // For all args p : &'a ω T where 'a is abstract: contains('a, *p, ω).
    for arg in body.args_iter() {
      for (region, places) in
        Place::from_local(arg, tcx).interior_pointers(tcx, body, def_id)
      {
        let region_contains = contains.entry(region).or_default();
        for (place, mutability) in places {
          // WARNING / TODO: this is a huge hack (that is conjoined w/ all_args).
          // Need a way to limit the number of possible pointers for functions with
          // many pointers in the input. This is almost certainly not sound, but hopefully
          // it works for most cases.
          if place.projection.len() <= 2 {
            region_contains.insert((tcx.mk_place_deref(place), mutability));
          }
        }
      }
    }

    // For all places p : *T or p : Box<T>: contains('UNK, *p, mut).
    let unk_contains = contains.entry(UNKNOWN_REGION).or_default();
    for (region, places) in &all_pointers {
      if *region == UNKNOWN_REGION {
        for (place, _) in places {
          unk_contains.insert((tcx.mk_place_deref(*place), Mutability::Mut));
        }
      }
    }

    info!(
      "Initial places in loan set: {}, total regions {}, definite regions: {}",
      contains.values().map(|set| set.len()).sum::<usize>(),
      contains.len(),
      definite.len()
    );

    debug!("Initial contains: {contains:#?}");
    debug!("Definite: {definite:#?}");

    // Compute a topological sort of the subset relation.
    let edge_pairs = subset
      .rows()
      .flat_map(|r1| subset.iter(r1).map(move |r2| (r1, r2)))
      .collect::<Vec<_>>();
    let subset_graph = VecGraph::new(num_regions, edge_pairs);
    let subset_sccs = Sccs::<RegionVid, RegionSccIndex>::new(&subset_graph);
    let mut scc_to_regions =
      IndexVec::from_elem_n(HybridBitSet::new_empty(num_regions), subset_sccs.num_sccs());
    for r in all_regions.clone() {
      let scc = subset_sccs.scc(r);
      scc_to_regions[scc].insert(r);
    }
    let scc_order = reverse_post_order(&subset_sccs, subset_sccs.scc(static_region));
    elapsed("relation construction", start);

    // Subset implies containment: l ∈ 'a ∧ 'a ⊆ 'b ⇒ l ∈ 'b
    // i.e. contains('b, l) :- contains('a, l), subset('a, 'b).
    //
    // contains('b, p2^[p], ω) :-
    //   contains('a, p, ω), subset('a, 'b),
    //   definite('b, T, p2^[]), !subset('b, 'a), p : T.
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
    //
    // Rather than iterating over the entire subset relation, we only do local fixpoints
    // within each strongly-connected component.
    let start = Instant::now();
    for r in all_regions {
      contains.entry(r).or_default();
    }
    for scc_idx in scc_order {
      loop {
        let mut changed = false;
        let scc = &scc_to_regions[scc_idx];
        for a in scc.iter() {
          for b in subset.iter(a) {
            if a == b {
              continue;
            }

            // SAFETY: a != b
            let a_contains =
              unsafe { &*(contains.get(&a).unwrap() as *const LoanSet<'tcx>) };
            let b_contains =
              unsafe { &mut *(contains.get_mut(&b).unwrap() as *mut LoanSet<'tcx>) };

            let cyclic = scc.contains(b);
            match definite.get(&b) {
              Some((ty, proj)) if !cyclic => {
                for (p, mutability) in a_contains.iter() {
                  let p_ty = p.ty(body.local_decls(), tcx).ty;
                  let p_proj = if *ty == p_ty {
                    let mut full_proj = p.projection.to_vec();
                    full_proj.extend(proj);
                    Place::make(p.local, tcx.mk_place_elems(&full_proj), tcx)
                  } else {
                    *p
                  };

                  changed |= b_contains.insert((p_proj, *mutability));
                }
              }
              _ => {
                let orig_len = b_contains.len();
                b_contains.extend(a_contains);
                changed |= b_contains.len() != orig_len;
              }
            }
          }
        }

        if !changed {
          break;
        }
      }
    }
    elapsed("fixpoint", start);

    info!(
      "Final places in loan set: {}",
      contains.values().map(|set| set.len()).sum::<usize>()
    );
    contains
  }

  /// Given a `place`, returns the set of direct places it could refer to.
  ///
  /// For example, in the program:
  /// ```
  /// let x = 1;
  /// let y = &x;
  /// ```
  ///
  /// The place `*y` (but NOT `y`) is an alias for `x`. Similarly, in the program:
  ///
  /// ```
  /// let v = vec![1, 2, 3];
  /// let n = &v[0];
  /// ```
  ///
  /// The place `*n` is an alias for `v` (even though they have different types!).
  pub fn aliases(&self, place: Place<'tcx>) -> PlaceSet<'tcx> {
    let mut aliases = HashSet::default();

    // Places with no derefs, or derefs from arguments, have no aliases
    if place.is_direct(self.body) {
      aliases.insert(place);
      return aliases;
    }

    // place = after[*ptr]
    let (ptr, after) = place.refs_in_projection().last().unwrap();

    // ptr : &'region orig_ty
    let ptr_ty = ptr.ty(self.body.local_decls(), self.tcx).ty;
    let (region, orig_ty) = match ptr_ty.kind() {
      _ if ptr_ty.is_box() => (UNKNOWN_REGION, ptr_ty.boxed_ty()),
      TyKind::RawPtr(TypeAndMut { ty, .. }) => (UNKNOWN_REGION, *ty),
      TyKind::Ref(Region(Interned(RegionKind::ReVar(region), _)), ty, _) => {
        (*region, *ty)
      }
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
    let region_aliases = region_loans.map(|(loan, _)| {
      let loan_ty = loan.ty(self.body.local_decls(), self.tcx).ty;
      if orig_ty == loan_ty {
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
  }
}

fn generate_conservative_constraints<'tcx>(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  region_to_pointers: &HashMap<RegionVid, Vec<(Place<'tcx>, Mutability)>>,
) -> Vec<(RegionVid, RegionVid)> {
  let get_ty = |p| tcx.mk_place_deref(p).ty(body.local_decls(), tcx).ty;
  let same_ty = |p1, p2| get_ty(p1) == get_ty(p2);

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
  use rustc_utils::{
    hashset,
    test_utils::{compare_sets, Placer},
  };

  use super::*;
  use crate::test_utils;

  fn alias_harness(
    input: &str,
    f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, &Body<'tcx>, Aliases<'tcx>) + Send,
  ) {
    test_utils::compile_body(input, |tcx, body_id, body_with_facts| {
      let body = &body_with_facts.body;
      let def_id = tcx.hir().body_owner_def_id(body_id);
      let aliases = Aliases::build(tcx, def_id.to_def_id(), body_with_facts);

      f(tcx, body, aliases)
    });
  }

  #[test]
  fn test_aliases_basic() {
    let input = r#"
    fn main() {
      fn foo<'a, 'b>(x: &'a i32, y: &'b i32) -> &'a i32 { x }

      let a = 1;
      let b = 2;
      let c = &a;
      let d = &b;
      let e = foo(c, d);      
    }
    "#;
    alias_harness(input, |tcx, body, aliases| {
      let p = Placer::new(tcx, body);
      let d_deref = p.local("d").deref().mk();
      let e_deref = p.local("e").deref().mk();

      // `*e` aliases only `a` (not `b`) because of the lifetime constraints on `foo`
      compare_sets(aliases.aliases(e_deref), hashset! { p.local("a").mk()});

      // `*e` aliases only `b` because nothing might relate it to `a`
      compare_sets(aliases.aliases(d_deref), hashset! { p.local("b").mk() });
    });
  }

  #[test]
  fn test_aliases_projection() {
    let input = r#"
fn main() {
  let a = vec![0];
  let b = a.get(0).unwrap();

  let c = (0, 0);
  let d = &c.1;
}
    "#;
    alias_harness(input, |tcx, body, aliases| {
      let p = Placer::new(tcx, body);
      let b_deref = p.local("b").deref().mk();
      let d_deref = p.local("d").deref().mk();

      // `*b` only aliases `a` because we don't have a projection for `a`
      compare_sets(aliases.aliases(b_deref), hashset! { p.local("a").mk() });

      // `*d` aliases `c.1` because we know the projection from the source
      compare_sets(
        aliases.aliases(d_deref),
        hashset! { p.local("c").field(1).mk() },
      );
    });
  }
}
