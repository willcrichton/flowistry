use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils::SpanExt},
  source_map,
};
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{ranges_from_spans, Range},
};

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
  pub selected_spans: Vec<Range>,
  pub body_span: Range,
}

impl FlowistryOutput for SliceOutput {
  fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges);
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
    self.body_span = other.body_span;
    self.selected_spans.extend(other.selected_spans);
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
    let body_span = tcx.hir().body(body_id).value.span;
    let targets =
      source_map::span_to_places(tcx, body, body_span, self.range.to_span(source_map)?);
    debug!("Targets: {targets:?}");

    let spanner = source_map::HirSpanner::new(tcx, body_id);
    let deps = infoflow::compute_dependency_spans(
      results,
      targets
        .iter()
        .map(|(place, loc, _)| (*place, *loc))
        .collect(),
      self.direction,
      &spanner,
    );

    let body_span = Range::from_span(tcx.hir().body(body_id).value.span, source_map)?;
    let selected_spans =
      ranges_from_spans(targets.iter().map(|(_, _, sp)| *sp), source_map)?;
    let output_spans = Span::merge_overlaps(deps.into_iter().flatten().collect());
    let ranges = ranges_from_spans(output_spans.into_iter(), source_map)?;
    debug!("found {} ranges in slice", ranges.len());

    Ok(SliceOutput {
      body_span,
      selected_spans,
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
