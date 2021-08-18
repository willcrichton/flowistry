use crate::core::{
  aliases::Aliases,
  analysis::{FlowistryAnalysis, FlowistryOutput},
  control_dependencies::ControlDependencies,
  indexed::{IndexSetIteratorExt, IndexedDomain},
  indexed_impls::{LocationDomain, PlaceDomain, PlaceSet},
  utils::elapsed,
};
use relevance::{RelevanceAnalysis, SliceSet};
use relevance_domain::RelevanceDomain;

use anyhow::{bail, Result};
use log::debug;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_graphviz as dot;
use rustc_hir::BodyId;
use rustc_index::bit_set::BitSet;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{self, TyCtxt},
};
use rustc_mir::{
  consumers::get_body_with_borrowck_facts,
  dataflow::{fmt::DebugWithContext, /*graphviz,*/ Analysis, Results, ResultsVisitor},
  /*util::write_mir_fn,*/
};
use rustc_span::Span;
use serde::Serialize;
use std::{
  cell::RefCell,
  collections::hash_map::DefaultHasher,
  fs::File,
  hash::{Hash, Hasher},
  io::Write,
  process::Command,
  rc::Rc,
  thread_local,
  time::Instant,
};

mod config;
// mod eval_extensions;
mod relevance;
mod relevance_domain;

pub use config::{Config, Range};

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: SliceSet<'tcx>,
  body: &'a Body<'tcx>,
  place_domain: Rc<PlaceDomain<'tcx>>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if !self.slice_span.contains(span) || context.is_place_assignment() {
      return;
    }

    let place_domain = &self.place_domain;
    self
      .slice_set
      .entry(location)
      .or_insert_with(|| PlaceSet::new(place_domain.clone()))
      .insert(place_domain.index(place));
  }
}

struct CollectResults<'a, 'tcx> {
  body: &'a Body<'tcx>,
  place_domain: Rc<PlaceDomain<'tcx>>,
  location_domain: &'a LocationDomain,
  relevant_spans: Vec<Span>,
  all_locals: BitSet<Local>,
  local_blacklist: BitSet<Local>,
  num_relevant_instructions: usize,
  num_instructions: usize,
  input_places: Vec<Place<'tcx>>,
  mutated_inputs: HashSet<usize>,
  relevant_inputs: HashSet<usize>,
}

impl<'a, 'tcx> CollectResults<'a, 'tcx> {
  fn check_statement(&mut self, state: &RelevanceDomain, location: Location) {
    if state
      .locations
      .contains(self.location_domain.index(&location))
    {
      let span = self.body.source_info(location).span;
      self.relevant_spans.push(span);
    }
  }

  fn add_locals(&mut self, state: &RelevanceDomain, _location: Location) {
    for place in state.places.iter() {
      self.all_locals.insert(place.local);

      let local = place.local.as_usize();
      if 1 <= local && local <= self.body.arg_count {
        self.relevant_inputs.insert(local);
      }
    }

    let place_domain = &self.place_domain;
    let mutated_inputs = self
      .input_places
      .iter()
      .enumerate()
      .filter_map(|(i, place)| state.mutated.contains(place_domain.index(place)).then(|| i));
    self.mutated_inputs.extend(mutated_inputs);
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
    let is_relevant = state
      .locations
      .contains(self.location_domain.index(&location));

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

    if state
      .locations
      .contains(self.location_domain.index(&location))
    {
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
  // let graphviz = graphviz::Formatter::new(body, &results, graphviz::OutputStyle::AfterOnly);
  // let mut buf = Vec::new();
  // dot::render(&graphviz, &mut buf)?;
  // let mut file = File::create("/tmp/graph.dot")?;
  // file.write_all(&buf)?;
  // let status = Command::new("dot")
  //   .args(&["-Tpng", "/tmp/graph.dot", "-o", path])
  //   .status()?;
  // if !status.success() {
  //   bail!("dot for {} failed", path)
  // };
  Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SliceLocation<'tcx> {
  Span(Span),
  PlacesOnExit(Vec<Place<'tcx>>),
}

impl SliceLocation<'tcx> {
  fn to_slice_set(
    &self,
    body: &Body<'tcx>,
    place_domain: Rc<PlaceDomain<'tcx>>,
  ) -> (SliceSet<'tcx>, Vec<Place<'tcx>>) {
    match self {
      SliceLocation::Span(slice_span) => {
        let mut finder = FindInitialSliceSet {
          slice_span: *slice_span,
          slice_set: HashMap::default(),
          body,
          place_domain,
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

        let place_set = places
          .iter()
          .map(|place| place_domain.index(place))
          .collect_indices(place_domain.clone());
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

thread_local! {
  pub static BODY_STACK: RefCell<Vec<BodyId>> = RefCell::new(Vec::new());
}

fn analyze_inner(
  config: &Config,
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  slice_location: &SliceLocation<'tcx>,
) -> Result<SliceOutput> {
  BODY_STACK.with(|body_stack| {
    body_stack.borrow_mut().push(body_id);

    let start = Instant::now();
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts =
      get_body_with_borrowck_facts(tcx, ty::WithOptConstParam::unknown(local_def_id));
    let body = &body_with_facts.body;
    let outlives_constraints = body_with_facts
      .input_facts
      .outlives
      .into_iter()
      .map(|(r1, r2, location)| {
        (r1, r2, location)
      })
      .collect::<Vec<_>>();
    elapsed("borrowck", start);

    let start = Instant::now();
    if config.debug {
      // let mut buffer = Vec::new();
      // write_mir_fn(tcx, body, &mut |_, _| Ok(()), &mut buffer)?;
      // debug!("{}", String::from_utf8(buffer)?);
      debug!("outlives constraints {:#?}", outlives_constraints);
    }

    // let should_be_conservative = config.eval_mode.pointer_mode == PointerMode::Conservative;
    // let conservative_sccs = if should_be_conservative {
    //   Some(eval_extensions::generate_conservative_constraints(
    //     tcx,
    //     body,
    //     outlives_constraints,
    //   ))
    // } else {
    //   None
    // };
    // let constraint_sccs = if should_be_conservative {
    //   conservative_sccs.as_ref().unwrap()
    // } else {
    //   constraint_sccs
    // };

    let extra_places = match &slice_location {
      SliceLocation::PlacesOnExit(places) => places.clone(),
      _ => vec![],
    };
    let aliases = Aliases::build(
      &config.eval_mode.mutability_mode,
      tcx,
      body,
      outlives_constraints,
      &extra_places,
    );

    let (slice_set, input_places) = slice_location.to_slice_set(body, aliases.place_domain.clone());
    debug!("Initial slice set: {:?}", slice_set);

    let control_dependencies = ControlDependencies::build(body.clone());
    debug!("Control dependencies: {:?}", control_dependencies);
    elapsed("pre-relevance", start);

    let start = Instant::now();
    let relevance_results =
      RelevanceAnalysis::new(config, slice_set, tcx, body, &aliases, control_dependencies)
        .into_engine(tcx, body)
        .iterate_to_fixpoint();
    elapsed("fixpoint", start);

    if config.debug && body_stack.borrow().len() == 1 {
      dump_results("target/relevance.png", body, &relevance_results)?;
    }

    let start = Instant::now();
    let source_map = tcx.sess.source_map();
    let mut visitor = CollectResults {
      body,
      place_domain: aliases.place_domain.clone(),
      location_domain: &relevance_results.analysis.location_domain,
      relevant_spans: vec![],
      all_locals: BitSet::new_empty(body.local_decls().len()),
      local_blacklist: BitSet::new_empty(body.local_decls().len()),
      num_relevant_instructions: 0,
      num_instructions: 0,
      input_places,
      mutated_inputs: HashSet::default(),
      relevant_inputs: HashSet::default(),
    };
    relevance_results.visit_reachable_with(body, &mut visitor);
    elapsed("collect", start);

    visitor.all_locals.subtract(&visitor.local_blacklist);
    let local_spans = visitor
      .all_locals
      .iter()
      .map(|local| body.local_decls()[local].source_info.span);

    let src_file = source_map.lookup_source_file(config.range.to_span().lo());
    let ranges = visitor
      .relevant_spans
      .into_iter()
      .chain(local_spans)
      // TODO: is there a better way to handle spans from macros  than
      //  filtering them out?
      .filter(|span| source_map.lookup_source_file(span.lo()).name == src_file.name)
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

thread_local! {
  pub static RESULT_CACHE: RefCell<HashMap<u64, SliceOutput>> = RefCell::new(HashMap::default());
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
  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

impl FlowistryOutput for SliceOutput {
  fn empty() -> Self {
    SliceOutput {
      ranges: Vec::new(),
      num_instructions: 0,
      num_relevant_instructions: 0,
      mutated_inputs: HashSet::default(),
      relevant_inputs: HashSet::default(),
    }
  }

  fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges.into_iter());
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
  }
}

pub struct SlicerAnalysis {
  pub config: Config,
}

impl SlicerAnalysis {
  pub fn slice_location<'tcx>(&self, tcx: TyCtxt<'tcx>) -> SliceLocation<'tcx> {
    match self.config.local {
      Some(local) => SliceLocation::PlacesOnExit(vec![Place {
        local: Local::from_usize(local),
        projection: tcx.intern_place_elems(&[]),
      }]),
      None => SliceLocation::Span(self.config.range.to_span()),
    }
  }
}

impl FlowistryAnalysis for SlicerAnalysis {
  type Output = SliceOutput;

  fn locations(&self, _tcx: TyCtxt) -> Vec<Span> {
    vec![self.config.range.to_span()]
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    RESULT_CACHE.with(|result_cache| {
      let slice_location = self.slice_location(tcx);
      match &slice_location {
        SliceLocation::PlacesOnExit(places) => {
          let key = (self.config.clone(), body_id, places);
          let hash = {
            let mut hasher = DefaultHasher::new();
            key.hash(&mut hasher);
            hasher.finish()
          };
          let result = { result_cache.borrow().get(&hash).cloned() };
          match result {
            Some(result) => Ok(result),
            None => {
              let result = analyze_inner(&self.config, tcx, body_id, &slice_location)?;
              result_cache.borrow_mut().insert(hash, result.clone());
              Ok(result)
            }
          }
        }
        SliceLocation::Span(_) => analyze_inner(&self.config, tcx, body_id, &slice_location),
      }
    })
  }
}

pub fn slice(config: Config, compiler_args: &[String]) -> Result<SliceOutput> {
  SlicerAnalysis { config }.run(compiler_args)
}
