use std::{cell::RefCell, iter};

use either::Either;
use log::{debug, trace};
use rustc_middle::mir::{visit::Visitor, *};
use rustc_span::Span;
use rustc_utils::{
  block_timer,
  source_map::spanner::{EnclosingHirSpans, Spanner},
  BodyExt, OperandExt, SpanExt,
};

use super::{mutation::ModularMutationVisitor, FlowDomain, FlowResults};
use crate::{
  indexed::{
    impls::{LocationOrArg, LocationOrArgSet},
    RefSet,
  },
  infoflow::mutation::Mutation,
  mir::placeinfo::PlaceInfo,
};

/// Which way to look for dependencies
#[derive(Clone, Copy, Debug)]
pub enum Direction {
  /// Things affects by the source
  Forward,

  /// Things that affect the source
  Backward,

  /// Both forward and backward
  Both,
}

#[derive(Debug, Clone)]
struct TargetDeps {
  all_forward: Vec<LocationOrArgSet>,
}

impl TargetDeps {
  pub fn new<'tcx>(
    targets: &[(Place<'tcx>, LocationOrArg)],
    results: &FlowResults<'_, 'tcx>,
  ) -> Self {
    let place_info = &results.analysis.place_info;
    let location_domain = results.analysis.location_domain();
    // let mut backward = LocationSet::new(location_domain);

    let expanded_targets = targets.iter().flat_map(|(place, location)| {
      place_info
        .reachable_values(*place, Mutability::Not)
        .iter()
        .map(move |reachable| (*reachable, *location))
    });

    let all_forward = expanded_targets
      .map(|(place, location)| {
        let state_location = match location {
          LocationOrArg::Arg(..) => Location::START,
          LocationOrArg::Location(location) => location,
        };
        let state = results.state_at(state_location);
        // backward.union(&aliases.deps(state, place));

        let mut forward = LocationOrArgSet::new(location_domain);
        forward.insert_all();
        for conflict in place_info.children(place_info.normalize(place)) {
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

pub fn deps<'a, 'tcx>(
  state: &'a FlowDomain<'tcx>,
  place_info: &'a PlaceInfo<'a, 'tcx>,
  place: Place<'tcx>,
) -> LocationOrArgSet<RefSet<'a, LocationOrArg>> {
  state.row_set(place_info.normalize(place))
}

/// Computes the dependencies of a place $p$ at a location $\ell$ in a given
/// direction.
///
/// * If the direction is backward, then the dependencies are locations that influence $p$.
/// * If the direction is forward, then the dependencies are locations that are influenced by $p$.
///
/// For efficiency reasons, this function actually takes a list of list of places at locations.
/// For example, if `all_targets = [[x@L1, y@L2], [z@L3]]` then the result would be
/// `[deps(x@L1) ∪ deps(y@L2), deps(z@L3)]`.
pub fn compute_dependencies<'tcx>(
  results: &FlowResults<'_, 'tcx>,
  all_targets: Vec<Vec<(Place<'tcx>, LocationOrArg)>>,
  direction: Direction,
) -> Vec<LocationOrArgSet> {
  block_timer!("compute_dependencies");
  log::info!("Computing dependencies for {} targets", all_targets.len());
  debug!("all_targets={all_targets:#?}");

  let aliases = &results.analysis.place_info;
  let body = results.analysis.body;
  let location_domain = results.analysis.location_domain();

  let outputs = RefCell::new(
    all_targets
      .iter()
      .map(|_| LocationOrArgSet::new(location_domain))
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
      let location = LocationOrArg::Arg(arg);
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
        let deps = deps(state, aliases, place);

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
          if let Some(place) = discr.as_place() {
            check(place);
          }
        }
        _ => ModularMutationVisitor::new(&results.analysis.place_info, |_, mutations| {
          for Mutation { mutated, .. } in mutations {
            check(mutated);
          }
        })
        .visit_location(body, location),
      }
    }
  };

  let backward = || {
    for (targets, outputs) in iter::zip(&all_targets, &mut *outputs.borrow_mut()) {
      for (place, location) in targets {
        match location {
          LocationOrArg::Arg(..) => outputs.insert(location),
          LocationOrArg::Location(location) => {
            let deps = results
              .analysis
              .deps_for(results.state_at(*location), *place);
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

/// Wraps [`compute_dependencies`] by translating each [`Location`] to a corresponding
/// source [`Span`] for the location.
pub fn compute_dependency_spans<'tcx>(
  results: &FlowResults<'_, 'tcx>,
  targets: Vec<Vec<(Place<'tcx>, LocationOrArg)>>,
  direction: Direction,
  spanner: &Spanner,
) -> Vec<Vec<Span>> {
  let body = results.analysis.body;

  let all_deps = compute_dependencies(results, targets, direction);
  debug!("all_deps={all_deps:?}");

  all_deps
    .into_iter()
    .map(|deps| {
      let location_spans = deps
        .iter()
        .flat_map(|location| {
          spanner.location_to_spans(*location, body, EnclosingHirSpans::OuterOnly)
        })
        .collect::<Vec<_>>();

      let merged_spans = Span::merge_overlaps(location_spans);
      trace!("Spans: {merged_spans:?}");
      merged_spans
    })
    .collect::<Vec<_>>()
}
