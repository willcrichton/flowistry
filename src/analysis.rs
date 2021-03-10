use crate::config::{Range, CONFIG};
use crate::relevance::RelevanceAnalysis;
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
use rustc_mir::dataflow::{graphviz, Analysis, AnalysisDomain, ResultsVisitor};
use rustc_mir::util::write_mir_fn;
use rustc_span::Span;
use serde::Serialize;
use std::{
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
  type FlowState = <RelevanceAnalysis as AnalysisDomain<'tcx>>::Domain;

  fn visit_statement_before_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir mir::Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(assign) => {
        let (place, _rvalue) = &**assign;
        let local = place.local;
        //println!("{:?} {:?} {:?}", state, local, state.contains(local));
        if state.contains(local) {
          let source_info = self.body.source_info(location);
          self.relevant_spans.push(source_info.span);
        }
      }
      _ => {}
    }
  }
}

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: HashSet<(Local, Location)>,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_local(&mut self, local: &Local, context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if !self.slice_span.contains(span) {
      return;
    }

    match context {
      PlaceContext::NonMutatingUse(_) => {
        self.slice_set.insert((*local, location));
      }
      _ => {}
    };
  }
}

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>,
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

  let mut visitor = CollectResults {
    body,
    relevant_spans: vec![],
  };
  let results = RelevanceAnalysis {
    slice_set: finder.slice_set,
  }
  .into_engine(tcx, body)
  .iterate_to_fixpoint();

  results.visit_reachable_with(body, &mut visitor);

  if config.debug {
    let graphviz = graphviz::Formatter::new(body, &results, graphviz::OutputStyle::AfterOnly);
    let mut buf = Vec::new();
    dot::render(&graphviz, &mut buf)?;
    let mut file = File::create("target/analysis.dot")?;
    file.write_all(&buf)?;
    Command::new("dot")
      .args(&["-Tpng", "target/analysis.dot", "-o", "target/analysis.png"])
      .status()?;
  }

  let ranges = visitor
    .relevant_spans
    .into_iter()
    .map(|span| Range::from_span(span, source_map))
    .collect();
  let output = SliceOutput { ranges };
  println!("{}", serde_json::to_string(&output).unwrap());

  Ok(())
}
