use crate::config::{Range, CONFIG};
use crate::points_to::{PointsToAnalysis, PlacePrim};
use crate::relevance::{RelevanceAnalysis, RelevanceDomain};
use anyhow::{Context, Result};
use rustc_graphviz as dot;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, graphviz, Analysis, Results, ResultsRefCursor,
  ResultsVisitor,
};
use rustc_mir::util::write_mir_fn;
use rustc_span::Span;
use serde::Serialize;
use std::{
  cell::RefCell,
  collections::HashSet,
  fs::File,
  io::{self, Write},
  process::Command,
};

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
    if state.relevant {
      let source_info = self.body.source_info(location);
      self.relevant_spans.push(source_info.span);
    }
  }
}

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: HashSet<PlacePrim>,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if !self.slice_span.contains(span) {
      return;
    }

    if let Some(prim) = PlacePrim::from_place(*place) {
      self.slice_set.insert(prim);
    }
  }
}

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>,
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

pub fn analyze(tcx: TyCtxt, body_id: &rustc_hir::BodyId) -> Result<()> {
  let config = CONFIG.get().context("Config")?;

  let local_def_id = body_id.hir_id.owner;
  let body = tcx.optimized_mir(local_def_id);

  println!("MIR");
  write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut io::stdout().lock())?;
  println!("============");

  //let borrowck_result = tcx.mir_borrowck(local_def_id);
  // println!("{:#?}", borrowck_result);

  let source_map = tcx.sess.source_map();
  let mut finder = FindInitialSliceSet {
    slice_span: config.range.to_span(source_map),
    slice_set: HashSet::new(),
    body,
  };
  finder.visit_body(body);
  println!("Initial slice set: {:?}", finder.slice_set);

  let points_to_results = PointsToAnalysis
    .into_engine(tcx, body)
    .iterate_to_fixpoint();

  let relevance_results = RelevanceAnalysis {
    slice_set: finder.slice_set,
    points_to: RefCell::new(ResultsRefCursor::new(body, &points_to_results)),
  }
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
  let output = SliceOutput { ranges };
  println!("{}", serde_json::to_string(&output).unwrap());

  Ok(())
}
