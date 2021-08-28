use crate::core::{
  aliases::Aliases,
  analysis::{FlowistryAnalysis, FlowistryOutput},
  control_dependencies::ControlDependencies,
  extensions::MutabilityMode,
  utils::qpath_to_span,
};
use anyhow::Result;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_mir::{
  consumers::BodyWithBorrowckFacts,
  dataflow::{Analysis, Results},
};
use rustc_span::Span;

pub use dataflow::{FlowAnalysis, FlowDomain};

mod dataflow;
pub mod dependencies;

pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> Results<'tcx, FlowAnalysis<'a, 'tcx>> {
  let body = &body_with_facts.body;
  let aliases = Aliases::build(
    &MutabilityMode::DistinguishMut,
    tcx,
    body,
    body_with_facts.input_facts.subset_base.clone(),
    &vec![],
  );

  let control_dependencies = ControlDependencies::build(body.clone());

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

  fn locations(&self, tcx: TyCtxt) -> Vec<Span> {
    vec![qpath_to_span(tcx, self.qpath.clone()).unwrap()]
  }

  fn analyze_function(&mut self, _tcx: TyCtxt, _body_id: BodyId) -> Result<Self::Output> {
    Ok(FlowOutput)
  }
}

pub fn flow(qpath: String, compiler_args: &[String]) -> Result<FlowOutput> {
  FlowHarness { qpath }.run(compiler_args)
}
