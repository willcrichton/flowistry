use anyhow::Result;
use flowistry::{
  mir::{
    aliases::Aliases, borrowck_facts::get_body_with_borrowck_facts,
    mutations::find_mutations,
  },
  source_map::{self, location_to_spans},
};
use log::debug;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{ranges_from_spans, Range},
};

struct MutationAnalysis {
  range: Range,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct MutationOutput {
  pub ranges: Vec<Range>,
  pub selected_spans: Vec<Range>,
  pub body_span: Range,
}

impl MutationOutput {
  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

impl FlowistryOutput for MutationOutput {
  fn merge(&mut self, other: MutationOutput) {
    self.ranges.extend(other.ranges);
    self.body_span = other.body_span;
    self.selected_spans.extend(other.selected_spans);
  }
}

impl FlowistryAnalysis for MutationAnalysis {
  type Output = MutationOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.range.to_span(tcx.sess.source_map())?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;
    let aliases = Aliases::build(tcx, def_id.to_def_id(), body_with_facts);

    let source_map = tcx.sess.source_map();
    let (selected_place, _, selected_span) =
      match source_map::span_to_place(body, self.range.to_span(source_map)?) {
        Some(t) => t,
        None => {
          return Ok(MutationOutput::default());
        }
      };
    debug!("selected_place {:?}", selected_place);

    let spanner = source_map::HirSpanner::new(tcx, body_id);

    let body_span = Range::from_span(tcx.hir().body(body_id).value.span, source_map)?;
    let selected_spans = ranges_from_spans([selected_span].into_iter(), source_map)?;

    let mutation_locations =
      find_mutations(tcx, body, def_id.to_def_id(), selected_place, aliases);
    let mutation_spans = mutation_locations
      .into_iter()
      .map(|location| location_to_spans(location, body, &spanner, source_map))
      .flatten();
    let ranges = ranges_from_spans(mutation_spans, source_map)?;

    Ok(MutationOutput {
      body_span,
      selected_spans,
      ranges,
      ..Default::default()
    })
  }
}

pub fn find(range: Range, compiler_args: &[String]) -> FlowistryResult<MutationOutput> {
  MutationAnalysis { range }.run(compiler_args)
}
