#![allow(warnings)]

use std::path::Path;

use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{run_dot, SpanExt},
  },
  source_map::{self, EnclosingHirSpans},
};
use petgraph::dot::{Config as DotConfig, Dot};
use rayon::prelude::*;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{FunctionIdentifier, Range},
};

#[derive(Debug, Clone, Encodable, Default)]
pub struct Slice {
  range: Range,
  slice: Vec<Range>,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct FocusOutput {
  slices: Vec<Slice>,
  body_span: Range,
}

impl FlowistryOutput for FocusOutput {
  fn merge(&mut self, other: Self) {
    self.slices.extend(other.slices);
    self.body_span = other.body_span;
  }
}

pub struct FocusAnalysis {
  id: FunctionIdentifier,
}

impl FlowistryAnalysis for FocusAnalysis {
  type Output = FocusOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.id.to_span(tcx)?])
  }
  fn analyze_function(
    &mut self,
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
  ) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;
    let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

    let source_map = tcx.sess.source_map();
    let spanner = source_map::Spanner::new(tcx, body_id, body);

    let targets = spanner
      .mir_spans
      .iter()
      .map(|mir_span| (mir_span.place, mir_span.location))
      .collect::<Vec<_>>();
    let backward_deps = infoflow::compute_dependency_spans(
      results,
      targets.clone(),
      Direction::Backward,
      &spanner,
    );
    let forward_deps =
      infoflow::compute_dependency_spans(results, targets, Direction::Forward, &spanner);

    let slices = spanner
      .mir_spans
      .into_iter()
      .zip(backward_deps)
      .zip(forward_deps)
      .filter_map(|((mir_span, mut backward), forward)| {
        backward.extend(forward);
        let spans = Span::merge_overlaps(backward);

        let range = Range::from_span(mir_span.span, source_map).ok()?;
        let slice = spans
          .into_iter()
          .filter_map(|span| Some(Range::from_span(span, source_map).ok()?))
          .collect::<Vec<_>>();
        Some(Slice { range, slice })
      })
      .collect::<Vec<_>>();

    let body_span = Range::from_span(spanner.body_span, source_map)?;

    Ok(FocusOutput { slices, body_span })
  }
}

pub fn focus(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<FocusOutput> {
  FocusAnalysis { id }.run(compiler_args)
}
