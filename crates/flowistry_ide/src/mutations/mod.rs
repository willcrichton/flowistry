use anyhow::{Context, Result};
use flowistry::{
  mir::{aliases::Aliases, borrowck_facts::get_body_with_borrowck_facts, utils::SpanExt},
  source_map::{self, EnclosingHirSpans, MirSpannedPlace},
};
use log::debug;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use self::find_mutations::find_mutations;
use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{ranges_from_spans, Range},
};

mod find_mutations;

struct MutationAnalysis {
  range: Range,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct MutationOutput {
  pub ranges: Vec<Range>,
  pub selected_spans: Vec<Range>,
  pub body_span: Range,
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
    let spanner = source_map::Spanner::new(tcx, body_id, body);
    let targets = spanner.span_to_places(self.range.to_span(source_map)?);

    let MirSpannedPlace {
      place: selected_place,
      ..
    } = targets
      .first()
      .context("Selection could not be mapped to a place.")?;
    debug!("selected_place {selected_place:?}");

    let body_span = Range::from_span(tcx.hir().body(body_id).value.span, source_map)?;
    let selected_spans =
      ranges_from_spans(targets.iter().map(|mir_span| mir_span.span), source_map)?;

    let mutated_locations =
      find_mutations(tcx, body, def_id.to_def_id(), *selected_place, aliases);
    let mutated_spans = mutated_locations
      .into_iter()
      .flat_map(|location| spanner.location_to_spans(location, EnclosingHirSpans::Full));
    let output_spans = Span::merge_overlaps(mutated_spans.collect());
    let ranges = ranges_from_spans(output_spans.into_iter(), source_map)?;

    Ok(MutationOutput {
      body_span,
      selected_spans,
      ranges,
    })
  }
}

pub fn find(range: Range, compiler_args: &[String]) -> FlowistryResult<MutationOutput> {
  MutationAnalysis { range }.run(compiler_args)
}
