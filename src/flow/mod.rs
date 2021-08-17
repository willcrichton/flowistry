use anyhow::Result;
use rustc_hir::{itemlikevisit::ItemLikeVisitor, BodyId};
use rustc_middle::ty::TyCtxt;
use rustc_mir::dataflow::Analysis;
use rustc_span::Span;

use crate::core::{
  aliases::Aliases,
  analysis::{FlowistryAnalysis, FlowistryOutput},
  control_dependencies::ControlDependencies,
  extensions::MutabilityMode,
  utils::qpath_to_span,
};

mod dataflow;

#[derive(Debug)]
pub struct FlowOutput;

impl FlowistryOutput for FlowOutput {
  fn empty() -> Self {
    FlowOutput
  }

  fn merge(&mut self, other: Self) {}
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
    let borrowck_result = tcx.mir_borrowck(local_def_id);

    let body = &borrowck_result.intermediates.body;
    let outlives_constraints = &borrowck_result.intermediates.outlives_constraints;
    let constraint_sccs = &borrowck_result.intermediates.constraint_sccs;

    let aliases = Aliases::build(
      &MutabilityMode::DistinguishMut,
      tcx,
      body,
      outlives_constraints,
      constraint_sccs,
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
