use super::aliases::Aliases;
use super::borrow_ranges::BorrowRanges;
use super::place_index::PlaceIndices;
use super::relevance::{RelevanceAnalysis, RelevanceDomain, SliceSet};
use super::post_dominators::compute_post_dominators;
use crate::config::{Range, CONFIG};

use anyhow::{bail, Context, Result};
use log::{debug, info};
use rustc_graphviz as dot;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::graphviz;
use rustc_mir::dataflow::{fmt::DebugWithContext, Analysis, Results, ResultsVisitor};
use rustc_mir::util::write_mir_fn;
use rustc_span::Span;
use std::{collections::HashSet, fs::File, io::Write, process::Command};

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

struct CollectResults<'a, 'tcx> {
  body: &'a Body<'tcx>,
  relevant_spans: Vec<Span>,
  all_locals: HashSet<Local>,
  place_indices: &'a PlaceIndices<'tcx>,
}

impl<'a, 'tcx> CollectResults<'a, 'tcx> {
  fn check_statement(&mut self, state: &RelevanceDomain, location: Location) {
    if state.statement_relevant {
      let span = self.body.source_info(location).span;
      self.relevant_spans.push(span);
    }
  }

  fn add_locals(&mut self, state: &RelevanceDomain) {
    let locals = &state
      .places
      .iter()
      .map(|place| self.place_indices.lookup(place).local)
      .collect::<HashSet<_>>();
    self.all_locals = &self.all_locals | &locals;
  }
}

impl<'a, 'mir, 'tcx> ResultsVisitor<'mir, 'tcx> for CollectResults<'a, 'tcx> {
  type FlowState = RelevanceDomain;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    _statement: &'mir mir::Statement<'tcx>,
    location: Location,
  ) {
    self.add_locals(state);
    self.check_statement(state, location);
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    terminator: &'mir mir::Terminator<'tcx>,
    location: Location,
  ) {
    self.add_locals(state);

    if let mir::TerminatorKind::SwitchInt { .. } = terminator.kind {
      /* Conditional control flow gets source-mapped to the entire corresponding if/loop/etc.
       * So eg if only one path is relevant, we mark the switch as relevant, but this would highlight
       * the entire if statement. So for now just ignore this relevant mark and let the statements
       * get individually highlighted as relevant or not.
       */
    } else {
      self.check_statement(state, location);
    }
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
  let status = Command::new("dot")
    .args(&["-Tpng", "/tmp/graph.dot", "-o", path])
    .status()?;
  if !status.success() {
    bail!("dot for {} failed", path)
  };
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

pub fn analyze_function(
  tcx: TyCtxt,
  body_id: &rustc_hir::BodyId,
  slice_span: Span,
) -> Result<SliceOutput> {
  CONFIG.get(|config| {
    let config = config.context("Missing config")?;

    let local_def_id = body_id.hir_id.owner;
    let borrowck_result = tcx.mir_borrowck(local_def_id);

    let body = &borrowck_result.intermediates.body;
    let borrow_set = &borrowck_result.intermediates.borrow_set;
    let outlives_constraints = &borrowck_result.intermediates.outlives_constraints;
    let borrows_out_of_scope_at_location = &borrowck_result
      .intermediates
      .borrows_out_of_scope_at_location;

    if config.debug {
      info!("MIR");
      let mut buffer = Vec::new();
      write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut buffer)?;
      info!("{}", String::from_utf8_lossy(&buffer));
      info!("borrow set {:#?}", borrow_set);
      info!("out of scope {:#?}", borrows_out_of_scope_at_location);
      info!("outlives constraints {:#?}", outlives_constraints);
    }

    let post_dominators = compute_post_dominators(body.clone());
    for bb in body.basic_blocks().indices() {
      if post_dominators.is_reachable(bb) {
        debug!("{:?} doimnated by {:?}", bb, post_dominators.immediate_dominator(bb));
      }
    }
  
    let borrow_ranges = BorrowRanges::new(tcx, body, borrow_set, borrows_out_of_scope_at_location)
      .into_engine(tcx, body)
      .iterate_to_fixpoint();
    let borrow_ranges = &borrow_ranges;

    if config.debug {
      dump_results("target/borrow_ranges.png", body, borrow_ranges)?;
    }

    let aliases = Aliases::new(body, borrow_set, borrow_ranges, outlives_constraints)
      .into_engine(tcx, body)
      .iterate_to_fixpoint();

    if config.debug {
      dump_results("target/aliases.png", body, &aliases)?;
    }

    let mut finder = FindInitialSliceSet {
      slice_span,
      slice_set: HashSet::new(),
      body,
    };
    finder.visit_body(body);
    debug!("Initial slice set: {:?}", finder.slice_set);

    let place_indices = PlaceIndices::build(body);

    let relevance_results = RelevanceAnalysis::new(
      finder.slice_set,
      tcx,
      body,
      borrow_set,
      borrow_ranges,
      &place_indices,
      &aliases,
      post_dominators
    )
    .into_engine(tcx, body)
    .iterate_to_fixpoint();

    if config.debug {
      dump_results("target/relevance.png", body, &relevance_results)?;
    }

    let source_map = tcx.sess.source_map();
    let mut visitor = CollectResults {
      body,
      relevant_spans: vec![],
      all_locals: HashSet::new(),
      place_indices: &place_indices,
    };
    relevance_results.visit_reachable_with(body, &mut visitor);

    let local_spans = visitor
      .all_locals
      .into_iter()
      .map(|local| body.local_decls()[local].source_info.span);

    let ranges = visitor
      .relevant_spans
      .into_iter()
      .chain(local_spans)
      .map(|span| Range::from_span(span, source_map))
      .collect();

    Ok(SliceOutput(ranges))
  })
}
