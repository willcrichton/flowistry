use log::{debug, trace};
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_middle::mir::{visit::Visitor, *};
use rustc_mir_dataflow::ResultsVisitor;
use rustc_span::Span;

use super::{
  analysis::{FlowAnalysis, FlowDomain},
  mutation::ModularMutationVisitor,
  FlowResults,
};
use crate::{
  block_timer,
  indexed::impls::{LocationSet, PlaceSet},
  mir::utils::SpanExt,
  source_map::{EnclosingHirSpans, Spanner},
};

#[derive(Clone, Copy, Debug)]
pub enum Direction {
  Forward,
  Backward,
  Both,
}

struct DepVisitor<'a, 'mir, 'tcx> {
  direction: Direction,
  target_deps: Vec<LocationSet>,
  outputs: Vec<(LocationSet, PlaceSet<'tcx>)>,
  analysis: &'a FlowAnalysis<'mir, 'tcx>,
}

impl DepVisitor<'_, '_, 'tcx> {
  fn visit(
    &mut self,
    state: &FlowDomain<'tcx>,
    opt_location: Option<Location>,
    to_check: PlaceSet<'tcx>,
    is_switch: bool,
  ) {
    for (target_locs, (out_locs, out_places)) in
      self.target_deps.iter().zip(self.outputs.iter_mut())
    {
      for (place, loc_deps) in to_check
        .iter()
        .copied()
        .map(|place| (place, state.row_set(self.analysis.aliases.normalize(place))))
        .filter(|(_, loc_deps)| !loc_deps.is_empty())
      {
        let matches = match self.direction {
          Direction::Forward => loc_deps.is_superset(target_locs),
          Direction::Backward => target_locs.is_superset(&loc_deps),
          Direction::Both => {
            loc_deps.is_superset(target_locs) || target_locs.is_superset(&loc_deps)
          }
        };

        if matches {
          trace!(
            "{opt_location:?}: place {:?} (deps {loc_deps:?}) / target_locs {target_locs:?}",
            place
          );
          out_places.insert(place);

          if let Some(location) = opt_location {
            if loc_deps.contains(location)
              || (is_switch && target_locs.contains(location))
            {
              out_locs.insert(location);
            }
          }
        }
      }
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for DepVisitor<'_, 'mir, 'tcx> {
  type FlowState = FlowDomain<'tcx>;

  fn visit_block_start(
    &mut self,
    state: &Self::FlowState,
    _block_data: &'mir BasicBlockData<'tcx>,
    block: BasicBlock,
  ) {
    if block == START_BLOCK {
      self.visit(
        state,
        None,
        self
          .analysis
          .location_domain()
          .all_args()
          .map(|(place, _)| place)
          .collect(),
        false,
      );
    }
  }

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    let mut to_check = PlaceSet::default();
    ModularMutationVisitor::new(
      self.analysis.tcx,
      self.analysis.body,
      self.analysis.def_id,
      |mutated, _, _, _| {
        to_check.extend(self.analysis.aliases.conflicts(mutated));
      },
    )
    .visit_statement(statement, location);
    self.visit(state, Some(location), to_check, false);
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    terminator: &'mir rustc_middle::mir::Terminator<'tcx>,
    location: Location,
  ) {
    match terminator.kind {
      TerminatorKind::SwitchInt { .. } => {
        let to_check = state.rows().map(|(p, _)| p).collect::<HashSet<_>>();
        self.visit(state, Some(location), to_check, true);
      }
      _ => {
        let mut to_check = PlaceSet::default();
        ModularMutationVisitor::new(
          self.analysis.tcx,
          self.analysis.body,
          self.analysis.def_id,
          |mutated, _, _, _| {
            to_check.extend(self.analysis.aliases.conflicts(mutated));
          },
        )
        .visit_terminator(terminator, location);
        self.visit(state, Some(location), to_check, false);
      }
    }
  }
}

pub fn compute_dependencies(
  results: &FlowResults<'_, 'tcx>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
) -> Vec<(LocationSet, PlaceSet<'tcx>)> {
  block_timer!("compute_dependencies");
  let body = results.analysis.body;
  let aliases = &results.analysis.aliases;
  let location_domain = results.analysis.location_domain();

  let target_deps = targets
    .into_iter()
    .map(|(place, location)| {
      let places = aliases.reachable_values(place);
      let state = results.state_at(location);

      let mut locations = LocationSet::new(location_domain);
      for place in places {
        locations.union(&state.row_set(aliases.normalize(*place)));
      }

      locations
    })
    .collect::<Vec<_>>();

  let outputs = target_deps
    .iter()
    .map(|_| (LocationSet::new(location_domain), PlaceSet::default()))
    .collect::<Vec<_>>();

  let mut visitor = DepVisitor {
    analysis: &results.analysis,
    direction,
    target_deps,
    outputs,
  };
  results.visit_reachable_with(body, &mut visitor);
  debug!("visitor.outputs: {:?}", visitor.outputs);

  visitor.outputs
}

pub fn compute_dependency_spans(
  results: &FlowResults<'_, 'tcx>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
  spanner: &Spanner,
) -> Vec<Vec<Span>> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;

  let deps = compute_dependencies(results, targets, direction);

  deps
    .into_iter()
    .map(|(locations, places)| {
      let location_spans = locations.iter().flat_map(|location| {
        spanner.location_to_spans(*location, EnclosingHirSpans::OuterOnly)
      });

      let place_spans = places
        .iter()
        .filter_map(|place| {
          body.local_decls()[place.local]
            .source_info
            .span
            .as_local(tcx)
        })
        .filter(|span| !spanner.invalid_span(*span));

      let all_spans = location_spans.chain(place_spans).collect::<Vec<_>>();
      trace!("Before merging: {all_spans:?}");
      Span::merge_overlaps(all_spans)
    })
    .collect::<Vec<_>>()
}
