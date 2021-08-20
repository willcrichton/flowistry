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

struct FlowPlaceVisitor<'a, 'tcx> {
  state: &'a FlowDomain<'tcx>,
  visitor: &'a mut FlowResultsVisitor,
}

struct FlowResultsVisitor {
  targets: Vec<IndexSet<Location>>,
  relevant: IndexSet<Location>,
}

impl Visitor<'tcx> for FlowPlaceVisitor<'_, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.visitor.relevant.contains(location) {
      return;
    }

    let place_deps = self.state.row_set(*place);
    if self
      .visitor
      .targets
      .iter()
      .any(|target| place_deps.is_superset(target))
    {
      debug!("ADDING LOCATION {:?}", location);
      self.visitor.relevant.insert(location);
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for FlowResultsVisitor {
  type FlowState = FlowDomain<'tcx>;

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
    let location_domain = results.analysis.location_domain.clone();

    let sliced_places = utils::span_to_places(body, self.config.range.to_span());
    debug!("sliced_places {:?}", sliced_places);

    let mut cursor = ResultsCursor::new(body, &results);
    let targets = sliced_places
      .into_iter()
      .map(|(place, location)| {
        cursor.seek_after_primary_effect(location);
        cursor.get().row_set(place)
      })
      .collect::<Vec<_>>();
    debug!("targets: {:?}", targets);

    let mut visitor = FlowResultsVisitor {
      targets,
      relevant: IndexSet::new(location_domain.clone()),
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
        spanner
          .find_enclosing_hir_span(mir_span)
          .and_then(|hir_span| Range::from_span(hir_span, source_map).ok())
      })
      .collect::<Vec<_>>();

    let mut output = SliceOutput::empty();
    output.ranges = ranges;
    Ok(output)
  }
}

pub fn forward_slice(config: Config, compiler_args: &[String]) -> Result<SliceOutput> {
  ForwardSliceAnalysis { config }.run(compiler_args)
}
