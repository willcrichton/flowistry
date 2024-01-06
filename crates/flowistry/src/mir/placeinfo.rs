//! Utilities for analyzing places: children, aliases, etc.

use std::{ops::ControlFlow, rc::Rc};

use indexical::ToIndex;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::*,
  ty::{
    Region, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TypeSuperVisitable, TypeVisitor,
  },
};
use rustc_utils::{
  block_timer,
  cache::{Cache, CopyCache},
  mir::{
    location_or_arg::{
      index::{LocationOrArgDomain, LocationOrArgIndex},
      LocationOrArg,
    },
    place::UNKNOWN_REGION,
  },
  BodyExt, MutabilityExt, PlaceExt,
};

use super::{aliases::Aliases, utils::PlaceSet};
use crate::extensions::{is_extension_active, MutabilityMode};

/// Utilities for analyzing places: children, aliases, etc.
pub struct PlaceInfo<'tcx> {
  pub(crate) tcx: TyCtxt<'tcx>,
  pub(crate) body: &'tcx Body<'tcx>,
  pub(crate) def_id: DefId,
  location_domain: Rc<LocationOrArgDomain>,

  // Core computed data structure
  aliases: Aliases<'tcx>,

  // Caching for derived analysis
  normalized_cache: CopyCache<Place<'tcx>, Place<'tcx>>,
  aliases_cache: Cache<Place<'tcx>, PlaceSet<'tcx>>,
  conflicts_cache: Cache<Place<'tcx>, PlaceSet<'tcx>>,
  reachable_cache: Cache<(Place<'tcx>, Mutability), PlaceSet<'tcx>>,
}

impl<'tcx> PlaceInfo<'tcx> {
  fn build_location_arg_domain(body: &Body) -> Rc<LocationOrArgDomain> {
    let all_locations = body.all_locations().map(LocationOrArg::Location);
    let all_locals = body.args_iter().map(LocationOrArg::Arg);
    let domain = all_locations.chain(all_locals).collect();
    Rc::new(LocationOrArgDomain::new(domain))
  }

  /// Computes all the metadata about places used within the infoflow analysis.
  pub fn build(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    block_timer!("aliases");
    let body = &body_with_facts.body;
    let location_domain = Self::build_location_arg_domain(body);
    let aliases = Aliases::build(tcx, def_id, body_with_facts);

    PlaceInfo {
      aliases,
      tcx,
      body,
      def_id,
      location_domain,
      aliases_cache: Cache::default(),
      normalized_cache: CopyCache::default(),
      conflicts_cache: Cache::default(),
      reachable_cache: Cache::default(),
    }
  }

  /// Normalizes a place via [`PlaceExt::normalize`] (cached).
  ///
  /// See the `PlaceExt` documentation for details on how normalization works.
  pub fn normalize(&self, place: Place<'tcx>) -> Place<'tcx> {
    self
      .normalized_cache
      .get(place, |place| place.normalize(self.tcx, self.def_id))
  }

  /// Computes the aliases of a place (cached).
  ///
  /// For example, if `x = &y`, then `*x` aliases `y`.
  /// Note that an alias is NOT guaranteed to be of the same type as `place`!
  pub fn aliases(&self, place: Place<'tcx>) -> &PlaceSet<'tcx> {
    // note: important that aliases are computed on the unnormalized place
    // which contains region information
    self
      .aliases_cache
      .get(self.normalize(place), move |_| self.aliases.aliases(place))
  }

  /// Returns all reachable fields of `place` without going through references.
  ///
  /// For example, if `x = (0, 1)` then `children(x) = {x, x.0, x.1}`.
  pub fn children(&self, place: Place<'tcx>) -> PlaceSet<'tcx> {
    PlaceSet::from_iter(place.interior_places(self.tcx, self.body, self.def_id))
  }

  /// Returns all places that conflict with `place`, i.e. that a mutation to `place`
  /// would also be a mutation to the conflicting place.
  ///
  /// For example, if `x = ((0, 1), 2)` then `conflicts(x.0) = {x, x.0, x.0.0, x.0.1}`, but not `x.1`.
  pub fn conflicts(&self, place: Place<'tcx>) -> &PlaceSet<'tcx> {
    self.conflicts_cache.get(place, |place| {
      let children = self.children(place);
      let parents = place
        .iter_projections()
        .take_while(|(_, elem)| !matches!(elem, PlaceElem::Deref))
        .map(|(place_ref, _)| Place::from_ref(place_ref, self.tcx));
      children.into_iter().chain(parents).collect()
    })
  }

  /// Returns all [direct](PlaceExt::is_direct) places that are reachable from `place`
  /// and can be used at the provided level of [`Mutability`] (cached).
  ///
  /// For example, if `x = 0` and `y = (0, &x)`, then `reachable_values(y, Mutability::Not)`
  /// is `{y, x}`. With `Mutability::Mut`, then the output is `{y}` (no `x`).
  pub fn reachable_values(
    &self,
    place: Place<'tcx>,
    mutability: Mutability,
  ) -> &PlaceSet<'tcx> {
    self.reachable_cache.get((place, mutability), |_| {
      let ty = place.ty(self.body.local_decls(), self.tcx).ty;
      let loans = self.collect_loans(ty, mutability);
      loans
        .into_iter()
        .chain([place])
        .filter(|place| {
          if let Some((place, _)) = place.refs_in_projection().last() {
            let ty = place.ty(self.body.local_decls(), self.tcx).ty;
            if ty.is_box() || ty.is_unsafe_ptr() {
              return true;
            }
          }
          place.is_direct(self.body)
        })
        .collect()
    })
  }

  fn collect_loans(&self, ty: Ty<'tcx>, mutability: Mutability) -> PlaceSet<'tcx> {
    let mut collector = LoanCollector {
      aliases: &self.aliases,
      unknown_region: Region::new_var(self.tcx, UNKNOWN_REGION),
      target_mutability: mutability,
      stack: vec![],
      loans: PlaceSet::default(),
    };
    collector.visit_ty(ty);
    collector.loans
  }

  /// Returns all [direct](PlaceExt::is_direct) places reachable from arguments
  /// to the current body.
  pub fn all_args(&self) -> impl Iterator<Item = (Place<'tcx>, LocationOrArgIndex)> + '_ {
    self.body.args_iter().flat_map(|local| {
      let location = local.to_index(&self.location_domain);
      let place = Place::from_local(local, self.tcx);
      let ptrs = place
        .interior_pointers(self.tcx, self.body, self.def_id)
        .into_values()
        .flat_map(|ptrs| {
          ptrs
            .into_iter()
            .filter(|(ptr, _)| ptr.projection.len() <= 2)
            .map(|(ptr, _)| self.tcx.mk_place_deref(ptr))
        });
      ptrs
        .chain([place])
        .flat_map(|place| place.interior_places(self.tcx, self.body, self.def_id))
        .map(move |place| (place, location))
    })
  }

  /// Returns the [`LocationOrArgDomain`] for the current body.
  pub fn location_domain(&self) -> &Rc<LocationOrArgDomain> {
    &self.location_domain
  }
}

// TODO: this visitor shares some structure with the PlaceCollector in mir utils.
// Can we consolidate these?
struct LoanCollector<'a, 'tcx> {
  aliases: &'a Aliases<'tcx>,
  unknown_region: Region<'tcx>,
  target_mutability: Mutability,
  stack: Vec<Mutability>,
  loans: PlaceSet<'tcx>,
}

impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for LoanCollector<'_, 'tcx> {
  type BreakTy = ();

  fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
    match ty.kind() {
      TyKind::Ref(_, _, mutability) => {
        self.stack.push(*mutability);
        ty.super_visit_with(self);
        self.stack.pop();
        return ControlFlow::Break(());
      }
      _ if ty.is_box() || ty.is_unsafe_ptr() => {
        self.visit_region(self.unknown_region);
      }
      _ => {}
    };

    ty.super_visit_with(self);
    ControlFlow::Continue(())
  }

  fn visit_region(&mut self, region: Region<'tcx>) -> ControlFlow<Self::BreakTy> {
    let region = match region.kind() {
      RegionKind::ReVar(region) => region,
      RegionKind::ReStatic => RegionVid::from_usize(0),
      // TODO: do we need to handle bound regions?
      // e.g. shows up with closures, for<'a> ...
      RegionKind::ReErased | RegionKind::ReLateBound(_, _) => {
        return ControlFlow::Continue(());
      }
      _ => unreachable!("{region:?}"),
    };
    if let Some(loans) = self.aliases.loans.get(&region) {
      let under_immut_ref = self.stack.iter().any(|m| *m == Mutability::Not);
      let ignore_mut =
        is_extension_active(|mode| mode.mutability_mode == MutabilityMode::IgnoreMut);
      self
        .loans
        .extend(loans.iter().filter_map(|(place, mutability)| {
          if ignore_mut {
            return Some(place);
          }
          let loan_mutability = if under_immut_ref {
            Mutability::Not
          } else {
            *mutability
          };
          self
            .target_mutability
            .is_permissive_as(loan_mutability)
            .then_some(place)
        }))
    }

    ControlFlow::Continue(())
  }
}

#[cfg(test)]
mod test {
  use rustc_utils::{
    hashset,
    test_utils::{compare_sets, Placer},
  };

  use super::*;
  use crate::test_utils;

  fn placeinfo_harness(
    input: &str,
    f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, &Body<'tcx>, PlaceInfo<'tcx>) + Send,
  ) {
    test_utils::compile_body(input, |tcx, body_id, body_with_facts| {
      let body = &body_with_facts.body;
      let def_id = tcx.hir().body_owner_def_id(body_id);
      let place_info = PlaceInfo::build(tcx, def_id.to_def_id(), body_with_facts);

      f(tcx, body, place_info)
    });
  }

  #[test]
  fn test_placeinfo_basic() {
    let input = r#"
fn main() {
  let a = 0;
  let mut b = 1;
  let c = ((0, &a), &mut b);
  let d = 0;
  let e = &d;
  let f = &e;
}
    "#;
    placeinfo_harness(input, |tcx, body, place_info| {
      let p = Placer::new(tcx, body);
      let c = p.local("c");
      compare_sets(place_info.children(c.mk()), hashset! {
        c.mk(),
        c.field(0).mk(),
        c.field(0).field(0).mk(),
        c.field(0).field(1).mk(),
        c.field(1).mk(),
      });

      compare_sets(place_info.conflicts(c.field(0).mk()), &hashset! {
        c.mk(),
        c.field(0).mk(),
        c.field(0).field(0).mk(),
        c.field(0).field(1).mk(),
        // c.field(1) not part of the set
      });

      // a and b are reachable at immut-level
      compare_sets(
        place_info.reachable_values(c.mk(), Mutability::Not),
        &hashset! {
          c.mk(),
          p.local("a").mk(),
          p.local("b").mk()
        },
      );

      // only b is reachable at mut-level
      compare_sets(
        place_info.reachable_values(c.mk(), Mutability::Mut),
        &hashset! {
          c.mk(),
          p.local("b").mk()
        },
      );

      // handles transitive references
      compare_sets(
        place_info.reachable_values(p.local("f").mk(), Mutability::Not),
        &hashset! {
          p.local("f").mk(),
          p.local("e").mk(),
          p.local("d").mk()
        },
      )
    });
  }
}
