use crate::core::{
  aliases::Aliases,
  analysis::{FlowistryAnalysis, FlowistryOutput},
  control_dependencies::ControlDependencies,
  extensions::MutabilityMode,
  utils::qpath_to_span,
};
use anyhow::Result;
use rustc_hir::BodyId;
use rustc_middle::ty::{TyCtxt, WithOptConstParam};
use rustc_mir::{consumers::get_body_with_borrowck_facts, dataflow::Analysis};
use rustc_span::Span;

mod dataflow;

#[derive(Debug)]
pub struct FlowOutput;

impl FlowistryOutput for FlowOutput {
  fn empty() -> Self {
    FlowOutput
  }

  fn merge(&mut self, _other: Self) {}
}

struct FlowAnalysis {
  qpath: String,
}

impl FlowistryAnalysis for FlowAnalysis {
  type Output = FlowOutput;

  fn locations(&self, tcx: TyCtxt) -> Vec<Span> {
    vec![qpath_to_span(tcx, self.qpath.clone()).unwrap()]
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts =
      get_body_with_borrowck_facts(tcx, WithOptConstParam::unknown(local_def_id));
    let body = &body_with_facts.body;

    let aliases = Aliases::build(
      &MutabilityMode::DistinguishMut,
      tcx,
      body,
      body_with_facts.input_facts.outlives,
      &vec![],
    );

    let control_dependencies = ControlDependencies::build(body.clone());

    let output = dataflow::FlowAnalysis::new(tcx, body, &aliases, &control_dependencies)
      .into_engine(tcx, body)
      .iterate_to_fixpoint();

    Ok(FlowOutput)
  }
}

pub fn flow(qpath: String, compiler_args: &[String]) -> Result<FlowOutput> {
  FlowAnalysis { qpath }.run(compiler_args)
}
