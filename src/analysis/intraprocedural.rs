use super::aliases::compute_aliases;
use super::control_dependencies::ControlDependencies;
use super::eval_extensions;
use super::relevance::{RelevanceAnalysis, RelevanceDomain, SliceSet};
use crate::config::{Config, PointerMode, Range};

use anyhow::{bail, Result};
use log::{debug, info};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
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
use serde::Serialize;
use std::{cell::RefCell, fs::File, io::Write, process::Command, thread_local, time::Instant};

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: SliceSet<'tcx>,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if !self.slice_span.contains(span) || context.is_place_assignment() {
      return;
    }

    self
      .slice_set
      .entry(location)
      .or_insert_with(HashSet::default)
      .insert(*place);
  }
}

struct CollectResults<'a, 'tcx> {
  body: &'a Body<'tcx>,
  relevant_spans: Vec<Span>,
  all_locals: HashSet<Local>,
  local_blacklist: HashSet<Local>,
  num_relevant_instructions: usize,
  num_instructions: usize,
  input_places: Vec<Place<'tcx>>,
  mutated_inputs: HashSet<usize>,
  relevant_inputs: HashSet<usize>,
}

impl<'a, 'tcx> CollectResults<'a, 'tcx> {
  fn check_statement(&mut self, state: &RelevanceDomain, location: Location) {
    if state.relevant_statements.contains_key(&location) {
      let span = self.body.source_info(location).span;
      self.relevant_spans.push(span);
    }
  }

  fn add_locals(&mut self, state: &RelevanceDomain, location: Location) {
    let locals = state
      .relevant_places
      .iter()
      .map(|place| place.local)
      .collect::<HashSet<_>>();
    self.all_locals = &self.all_locals | &locals; //&(&locals - &self.relevant_locals);

    for place in state.relevant_places.iter() {
      let local = place.local.as_usize();
      if 1 <= local && local <= self.body.arg_count {
        self.relevant_inputs.insert(local - 1);
      }
    }

    if let Some(trace) = state.relevant_statements.get(&location) {
      let mutated_inputs = self
        .input_places
        .iter()
        .enumerate()
        .filter_map(|(i, place)| trace.mutated.contains(place).then(|| i));

      self.mutated_inputs.extend(mutated_inputs);
    }
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
    self.add_locals(state, location);
    let is_relevant = state.relevant_statements.contains_key(&location);

    if let StatementKind::Assign(box (lhs, Rvalue::Discriminant(_))) = statement.kind {
      /* For whatever reason, in statements like `match x { None => .. }` then the discriminant
       * is source-mapped to the first match pattern (e.g. None above) which produces incorrect highlighting.
       * So for now, we just explictly ignore relevant statements/locals of the form `_1 = discriminant(..)`
       */
      self.local_blacklist.insert(lhs.local);
    } else {
      self.check_statement(state, location);

      if is_relevant {
        if let StatementKind::Assign(box (place, _)) = statement.kind {
          self.all_locals.insert(place.local);
        }
      }
    }

    if is_relevant {
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
    self.add_locals(state, location);

    if let mir::TerminatorKind::SwitchInt { .. } = terminator.kind {
      /* Conditional control flow gets source-mapped to the entire corresponding if/loop/etc.
       * So eg if only one path is relevant, we mark the switch as relevant, but this would highlight
       * the entire if statement. So for now just ignore this relevant mark and let the statements
       * get individually highlighted as relevant or not.
       */
    } else {
      self.check_statement(state, location);
    }

    if state.relevant_statements.contains_key(&location) {
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

#[derive(Debug, Clone, Serialize)]
pub struct SliceOutput {
  ranges: Vec<Range>,
  pub num_instructions: usize,
  pub num_relevant_instructions: usize,
  pub mutated_inputs: HashSet<usize>,
  pub relevant_inputs: HashSet<usize>,
}

impl SliceOutput {
  pub fn new() -> Self {
    SliceOutput {
      ranges: Vec::new(),
      num_instructions: 0,
      num_relevant_instructions: 0,
      mutated_inputs: HashSet::default(),
      relevant_inputs: HashSet::default(),
    }
  }

  pub fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges.into_iter());
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
  }

  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

pub fn elapsed(name: &str, start: Instant) {
  info!("{} took {}s", name, start.elapsed().as_nanos() as f64 / 1e9)
}

// #[derive(Hash, PartialEq, Eq, Clone)]
// struct CacheKey(Config, BodyId, SliceLocation);

thread_local! {
  // static ANALYSIS_CACHE: RefCell<HashMap<CacheKey, SliceOutput>> = RefCell::new(HashMap::new());
  pub static BODY_STACK: RefCell<Vec<BodyId>> = RefCell::new(Vec::new());
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SliceLocation<'tcx> {
  Span(Span),
  PlacesOnExit(Vec<Place<'tcx>>),
}

impl SliceLocation<'tcx> {
  fn to_slice_set(&self, body: &Body<'tcx>) -> (SliceSet<'tcx>, Vec<Place<'tcx>>) {
    match self {
      SliceLocation::Span(slice_span) => {
        let mut finder = FindInitialSliceSet {
          slice_span: *slice_span,
          slice_set: HashMap::default(),
          body,
        };
        finder.visit_body(body);
        (finder.slice_set, vec![])
      }
      SliceLocation::PlacesOnExit(places) => {
        let return_locations =
          body
            .basic_blocks()
            .iter_enumerated()
            .filter_map(|(block, bb_data)| {
              if let TerminatorKind::Return = bb_data.terminator().kind {
                let statement_index = bb_data.statements.len();
                Some(Location {
                  block,
                  statement_index,
                })
              } else {
                None
              }
            });

        let place_set = places.iter().cloned().collect::<HashSet<_>>();
        (
          return_locations
            .map(|location| (location, place_set.clone()))
            .collect::<HashMap<_, _>>(),
          places.clone(),
        )
      }
    }
  }
}

pub fn analyze_function(
  config: &Config,
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  slice_location: &SliceLocation<'tcx>,
) -> Result<SliceOutput> {
  BODY_STACK.with(|body_stack| {
    body_stack.borrow_mut().push(body_id);

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

    let aliases = compute_aliases(
      config,
      tcx,
      body,
      borrow_set,
      outlives_constraints,
      constraint_sccs,
    );

    let (slice_set, input_places) = slice_location.to_slice_set(body);
    debug!("Initial slice set: {:?}", slice_set);

    let control_dependencies = ControlDependencies::build(body.clone());
    debug!("Control dependencies: {:?}", control_dependencies);

    elapsed("pre-relevance", start);

    let start = Instant::now();
    let relevance_results =
      RelevanceAnalysis::new(config, slice_set, tcx, body, &aliases, control_dependencies)
        .into_engine(tcx, body)
        .iterate_to_fixpoint();

    if config.debug && body_stack.borrow().len() == 1 {
      dump_results("target/relevance.png", body, &relevance_results)?;
    }

    let source_map = tcx.sess.source_map();
    let mut visitor = CollectResults {
      body,
      relevant_spans: vec![],
      all_locals: HashSet::default(),
      local_blacklist: HashSet::default(),
      num_relevant_instructions: 0,
      num_instructions: 0,
      input_places,
      mutated_inputs: HashSet::default(),
      relevant_inputs: HashSet::default(),
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

    body_stack.borrow_mut().pop();

    Ok(SliceOutput {
      ranges,
      num_instructions: visitor.num_instructions,
      num_relevant_instructions: visitor.num_relevant_instructions,
      mutated_inputs: visitor.mutated_inputs,
      relevant_inputs: visitor.relevant_inputs,
    })
  })
}
