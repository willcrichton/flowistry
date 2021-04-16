use super::aliases::compute_aliases;
use super::eval_extensions;
use super::post_dominators::compute_post_dominators;
use super::relevance::{RelevanceAnalysis, RelevanceDomain, SliceSet};
use crate::config::{Config, PointerMode, Range};

use anyhow::{bail, Result};
use log::{debug, info};
use rustc_graphviz as dot;
use rustc_hir::BodyId;
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
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
  fs::File,
  io::Write,
  process::Command,
  thread_local,
  time::Instant,
};

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
  relevant_locals: HashSet<Local>,
  relevant_spans: Vec<Span>,
  all_locals: HashSet<Local>,
  local_blacklist: HashSet<Local>,
  num_relevant_instructions: usize,
  num_instructions: usize
}

impl<'a, 'tcx> CollectResults<'a, 'tcx> {
  fn check_statement(&mut self, state: &RelevanceDomain, location: Location) {
    if state.statement_relevant {
      let span = self.body.source_info(location).span;
      self.relevant_spans.push(span);
    }
  }

  fn add_locals(&mut self, state: &RelevanceDomain) {
    let locals = state
      .places
      .iter()
      .map(|place| place.local)
      .collect::<HashSet<_>>();
    self.all_locals = &self.all_locals | &(&locals - &self.relevant_locals);
  }
}

impl<'a, 'mir, 'tcx> ResultsVisitor<'mir, 'tcx> for CollectResults<'a, 'tcx> {
  type FlowState = RelevanceDomain<'tcx>;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir mir::Statement<'tcx>,
    location: Location,
  ) {
    self.add_locals(state);

    if let StatementKind::Assign(box (lhs, Rvalue::Discriminant(_))) = statement.kind {
      /* For whatever reason, in statements like `match x { None => .. }` then the discriminant
       * is source-mapped to the first match pattern (e.g. None above) which produces incorrect highlighting.
       * So for now, we just explictly ignore relevant statements/locals of the form `_1 = discriminant(..)`
       */
      self.local_blacklist.insert(lhs.local);
    } else {
      self.check_statement(state, location);

      if state.statement_relevant {
        if let StatementKind::Assign(box (place, _)) = statement.kind {
          self.all_locals.insert(place.local);
        }
      }
    }

    if state.statement_relevant {
      self.num_relevant_instructions += 1;
    }
    self.num_instructions += 1;
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

    if state.statement_relevant {
      self.num_relevant_instructions += 1;
    }
    self.num_instructions += 1;
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

#[derive(Debug, Clone)]
pub struct SliceOutput {
  spans: Vec<Range>,
  pub num_instructions: usize,
  pub num_relevant_instructions: usize
}

impl SliceOutput {
  pub fn new() -> Self {
    SliceOutput {
      spans: Vec::new(),
      num_instructions: 0,
      num_relevant_instructions: 0
    }
  }

  pub fn merge(&mut self, other: SliceOutput) {
    self.spans.extend(other.spans.into_iter());
  }

  pub fn ranges(&self) -> &Vec<Range> {
    &self.spans
  }
}

pub fn elapsed(name: &str, start: Instant) {
  info!("{} took {}s", name, start.elapsed().as_nanos() as f64 / 1e9)
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct CacheKey(Config, BodyId, Option<Span>, Vec<Local>);

thread_local! {
  static ANALYSIS_CACHE: RefCell<HashMap<CacheKey, (SliceOutput, HashSet<Local>)>> = RefCell::new(HashMap::new());
  pub static BODY_STACK: RefCell<Vec<BodyId>> = RefCell::new(Vec::new());
}

pub fn analyze_function(
  config: &Config,
  tcx: TyCtxt,
  body_id: BodyId,
  slice_span: Option<Span>,
  relevant_locals: Vec<Local>,
) -> Result<(SliceOutput, HashSet<Local>)> {
  let analyze = || -> Result<_> {
    let start = Instant::now();
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let borrowck_result = tcx.mir_borrowck(local_def_id);
    elapsed("borrowck", start);

    let start = Instant::now();
    let body = &borrowck_result.intermediates.body;
    let borrow_set = &borrowck_result.intermediates.borrow_set;
    let outlives_constraints = &borrowck_result.intermediates.outlives_constraints;
    let constraint_sccs = &borrowck_result.intermediates.constraint_sccs;

    let mut buffer = Vec::new();
    write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut buffer)?;
    debug!("{}", String::from_utf8(buffer)?);
    debug!("borrow set {:#?}", borrow_set);
    debug!("outlives constraints {:#?}", outlives_constraints);
    debug!("sccs {:#?}", constraint_sccs);

    let post_dominators = compute_post_dominators(body.clone());
    for bb in body.basic_blocks().indices() {
      if post_dominators.is_reachable(bb) {
        debug!(
          "{:?} dominated by {:?}",
          bb,
          post_dominators.immediate_dominator(bb)
        );
      }
    }

    let should_be_conservative = config.eval_mode.pointer_mode == PointerMode::Conservative;
    let conservative_sccs = if should_be_conservative {
      Some(eval_extensions::generate_conservative_constraints(
        tcx,
        body,
        outlives_constraints,
      ))
    } else {
      None
    };

    let constraint_sccs = if should_be_conservative {
      conservative_sccs.as_ref().unwrap()
    } else {
      constraint_sccs
    };

    let aliases = compute_aliases(config, tcx, body, borrow_set, outlives_constraints, constraint_sccs);

    let slice_set = if let Some(slice_span) = slice_span {
      let mut finder = FindInitialSliceSet {
        slice_span,
        slice_set: HashSet::new(),
        body,
      };
      finder.visit_body(body);
      finder.slice_set
    } else {
      HashSet::new()
    };

    debug!("Initial slice set: {:?}", slice_set);
    elapsed("pre-relevance", start);

    let start = Instant::now();
    let relevant_locals = relevant_locals.iter().cloned().collect::<HashSet<_>>();
    let relevance_results = RelevanceAnalysis::new(
      config,
      slice_set,
      relevant_locals.clone(),
      tcx,
      body,
      &aliases,
      post_dominators,
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
      relevant_locals: relevant_locals.clone(),
      all_locals: HashSet::new(),
      local_blacklist: HashSet::new(),
      num_relevant_instructions: 0,
      num_instructions: 0
    };
    relevance_results.visit_reachable_with(body, &mut visitor);
    elapsed("relevance", start);

    let all_locals = &visitor.all_locals - &visitor.local_blacklist;
    let local_spans = all_locals
      .into_iter()
      .map(|local| body.local_decls()[local].source_info.span);

    let ranges = visitor
      .relevant_spans
      .into_iter()
      .chain(local_spans)
      .filter_map(|span| Range::from_span(span, source_map).ok())
      .collect();

    Ok((SliceOutput {
      spans: ranges,
      num_instructions: visitor.num_instructions,
      num_relevant_instructions: visitor.num_relevant_instructions
    }, visitor.all_locals))
  };

  ANALYSIS_CACHE.with(|cache| {
    let key = CacheKey(
      config.clone(),
      body_id,
      slice_span.clone(),
      relevant_locals.clone(),
    );

    if !cache.borrow().contains_key(&key) {
      let results = BODY_STACK.with(|body_stack| {
        body_stack.borrow_mut().push(body_id);
        let results = analyze();
        body_stack.borrow_mut().pop();
        results
      })?;
      cache.borrow_mut().insert(key.clone(), results);
    }

    Ok(cache.borrow().get(&key).unwrap().clone())
  })
}
