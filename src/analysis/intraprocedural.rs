use super::points_to::PointsToAnalysis;
use super::relevance::{RelevanceAnalysis, RelevanceDomain, SliceSet};
use crate::config::{Range, CONFIG};
use anyhow::{Context, Result};
use log::debug;
use rustc_graphviz as dot;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::{fmt::DebugWithContext, graphviz, Analysis, Results, ResultsVisitor};
use rustc_mir::util::write_mir_fn;
use rustc_span::Span;
use std::{collections::HashSet, fs::File, io::Write, process::Command};

struct CollectResults<'a, 'tcx> {
  body: &'a Body<'tcx>,
  relevant_spans: Vec<Span>,
}

impl<'a, 'mir, 'tcx> ResultsVisitor<'mir, 'tcx> for CollectResults<'a, 'tcx> {
  type FlowState = RelevanceDomain;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    _statement: &'mir mir::Statement<'tcx>,
    location: Location,
  ) {
    if state.statement_relevant {
      let source_info = self.body.source_info(location);
      self.relevant_spans.push(source_info.span);
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    _terminator: &'mir mir::Terminator<'tcx>,
    location: Location,
  ) {
    if state.statement_relevant {
      let source_info = self.body.source_info(location);
      self.relevant_spans.push(source_info.span);
    }
  }
}

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: SliceSet,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_place(&mut self, _place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if !self.slice_span.contains(span) {
      return;
    }

    self.slice_set.insert(location);
  }
}

fn dump_results<'tcx, A>(path: &str, body: &Body<'tcx>, results: &Results<'tcx, A>) -> Result<()>
where
  A: Analysis<'tcx>,
  A::Domain: DebugWithContext<A>,
{
  let graphviz = graphviz::Formatter::new(body, &results, graphviz::OutputStyle::AfterOnly);
  let mut buf = Vec::new();
  dot::render(&graphviz, &mut buf)?;
  let mut file = File::create("/tmp/graph.dot")?;
  file.write_all(&buf)?;
  Command::new("dot")
    .args(&["-Tpng", "/tmp/graph.dot", "-o", path])
    .status()?;
  Ok(())
}

pub struct SliceOutput(Vec<Range>);

impl SliceOutput {
  pub fn new() -> Self {
    SliceOutput(Vec::new())
  }

  pub fn merge(&mut self, other: SliceOutput) {
    self.0.extend(other.0.into_iter());
  }

  pub fn ranges(&self) -> &Vec<Range> {
    &self.0
  }
}

pub fn analyze_function(tcx: TyCtxt, body_id: &rustc_hir::BodyId) -> Result<SliceOutput> {
  CONFIG.get(|config| {
    let config = config.context("Missing config")?;

    let local_def_id = body_id.hir_id.owner;
    let body = tcx.optimized_mir(local_def_id);

    debug!("MIR");
    let mut buffer = Vec::new();
    write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut buffer)?;
    debug!("{}", String::from_utf8_lossy(&buffer));
    debug!("============");

    // let borrowck_result = tcx.mir_borrowck(local_def_id);

    let source_map = tcx.sess.source_map();
    let mut finder = FindInitialSliceSet {
      slice_span: config.range.to_span(source_map),
      slice_set: HashSet::new(),
      body,
    };
    finder.visit_body(body);
    debug!("Initial slice set: {:?}", finder.slice_set);

    let points_to_results = PointsToAnalysis { tcx, body }
      .into_engine(tcx, body)
      .iterate_to_fixpoint();

    let relevance_results =
      RelevanceAnalysis::new(finder.slice_set, tcx, body_id, body, &points_to_results)
        .into_engine(tcx, body)
        .iterate_to_fixpoint();

    if config.debug {
      dump_results("target/points_to.png", body, &points_to_results)?;
      dump_results("target/relevance.png", body, &relevance_results)?;
    }

    let mut visitor = CollectResults {
      body,
      relevant_spans: vec![],
    };
    relevance_results.visit_reachable_with(body, &mut visitor);

    let ranges = visitor
      .relevant_spans
      .into_iter()
      .map(|span| Range::from_span(span, source_map))
      .collect();

    Ok(SliceOutput(ranges))
  })
}
