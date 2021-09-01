use crate::core::{
  aliases::Aliases,
  analysis::{FlowistryAnalysis, FlowistryOutput},
  control_dependencies::ControlDependencies,
  extensions::MutabilityMode,
  utils,
};
use anyhow::Result;
use log::debug;
use polonius_engine::AllFacts;
use rustc_hir::BodyId;
use rustc_middle::{mir::Body, ty::TyCtxt};
use rustc_mir::{
  consumers::RustcFacts,
  dataflow::{Analysis, Results},
};
use rustc_span::Span;

pub use dataflow::{FlowAnalysis, FlowDomain};

mod dataflow;
pub mod dependencies;

pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  facts: &'a AllFacts<RustcFacts>,
) -> Results<'tcx, FlowAnalysis<'a, 'tcx>> {
  let aliases = Aliases::build(
    &MutabilityMode::DistinguishMut,
    tcx,
    body,
    facts.subset_base.clone(),
    &vec![],
  );

  let control_dependencies = ControlDependencies::build(body.clone());
  debug!("Control dependencies: {:?}", control_dependencies);

  FlowAnalysis::new(tcx, body, aliases, control_dependencies)
    .into_engine(tcx, body)
    .iterate_to_fixpoint()
}

#[derive(Debug)]
pub struct FlowOutput;

impl FlowistryOutput for FlowOutput {
  fn empty() -> Self {
    FlowOutput
  }

  fn merge(&mut self, _other: Self) {}
}

struct FlowHarness {
  qpath: String,
}

impl FlowistryAnalysis for FlowHarness {
  type Output = FlowOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![utils::qpath_to_span(tcx, self.qpath.clone())?])
  }

  fn analyze_function(&mut self, _tcx: TyCtxt, _body_id: BodyId) -> Result<Self::Output> {
    Ok(FlowOutput)
  }
}

pub fn flow(qpath: String, compiler_args: &[String]) -> Result<FlowOutput> {
  FlowHarness { qpath }.run(compiler_args)
}
