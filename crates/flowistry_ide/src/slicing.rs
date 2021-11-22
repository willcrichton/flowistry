use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{ranges_from_spans, Range},
};
use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils},
  source_map::HirSpanner,
};
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

struct ForwardSliceAnalysis {
  direction: Direction,
  range: Range,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct SliceOutput {
  pub ranges: Vec<Range>,
  pub num_instructions: usize,
  pub num_relevant_instructions: usize,
  pub mutated_inputs: HashSet<usize>,
  pub relevant_inputs: HashSet<usize>,
  pub sliced_spans: Vec<Range>,
  pub body_span: Range,
}

impl SliceOutput {
  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

impl FlowistryOutput for SliceOutput {
  fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges);
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
    self.body_span = other.body_span;
    self.sliced_spans.extend(other.sliced_spans);
  }
}

impl FlowistryAnalysis for ForwardSliceAnalysis {
  type Output = SliceOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.range.to_span(tcx.sess.source_map())?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;

    let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

    let source_map = tcx.sess.source_map();
    let (sliced_places, sliced_spans) =
      utils::span_to_places(body, self.range.to_span(source_map)?);
    debug!("sliced_places {:?}", sliced_places);

    let spanner = HirSpanner::new(tcx, body_id);
    let deps = infoflow::compute_dependency_spans(results, sliced_places, self.direction, &spanner);

    let body_span = Range::from_span(tcx.hir().body(body_id).value.span, source_map)?;
    let sliced_spans = ranges_from_spans(sliced_spans.into_iter(), source_map)?;
    let ranges = ranges_from_spans(deps.into_iter().flatten(), source_map)?;
    debug!("found {} ranges in slice", ranges.len());

    Ok(SliceOutput {
      body_span,
      sliced_spans,
      ranges,
      ..Default::default()
    })
  }
}

pub fn slice(
  direction: Direction,
  range: Range,
  compiler_args: &[String],
) -> FlowistryResult<SliceOutput> {
  ForwardSliceAnalysis { direction, range }.run(compiler_args)
}
