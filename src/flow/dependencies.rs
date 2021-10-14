use rustc_middle::mir::*;
use rustc_mir_dataflow::{Results, ResultsRefCursor, ResultsVisitor};

use super::dataflow::{FlowAnalysis, FlowDomain};
use crate::core::{
  config::Range,
  indexed_impls::{LocationSet, PlaceSet},
  utils,
};

#[derive(Clone, Copy, Debug)]
pub enum Direction {
  Forward,
  Backward,
}

struct ForwardVisitor<'tcx> {
  expanded_targets: Vec<(PlaceSet<'tcx>, Location)>,
  target_deps: Vec<(LocationSet, PlaceSet<'tcx>)>,
  outputs: Vec<(LocationSet, PlaceSet<'tcx>)>,
}

impl ResultsVisitor<'mir, 'tcx> for ForwardVisitor<'tcx> {
  type FlowState = FlowDomain<'tcx>;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    _statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    for ((target_places, _), ((locs, places), (out_locs, out_places))) in self
      .expanded_targets
      .iter()
      .zip(self.target_deps.iter().zip(self.outputs.iter_mut()))
    {
      let mut relevant_loc = false;
      for place in state.locations.rows() {
        if let Some(loc_deps) = state.locations.row_set(place) {
          if loc_deps.contains(location) {
            if !relevant_loc && loc_deps.is_superset(locs) {
              relevant_loc = true;
            }

            if let Some(place_deps) = state.places.row_set(place) {
              let contains_target = {
                let mut place_deps = place_deps.to_owned();
                place_deps.intersect(&target_places);
                place_deps.len() > 0
              };
              if contains_target && place_deps.is_superset(places) {
                out_places.insert(place);
              }
            }
          }
        }
      }

      if relevant_loc {
        out_locs.insert(location);
      }
    }
  }
}

pub fn compute_dependencies(
  results: &Results<'tcx, FlowAnalysis<'mir, 'tcx>>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
) -> Vec<(LocationSet, PlaceSet<'tcx>)> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;
  let aliases = &results.analysis.aliases;

  let new_location_set = || LocationSet::new(results.analysis.location_domain().clone());
  let new_place_set = || PlaceSet::new(results.analysis.place_domain().clone());

  let expanded_targets = targets
    .into_iter()
    .map(|(place, location)| {
      let mut places = new_place_set();
      places.insert(place);

      for (_, ptrs) in utils::interior_pointers(place, tcx, body, results.analysis.def_id) {
        for (place, _) in ptrs {
          places.union(&aliases.aliases.row_set(tcx.mk_place_deref(place)).unwrap());
        }
      }

      (places, location)
    })
    .collect::<Vec<_>>();

  let target_deps = {
    let mut cursor = ResultsRefCursor::new(body, results);
    let get_deps = |(targets, location): &(PlaceSet<'tcx>, Location)| {
      cursor.seek_after_primary_effect(*location);
      let state = cursor.get();

      let mut locations = new_location_set();
      let mut places = new_place_set();

      for target in targets.indices() {
        if let Some(dep_locations) = state.locations.row_set(target) {
          locations.union(&dep_locations);
        }

        if let Some(dep_places) = state.places.row_set(target) {
          places.union(&dep_places);
        }
      }

      (locations, places)
    };
    expanded_targets.iter().map(get_deps).collect::<Vec<_>>()
  };

  match direction {
    Direction::Backward => target_deps,
    Direction::Forward => {
      let mut outputs = target_deps
        .iter()
        .map(|_| (new_location_set(), new_place_set()))
        .collect::<Vec<_>>();
      for ((target_places, _), (_, places)) in expanded_targets.iter().zip(outputs.iter_mut()) {
        places.union(target_places);
      }

      let mut visitor = ForwardVisitor {
        expanded_targets,
        target_deps,
        outputs,
      };
      results.visit_reachable_with(body, &mut visitor);

      visitor.outputs
    }
  }
}

pub fn compute_dependency_ranges(
  results: &Results<'tcx, FlowAnalysis<'mir, 'tcx>>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
  spanner: &utils::HirSpanner,
) -> Vec<Vec<Range>> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;

  let source_map = tcx.sess.source_map();
  let deps = compute_dependencies(results, targets, direction);

  deps
    .into_iter()
    .map(|(locations, places)| {
      let location_spans = locations
        .iter()
        .map(|location| utils::location_to_spans(*location, body, spanner, source_map))
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

      location_spans
        .chain(place_spans)
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>()
}
