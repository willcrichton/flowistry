use super::dataflow::{FlowAnalysis, FlowDomain};
use crate::core::{
  config::Range,
  indexed::{IndexSet, IndexSetIteratorExt},
  indexed_impls::{arg_location, location_arg, LocationSet},
  utils::{self},
};
use log::debug;
use rustc_data_structures::graph::{iterate::reverse_post_order, WithPredecessors};
use rustc_index::bit_set::{BitSet, HybridBitSet};
use rustc_middle::mir::*;
use rustc_mir::dataflow::{Results, ResultsCursor, ResultsVisitor};
use rustc_span::Span;
use smallvec::{smallvec, SmallVec};

#[derive(Clone, Copy, Debug)]
pub enum Direction {
  Forward,
  Backward,
}

struct FindDependencies<'a, 'mir, 'tcx> {
  analysis: &'a FlowAnalysis<'mir, 'tcx>,
  targets: Vec<LocationSet>,
  relevant_locations: Vec<LocationSet>,
  relevant_locals: Vec<BitSet<Local>>,
  direction: Direction,
}

impl FindDependencies<'_, '_, 'tcx> {
  fn check(&mut self, place: Place<'tcx>, state: &FlowDomain<'tcx>, location: Location) {
    let aliases = self.analysis.aliases.loans(place);
    let alias_deps = aliases.iter().filter_map(|alias| state.row_set(*alias));

    for deps in alias_deps {
      let direction = self.direction;
      let target_idxs = self
        .targets
        .iter()
        .enumerate()
        .filter(|(_, target)| match direction {
          Direction::Forward => deps.is_superset(target),
          Direction::Backward => target.is_superset(&deps),
        });

      for (i, _) in target_idxs {
        self.relevant_locations[i].insert(location);
        self.relevant_locals[i].insert(place.local);
      }
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for FindDependencies<'_, 'mir, 'tcx> {
  type FlowState = FlowDomain<'tcx>;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(box (mutated, _)) => {
        self.check(*mutated, state, location);
      }
      _ => {}
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    terminator: &'mir Terminator<'tcx>,
    location: Location,
  ) {
    match &terminator.kind {
      TerminatorKind::SwitchInt { discr, .. } => {
        if let Some(place) = utils::operand_to_place(discr) {
          self.check(place, state, location);
        }
      }

      TerminatorKind::Call {
        args, destination, ..
      } => {
        if let Some((dst_place, _)) = destination {
          self.check(*dst_place, state, location);
        }

        let arg_mut_ptrs = utils::arg_mut_ptrs(
          &utils::arg_places(&args),
          self.analysis.tcx,
          self.analysis.body,
        );
        for mut_ptr in arg_mut_ptrs {
          self.check(mut_ptr, state, location);
        }
      }

      TerminatorKind::DropAndReplace { place, .. } => {
        self.check(*place, state, location);
      }

      _ => {}
    }
  }
}

pub fn compute_dependencies(
  results: &Results<'tcx, FlowAnalysis<'mir, 'tcx>>,
  targets: Vec<(Place<'tcx>, Location)>,
  direction: Direction,
) -> Vec<(IndexSet<Location>, Vec<Span>)> {
  let analysis = &results.analysis;
  let body = analysis.body;
  let mut cursor = ResultsCursor::new(body, results);
  let targets = targets
    .into_iter()
    .filter_map(|(place, location)| {
      cursor.seek_after_primary_effect(location);
      cursor.get().row_set(place).map(|set| set.to_owned())
    })
    .collect::<Vec<_>>();
  debug!("Targets: {:?}", targets);

  let n = targets.len();
  let mut finder = FindDependencies {
    analysis: &results.analysis,
    targets,
    relevant_locations: vec![LocationSet::new(analysis.location_domain().clone()); n],
    relevant_locals: vec![BitSet::new_empty(body.local_decls().len()); n],
    direction,
  };
  results.visit_reachable_with(body, &mut finder);

  let _arg_locs: LocationSet = body
    .args_iter()
    .map(|arg| arg_location(arg, body))
    .collect_indices(analysis.location_domain().clone());

  for (i, target) in finder.targets.iter().enumerate() {
    for loc in target.iter() {
      if loc.block.as_usize() == body.basic_blocks().len() {
        let arg = location_arg(*loc, body);
        finder.relevant_locals[i].insert(arg);
      }
    }
  }

  finder
    .relevant_locations
    .into_iter()
    .zip(finder.relevant_locals.into_iter().map(|locals| {
      locals
        .iter()
        .map(|local| body.local_decls()[local].source_info.span)
        .collect::<Vec<_>>()
    }))
    .collect()
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
  debug!("deps: {:#?}", deps);

  let location_to_spans = |location: Location| -> SmallVec<[Span; 4]> {
    let mut mir_spans: SmallVec<[Span; 2]> = smallvec![body.source_info(location).span];
    let block = &body.basic_blocks()[location.block];
    if location.statement_index == block.statements.len() {
      match block.terminator().kind {
        TerminatorKind::SwitchInt { .. } => {
          let mut reachable_set = HybridBitSet::new_empty(body.basic_blocks().len());
          for block in reverse_post_order(body, location.block) {
            reachable_set.insert(block);
          }

          for pred in WithPredecessors::predecessors(body, location.block)
            .filter(|pred| reachable_set.contains(*pred))
          {
            let loop_span = body.source_info(body.terminator_loc(pred)).span;
            mir_spans.push(loop_span);
          }
        }
        _ => {}
      }
    }

    // source_callsite gets the top-level source location if span is
    // from a macro expansion
    for span in mir_spans.iter_mut() {
      *span = span.source_callsite();
    }

    let format_spans = |spans: &[Span]| -> String {
      spans
        .iter()
        .map(|span| utils::span_to_string(*span, source_map))
        .collect::<Vec<_>>()
        .join(" -- ")
    };

    let hir_spans = mir_spans
      .clone()
      .into_iter()
      .map(|mir_span| spanner.find_enclosing_hir_span(mir_span).into_iter())
      .flatten()
      .collect::<SmallVec<[Span; 4]>>();

    debug!(
      "Location {:?} ({})\n  has MIR spans:\n  {}\n  and HIR spans:\n  {}",
      location,
      utils::location_to_string(location, body),
      format_spans(&mir_spans),
      format_spans(&hir_spans)
    );

    hir_spans
  };

  deps
    .into_iter()
    .map(|(locs, args)| {
      locs
        .iter()
        .map(|loc| location_to_spans(*loc).into_iter())
        .flatten()
        .chain(args.into_iter())
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>()
}
