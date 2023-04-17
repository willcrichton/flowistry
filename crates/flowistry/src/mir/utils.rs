//! A potpourri of utilities for working with the MIR, primarily exposed as extension traits.

use std::{
  io::Write,
  path::Path,
  process::{Command, Stdio},
};

use anyhow::{bail, Result};
use either::Either;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_graphviz as dot;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{GenericArgKind, RegionKind, RegionVid, Ty, TyCtxt},
};
use rustc_mir_dataflow::{fmt::DebugWithContext, graphviz, Analysis, Results};
use rustc_utils::{BodyExt, OperandExt, PlaceExt};

use crate::{
  extensions::{is_extension_active, MutabilityMode},
  indexed::impls::LocationOrArg,
};

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
pub fn arg_places<'tcx>(args: &[Operand<'tcx>]) -> Vec<(usize, Place<'tcx>)> {
  args
    .iter()
    .enumerate()
    .filter_map(|(i, arg)| arg.as_place().map(move |place| (i, place)))
    .collect::<Vec<_>>()
}

#[derive(Default)]
pub struct PlaceCollector<'tcx>(pub Vec<Place<'tcx>>);

impl<'tcx> Visitor<'tcx> for PlaceCollector<'tcx> {
  fn visit_place(
    &mut self,
    place: &Place<'tcx>,
    _context: PlaceContext,
    _location: Location,
  ) {
    self.0.push(*place);
  }
}

pub fn run_dot(path: &Path, buf: Vec<u8>) -> Result<()> {
  let mut p = Command::new("dot")
    .args(["-Tpdf", "-o", &path.display().to_string()])
    .stdin(Stdio::piped())
    .spawn()?;

  p.stdin.as_mut().unwrap().write_all(&buf)?;
  let status = p.wait()?;

  if !status.success() {
    bail!("dot for {} failed", path.display())
  };

  Ok(())
}

pub fn dump_results<'tcx, A>(
  body: &Body<'tcx>,
  results: &Results<'tcx, A>,
  _def_id: DefId,
  _tcx: TyCtxt<'tcx>,
) -> Result<()>
where
  A: Analysis<'tcx>,
  A::Domain: DebugWithContext<A>,
{
  let graphviz =
    graphviz::Formatter::new(body, results, graphviz::OutputStyle::AfterOnly);
  let mut buf = Vec::new();
  dot::render(&graphviz, &mut buf)?;

  let output_dir = Path::new("target");
  // let fname = tcx.def_path_debug_str(def_id);
  let fname = "results";
  let output_path = output_dir.join(format!("{fname}.pdf"));

  run_dot(&output_path, buf)
}

pub fn location_to_string(location: LocationOrArg, body: &Body<'_>) -> String {
  match location {
    LocationOrArg::Arg(local) => format!("{local:?}"),
    LocationOrArg::Location(location) => match body.stmt_at(location) {
      Either::Left(stmt) => format!("{:?}", stmt.kind),
      Either::Right(terminator) => format!("{:?}", terminator.kind),
    },
  }
}

// This is a temporary hack to reduce spurious dependencies in generators
// arising from async functions. The issue is that the &mut std::task::Context
// variable interferes with both the modular approximation and the alias analysis.
// As a patch up, we ignore subset constraints arising from lifetimes appearing
// in the Context type, as well as ignore any place of type Context in function calls.
//
// See test: async_two_await
pub struct AsyncHack<'a, 'tcx> {
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
