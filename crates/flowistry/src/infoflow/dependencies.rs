use std::{cell::RefCell, iter};

use either::Either;
use log::{debug, trace};
use rustc_middle::mir::{visit::Visitor, *};
use rustc_span::Span;

use super::{mutation::ModularMutationVisitor, FlowResults};
use crate::{
  block_timer,
  indexed::impls::LocationSet,
  mir::utils::{BodyExt, OperandExt, SpanExt},
  source_map::{EnclosingHirSpans, Spanner},
};

#[derive(Clone, Copy, Debug)]
pub enum Direction {
  Forward,
  Backward,
  Both,
}

#[derive(Debug, Clone)]
struct TargetDeps {
  // backward: LocationSet,
  all_forward: Vec<LocationSet>,
}

impl TargetDeps {
  pub fn new<'tcx>(
    targets: &[(Place<'tcx>, Location)],
    results: &FlowResults<'_, 'tcx>,
  ) -> Self {
    let aliases = &results.analysis.aliases;
    let location_domain = results.analysis.location_domain();
    // let mut backward = LocationSet::new(location_domain);

    let expanded_targets = targets.iter().flat_map(|(place, location)| {
      aliases
        .reachable_values(*place, Mutability::Not)
        .iter()
        .map(move |reachable| (*reachable, *location))
    });

    let all_forward = expanded_targets
      .map(|(place, location)| {
        let state_location = if location_domain.location_to_local(location).is_some() {
          Location::START
        } else {
          location
        };
        let state = results.state_at(state_location);
        // backward.union(&aliases.deps(state, place));

        let mut forward = LocationSet::new(location_domain);
        forward.insert_all();
        for conflict in aliases.children(aliases.normalize(place)) {
          // conflict should already be normalized because the input to aliases.children is normalized
          let deps = state.row_set(conflict);
          trace!("place={place:?}, conflict={conflict:?}, deps={deps:?}");
          forward.intersect(&deps);
        }

        forward.insert(location);

        forward
      })
      .collect::<Vec<_>>();

    TargetDeps {
      // backward,
      all_forward,
    }
  }
}

pub fn compute_dependencies<'tcx>(
  results: &FlowResults<'_, 'tcx>,
  all_targets: Vec<Vec<(Place<'tcx>, Location)>>,
  direction: Direction,
) -> Vec<LocationSet> {
  block_timer!("compute_dependencies");
  log::info!("Computing dependencies for {} targets", all_targets.len());
  debug!("all_targets={all_targets:#?}");

  let aliases = &results.analysis.aliases;
  let body = results.analysis.body;
  let location_domain = results.analysis.location_domain();

  let outputs = RefCell::new(
    all_targets
      .iter()
      .map(|_| LocationSet::new(location_domain))
      .collect::<Vec<_>>(),
  );

  let forward = || {
    let all_target_deps = all_targets
      .iter()
      .map(|targets| TargetDeps::new(targets, results))
      .collect::<Vec<_>>();
    log::info!(
      "sub-targets: {}",
      all_target_deps
        .iter()
        .map(|deps| deps.all_forward.len())
        .sum::<usize>()
    );
    debug!("all_target_deps={all_target_deps:#?}");

    for arg in body.args_iter() {
      let location = location_domain.arg_to_location(arg);
      for (target_deps, outputs) in
        iter::zip(&all_target_deps, &mut *outputs.borrow_mut())
      {
        if target_deps
          .all_forward
          .iter()
          .any(|fwd| fwd.len() == 1 && fwd.contains(location))
        {
          outputs.insert(location);
        }
      }
    }

    for location in body.all_locations() {
      let state = results.state_at(location);
      let check = |place| {
        let deps = aliases.deps(state, place);

        for (target_deps, outputs) in
          iter::zip(&all_target_deps, &mut *outputs.borrow_mut())
        {
          if target_deps
            .all_forward
            .iter()
            .any(|fwd| deps.is_superset(fwd))
          {
            outputs.insert(location);
          }
        }
      };

      match body.stmt_at(location) {
        Either::Right(Terminator {
          kind: TerminatorKind::SwitchInt { discr, .. },
          ..
        }) => {
          if let Some(place) = discr.to_place() {
            check(place);
          }
        }
        _ => {
          ModularMutationVisitor::new(&results.analysis.aliases, |mutated, _, _, _| {
            check(mutated)
          })
          .visit_location(body, location)
        }
      }
    }
  };

  let backward = || {
    for (targets, outputs) in iter::zip(&all_targets, &mut *outputs.borrow_mut()) {
      for (place, location) in targets {
        for value in results
          .analysis
          .aliases
          .reachable_values(*place, Mutability::Not)
        {
          if location_domain.location_to_local(*location).is_some() {
            outputs.insert(location);
          } else {
            let deps = aliases.deps(results.state_at(*location), *value);
            outputs.union(&deps);
          }
        }
      }
    }
  };

  match direction {
    Direction::Forward => forward(),
    Direction::Backward => backward(),
    Direction::Both => {
      forward();
      backward();
    }
  };

  outputs.into_inner()
}

pub fn compute_dependency_spans<'tcx>(
  results: &FlowResults<'_, 'tcx>,
  targets: Vec<Vec<(Place<'tcx>, Location)>>,
  direction: Direction,
  spanner: &Spanner,
) -> Vec<Vec<Span>> {
  let body = results.analysis.body;

  let location_domain = results.analysis.location_domain();
  let all_deps = compute_dependencies(results, targets, direction);
  debug!("all_deps={all_deps:?}");

  all_deps
    .into_iter()
    .map(|deps| {
      let location_spans = deps
        .iter()
        .flat_map(|location| {
          spanner.location_to_spans(
            *location,
            location_domain,
            body,
            EnclosingHirSpans::OuterOnly,
          )
        })
        .collect::<Vec<_>>();

      let merged_spans = Span::merge_overlaps(location_spans);
      trace!("Spans: {merged_spans:?}");
      merged_spans
    })
    .collect::<Vec<_>>()
}
