use super::{
  analysis::{FlowAnalysis, FlowDomain},
  FlowResults,
};
use crate::{
  block_timer,
  indexed::{
    impls::{LocationSet, PlaceIndex, PlaceSet},
    IndexedDomain,
  },
  mir::utils,
  source_map::{location_to_spans, HirSpanner},
};
use log::{debug, trace};
use rustc_middle::mir::*;
use rustc_mir_dataflow::ResultsVisitor;
use rustc_span::Span;

#[derive(Clone, Copy, Debug)]
pub enum Direction {
  Forward,
  Backward,
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
    to_check: Vec<PlaceIndex>,
  ) {
    for (target_locs, (out_locs, out_places)) in
      self.target_deps.iter().zip(self.outputs.iter_mut())
    {
      for place in to_check.iter() {
        if let Some(loc_deps) = state.row_set(*place) {
          if loc_deps.indices().next().is_none() {
            continue;
          }

          let matches = match self.direction {
            Direction::Forward => loc_deps.is_superset(target_locs),
            Direction::Backward => target_locs.is_superset(&loc_deps),
          };

          if matches {
            trace!(
              "{:?}: place {:?} (deps {:?}) / target_locs {:?}",
              opt_location,
              state.row_domain.value(*place),
              loc_deps,
              target_locs
            );
            out_places.insert(*place);

            if let Some(location) = opt_location {
              if loc_deps.contains(location) {
                out_locs.insert(location);
              }
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
      let place_domain = self.analysis.place_domain();
      self.visit(state, None, place_domain.all_args(self.analysis.body));
    }
  }

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    match statement.kind {
      StatementKind::Assign(box (lhs, _)) => {
        self.visit(
          state,
          Some(location),
          self.analysis.aliases.conflicts(lhs).indices().collect(),
        );
      }
      _ => {}
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    terminator: &'mir rustc_middle::mir::Terminator<'tcx>,
    location: Location,
  ) {
    if matches!(
      terminator.kind,
      TerminatorKind::Call { .. }
        | TerminatorKind::DropAndReplace { .. }
        | TerminatorKind::SwitchInt { .. }
    ) {
      // TODO: optimize this by only checking the set of possibly mutated objects
      // BIGGER TODO: unify this logic with dataflow.rs to avoid copying
      self.visit(
        state,
        Some(location),
        self.analysis.place_domain().as_vec().indices().collect(),
      );
    }
  }
}

pub fn compute_dependencies(
  results: &FlowResults<'_, 'tcx>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
) -> Vec<(LocationSet, PlaceSet<'tcx>)> {
  block_timer!("compute_dependencies");
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;
  let aliases = &results.analysis.aliases;

  let new_location_set = || LocationSet::new(results.analysis.location_domain().clone());
  let new_place_set = || PlaceSet::new(results.analysis.place_domain().clone());

  let expanded_targets = targets
    .iter()
    .map(|(place, location)| {
      let mut places = new_place_set();
      places.insert(*place);

      for (_, ptrs) in utils::interior_pointers(*place, tcx, body, results.analysis.def_id) {
        for (place, _) in ptrs {
          debug!(
            "{:?} // {:?}",
            tcx.mk_place_deref(place),
            aliases.aliases.row_set(tcx.mk_place_deref(place))
          );
          places.union(&aliases.aliases.row_set(tcx.mk_place_deref(place)).unwrap());
        }
      }

      (places, *location)
    })
    .collect::<Vec<_>>();
  debug!(
    "Expanded targets from {:?} to {:?}",
    targets, expanded_targets
  );

  let target_deps = {
    let get_deps = |(targets, location): &(PlaceSet<'tcx>, Location)| {
      let state = results.state_at(*location);

      let mut locations = new_location_set();
      for target in targets.indices() {
        if let Some(dep_locations) = state.row_set(target) {
          locations.union(&dep_locations);
        }
      }

      locations
    };
    expanded_targets.iter().map(get_deps).collect::<Vec<_>>()
  };
  debug!("Target deps: {:?}", target_deps);

  let mut outputs = target_deps
    .iter()
    .map(|_| (new_location_set(), new_place_set()))
    .collect::<Vec<_>>();
  for ((target_places, _), (_, places)) in expanded_targets.iter().zip(outputs.iter_mut()) {
    places.union(target_places);
  }

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
  spanner: &HirSpanner,
) -> Vec<Vec<Span>> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;

  let source_map = tcx.sess.source_map();
  let deps = compute_dependencies(results, targets, direction);

  deps
    .into_iter()
    .map(|(locations, places)| {
      let location_spans = locations
        .iter()
        .map(|location| location_to_spans(*location, body, spanner, source_map))
        .flatten();

      let place_spans = places
        .iter()
        .filter(|place| **place != Place::return_place())
        .map(|place| {
          body.local_decls()[place.local]
            .source_info
            .span
            .source_callsite()
        });

      location_spans.chain(place_spans).collect::<Vec<_>>()
    })
    .collect::<Vec<_>>()
}
