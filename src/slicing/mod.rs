use crate::{
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    config::Range,
    utils,
  },
  flow::{self, Direction},
};
use anyhow::Result;
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;

use rustc_span::Span;
use serde::Serialize;

struct ForwardSliceAnalysis {
  direction: Direction,
  range: Range,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SliceOutput {
  pub ranges: Vec<Range>,
  pub num_instructions: usize,
  pub num_relevant_instructions: usize,
  pub mutated_inputs: HashSet<usize>,
  pub relevant_inputs: HashSet<usize>,
  pub body_span: Range,
}

impl SliceOutput {
  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

impl FlowistryOutput for SliceOutput {
  fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges.into_iter());
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
    self.body_span = other.body_span;
  }
}

impl FlowistryAnalysis for ForwardSliceAnalysis {
  type Output = SliceOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.range.to_span(tcx.sess.source_map())?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let body_with_facts = utils::get_body_with_borrowck_facts(tcx, body_id);
    let body = &body_with_facts.body;
    debug!("{}", utils::mir_to_string(tcx, body)?);

    let results = flow::compute_flow(tcx, body_id, &body_with_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/flow.png", body, &results)?;
    }

    let source_map = tcx.sess.source_map();
    let sliced_places = utils::span_to_places(body, self.range.to_span(source_map)?);
    debug!("sliced_places {:?}", sliced_places);

    let spanner = utils::HirSpanner::new(tcx, body_id);

    let deps = flow::compute_dependency_ranges(&results, sliced_places, self.direction, &spanner);

    let mut output = SliceOutput::default();
    output.body_span = Range::from_span(tcx.hir().body(body_id).value.span, source_map)?;
    output.ranges = deps.into_iter().map(|v| v.into_iter()).flatten().collect();
    Ok(output)
  }
}

pub fn slice(direction: Direction, range: Range, compiler_args: &[String]) -> Result<SliceOutput> {
  ForwardSliceAnalysis { direction, range }.run(compiler_args)
}
