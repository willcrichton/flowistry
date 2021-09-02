use crate::core::{aliases::Aliases, control_dependencies::ControlDependencies};
use log::debug;
use polonius_engine::AllFacts;
use rustc_hir::BodyId;
use rustc_middle::{mir::Body, ty::TyCtxt};
use rustc_mir::{
  consumers::RustcFacts,
  dataflow::{Analysis, Results},
};

pub use dataflow::{FlowAnalysis, FlowDomain};
pub use dependencies::{compute_dependency_ranges, Direction};

mod dataflow;
mod dependencies;

pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body: &'a Body<'tcx>,
  facts: &'a AllFacts<RustcFacts>,
) -> Results<'tcx, FlowAnalysis<'a, 'tcx>> {
  let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
  let aliases = Aliases::build(tcx, def_id, body, facts.subset_base.clone());

  let control_dependencies = ControlDependencies::build(body.clone());
  debug!("Control dependencies: {:?}", control_dependencies);

  FlowAnalysis::new(tcx, def_id, body, aliases, control_dependencies)
    .into_engine(tcx, body)
    .iterate_to_fixpoint()
}
