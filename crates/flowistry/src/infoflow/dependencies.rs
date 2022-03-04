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
  indexed::{
    impls::{LocationSet, PlaceSet},
    IndexedDomain,
  },
  mir::utils::SpanExt,
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
  backward: LocationSet,
  all_forward: Vec<LocationSet>,
}

impl TargetDeps {
  pub fn new(
    targets: Vec<(Place<'tcx>, Location)>,
    results: &FlowResults<'_, 'tcx>,
  ) -> Self {
    let aliases = &results.analysis.aliases;
    let location_domain = results.analysis.location_domain();
    let mut backward = LocationSet::new(location_domain);
    let mut all_forward = Vec::new();

    let expanded_targets = targets
      .into_iter()
      .flat_map(|(place, location)| {
        aliases
          .reachable_values(place)
          .iter()
          .map(move |reachable| (*reachable, location))
      })
      .collect::<Vec<_>>();
    debug!("expanded_targets={expanded_targets:#180?}");

    for (place, location) in expanded_targets {
      let state = results.state_at(location);
      backward.union(&aliases.deps(state, place));

      let mut forward = LocationSet::new(location_domain);
      forward.insert_all();
      for conflict in aliases.conflicts(place) {
        let deps = aliases.deps(state, *conflict);
        trace!("place={place:?}, conflict={conflict:?}, deps={deps:?}");
        forward.intersect(&deps);
      }
      all_forward.push(forward);
    }

    TargetDeps {
      backward,
      all_forward,
    }
  }
}

struct DepVisitor<'a, 'mir, 'tcx> {
  direction: Direction,
  target_deps: Vec<TargetDeps>,
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
    let to_check = to_check
      .iter()
      .copied()
      .map(|place| (place, state.row_set(self.analysis.aliases.normalize(place))))
      .filter(|(_, loc_deps)| !loc_deps.is_empty())
      .collect::<Vec<_>>();

    for (target_locs, (out_locs, out_places)) in
      self.target_deps.iter().zip(self.outputs.iter_mut())
    {
      for (place, loc_deps) in to_check.iter() {
        let fwd = || {
          target_locs
            .all_forward
            .iter()
            .any(|fwd_target| loc_deps.is_superset(fwd_target))
        };
        let bwd = || target_locs.backward.is_superset(loc_deps);
        let matches = match self.direction {
          Direction::Forward => fwd(),
          Direction::Backward => bwd(),
          Direction::Both => fwd() || bwd(),
        };

        if matches {
          trace!(
            "{opt_location:?}: place {:?} (deps {loc_deps:?}) / target_locs {target_locs:?}",
            place
          );
          out_places.insert(*place);

          if let Some(location) = opt_location {
            if loc_deps.contains(location)
              || (is_switch && target_locs.backward.contains(location))
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
      let location_domain = self.analysis.location_domain();
      for (place, idx) in self.analysis.aliases.all_args() {
        let mut to_check = HashSet::default();
        to_check.insert(place);
        self.visit(state, Some(*location_domain.value(idx)), to_check, false);
      }
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
  targets: Vec<Vec<(Place<'tcx>, Location)>>,
  direction: Direction,
) -> Vec<(LocationSet, PlaceSet<'tcx>)> {
  block_timer!("compute_dependencies");
  log::info!("Computing dependencies for {:?} targets", targets.len());
  debug!("targets={targets:#?}");

  let body = results.analysis.body;
  let location_domain = results.analysis.location_domain();

  let target_deps = targets
    .into_iter()
    .map(|targets| TargetDeps::new(targets, results))
    .collect::<Vec<_>>();
  debug!("target_deps={target_deps:#?}");

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
  targets: Vec<Vec<(Place<'tcx>, Location)>>,
  direction: Direction,
  spanner: &Spanner,
) -> Vec<Vec<Span>> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;

  let location_domain = results.analysis.location_domain();
  let deps = compute_dependencies(results, targets, direction);

  deps
    .into_iter()
    .map(|(locations, places)| {
      let mut location_spans = locations
        .iter()
        .flat_map(|location| {
          spanner.location_to_spans(
            *location,
            location_domain,
            EnclosingHirSpans::OuterOnly,
          )
        })
        .collect::<Vec<_>>();

      let place_spans = places
        .iter()
        .filter_map(|place| {
          let decl = &body.local_decls()[place.local];
          // We only include spans of places that are user-defined.
          // Other spans may include more code than we expect, e.g. the span
          // of the place representing the output of a match expression is the entire
          // match expression. See match_branch for an example where this matters.
          if decl.is_user_variable() || place.local == RETURN_PLACE {
            decl.source_info.span.as_local(tcx)
          } else {
            None
          }
        })
        .filter(|span| !spanner.invalid_span(*span))
        .collect::<Vec<_>>();

      trace!("Location spans: {location_spans:?}");
      trace!("Place spans: {place_spans:?}");

      location_spans.extend(place_spans);
      trace!("Before merging: {location_spans:?}");
      let merged_spans = Span::merge_overlaps(location_spans);
      trace!("After merging: {merged_spans:?}");
      merged_spans
    })
    .collect::<Vec<_>>()
}
