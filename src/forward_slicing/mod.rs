use crate::{
  backward_slicing::{Config, Range, SliceOutput},
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    indexed::IndexSet,
    utils,
  },
  flow::{compute_flow, FlowDomain},
};
use anyhow::Result;
use log::debug;
use rustc_hir::BodyId;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{TyCtxt, WithOptConstParam},
};
use rustc_mir::{
  consumers::get_body_with_borrowck_facts,
  dataflow::{ResultsCursor, ResultsVisitor},
};
use rustc_span::Span;

struct FlowPlaceVisitor<'a, 'mir, 'tcx> {
  state: &'a FlowDomain<'tcx>,
  visitor: &'a mut FlowResultsVisitor<'mir, 'tcx>,
}

struct FlowResultsVisitor<'mir, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  targets: Vec<IndexSet<Location>>,
  relevant: IndexSet<Location>,
  relevant_args: Vec<Span>,
}

impl FlowResultsVisitor<'_, 'tcx> {
  fn check(&mut self, place: Place<'tcx>, state: &FlowDomain<'tcx>) -> bool {
    match state.row_set(place) {
      Some(place_deps) => self
        .targets
        .iter()
        .any(|target| place_deps.is_superset(target)),
      None => false,
    }
  }
}

impl Visitor<'tcx> for FlowPlaceVisitor<'_, '_, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.visitor.check(*place, self.state) {
      self.visitor.relevant.insert(location);
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for FlowResultsVisitor<'mir, 'tcx> {
  type FlowState = FlowDomain<'tcx>;

  fn visit_block_start(
    &mut self,
    state: &Self::FlowState,
    _block_data: &'mir BasicBlockData<'tcx>,
    block: BasicBlock,
  ) {
    if block == Location::START.block {
      for arg in self.body.args_iter() {
        let arg_place = utils::local_to_place(arg, self.tcx);
        if self.check(arg_place, state) {
          self
            .relevant_args
            .push(self.body.local_decls()[arg].source_info.span);
        }
      }
    }
  }

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    FlowPlaceVisitor {
      state,
      visitor: self,
    }
    .visit_statement(statement, location);
  }
}

struct ForwardSliceAnalysis {
  config: Config,
}

impl FlowistryAnalysis for ForwardSliceAnalysis {
  type Output = SliceOutput;

  fn locations(&self, _tcx: TyCtxt) -> Vec<Span> {
    vec![self.config.range.to_span()]
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts =
      get_body_with_borrowck_facts(tcx, WithOptConstParam::unknown(local_def_id));
    let body = &body_with_facts.body;
    debug!("{}", utils::mir_to_string(tcx, body)?);

    let results = compute_flow(tcx, &body_with_facts);
    // utils::dump_results("target/flow.png", body, &results)?;

    let sliced_places = utils::span_to_places(tcx, body, self.config.range.to_span());
    debug!("sliced_places {:?}", sliced_places);

    let mut cursor = ResultsCursor::new(body, &results);
    let targets = sliced_places
      .into_iter()
      .filter_map(|(place, location)| {
        cursor.seek_after_primary_effect(location);
        cursor.get().row_set(place).map(|set| set.to_owned())
      })
      .collect::<Vec<_>>();
    debug!("targets: {:?}", targets);

    let mut visitor = FlowResultsVisitor {
      tcx,
      body,
      targets,
      relevant: IndexSet::new(results.analysis.location_domain.clone()),
      relevant_args: Vec::new(),
    };
    results.visit_reachable_with(body, &mut visitor);

    let hir_body = tcx.hir().body(body_id);
    let spanner = utils::HirSpanner::new(hir_body);

    let source_map = tcx.sess.source_map();
    let ranges = visitor
      .relevant
      .iter()
      .filter_map(|location| {
        let mir_span = body.source_info(*location).span;
        spanner.find_enclosing_hir_span(mir_span)
      })
      .chain(visitor.relevant_args.into_iter())
      .filter_map(|span| Range::from_span(span, source_map).ok())
      .collect::<Vec<_>>();

    let mut output = SliceOutput::empty();
    output.ranges = ranges;
    Ok(output)
  }
}

pub fn forward_slice(config: Config, compiler_args: &[String]) -> Result<SliceOutput> {
  ForwardSliceAnalysis { config }.run(compiler_args)
}
