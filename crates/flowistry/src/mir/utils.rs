//! A potpourri of utilities for working with the MIR, primarily exposed as extension traits.

use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::*,
  ty::{GenericArgKind, RegionKind, RegionVid, Ty, TyCtxt},
};
use rustc_span::source_map::Spanned;
use rustc_utils::{BodyExt, OperandExt, PlaceExt};

use crate::extensions::{is_extension_active, MutabilityMode};

/// An unordered collections of MIR [`Place`]s.
///
/// *Note:* this used to be implemented as an [`IndexSet`](indexical::IndexSet),
/// but in practice it was very hard to determine up-front a fixed domain of
/// [`Place`]s that was not "every possible place in the body".
pub type PlaceSet<'tcx> = HashSet<Place<'tcx>>;

/// Given the arguments to a function, returns all projections of the arguments that are mutable pointers.
pub fn arg_mut_ptrs<'tcx>(
  args: &[(usize, Place<'tcx>)],
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  def_id: DefId,
) -> Vec<(usize, Place<'tcx>)> {
  let ignore_mut =
    is_extension_active(|mode| mode.mutability_mode == MutabilityMode::IgnoreMut);
  args
    .iter()
    .flat_map(|(i, place)| {
      place
        .interior_pointers(tcx, body, def_id)
        .into_iter()
        .flat_map(|(_, places)| {
          places
            .into_iter()
            .filter_map(|(place, mutability)| match mutability {
              Mutability::Mut => Some(place),
              Mutability::Not => ignore_mut.then_some(place),
            })
        })
        .map(move |place| (*i, tcx.mk_place_deref(place)))
    })
    .collect::<Vec<_>>()
}

/// Given the arguments to a function, returns all places in the arguments.
pub fn arg_places<'tcx>(args: &[Spanned<Operand<'tcx>>]) -> Vec<(usize, Place<'tcx>)> {
  args
    .iter()
    .enumerate()
    .filter_map(|(i, arg)| arg.node.as_place().map(move |place| (i, place)))
    .collect::<Vec<_>>()
}

/// A hack to temporary hack to reduce spurious dependencies in generators
/// arising from async functions.
///
/// The issue is that the `&mut std::task::Context` variable interferes with both
/// the modular approximation and the alias analysis. As a patch up, we ignore subset
/// constraints arising from lifetimes appearing in the Context type, as well as ignore
/// any place of type Context in function calls.
///
/// See test: async_two_await
pub(crate) struct AsyncHack<'a, 'tcx> {
  context_ty: Option<Ty<'tcx>>,
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> AsyncHack<'a, 'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>, body: &'a Body<'tcx>, def_id: DefId) -> Self {
    let context_ty = body.async_context(tcx, def_id);
    AsyncHack {
      context_ty,
      tcx,
      body,
    }
  }

  pub fn ignore_regions(&self) -> HashSet<RegionVid> {
    match self.context_ty {
      Some(context_ty) => context_ty
        .walk()
        .filter_map(|part| match part.unpack() {
          GenericArgKind::Lifetime(r) => match r.kind() {
            RegionKind::ReVar(rv) => Some(rv),
            _ => None,
          },
          _ => None,
        })
        .collect::<HashSet<_>>(),
      None => HashSet::default(),
    }
  }

  pub fn ignore_place(&self, place: Place<'tcx>) -> bool {
    match self.context_ty {
      Some(context_ty) => {
        self
          .tcx
          .erase_regions(place.ty(&self.body.local_decls, self.tcx).ty)
          == self.tcx.erase_regions(context_ty)
      }
      None => false,
    }
  }
}
