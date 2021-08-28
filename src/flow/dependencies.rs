use super::dataflow::{FlowAnalysis, FlowDomain};
use crate::{
  backward_slicing::Range,
  core::{indexed::IndexSet, utils},
};
use rustc_middle::mir::{
  visit::{PlaceContext, Visitor},
  *,
};
use rustc_mir::dataflow::{Results, ResultsVisitor};
use rustc_span::Span;

// struct FindDependenciesInPlace<'a, 'b, 'mir, 'tcx> {
//   state: &'a FlowDomain<'tcx>,
//   visitor: &'a mut FindDependencies<'b, 'mir, 'tcx>,
// }

pub enum Direction {
  Forward,
  Backward,
}

struct FindDependencies<'a, 'mir, 'tcx> {
  analysis: &'a FlowAnalysis<'mir, 'tcx>,
  targets: Vec<IndexSet<Location>>,
  relevant_locs: Vec<IndexSet<Location>>,
  relevant_args: Vec<Vec<Span>>,
  direction: Direction,
}

impl FindDependencies<'_, '_, 'tcx> {
  fn check(&mut self, place: Place<'tcx>, state: &FlowDomain<'tcx>) -> Vec<usize> {
    match state.row_set(place) {
      Some(place_deps) => self
        .targets
        .iter()
        .enumerate()
        .filter(|(_, target)| match self.direction {
          Direction::Forward => place_deps.is_superset(target),
          Direction::Backward => target.is_superset(&place_deps),
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>(),
      None => vec![],
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for FindDependencies<'_, 'mir, 'tcx> {
  type FlowState = FlowDomain<'tcx>;

  fn visit_block_start(
    &mut self,
    state: &Self::FlowState,
    _block_data: &'mir BasicBlockData<'tcx>,
    block: BasicBlock,
  ) {
    if block == Location::START.block {
      for arg in self.analysis.body.args_iter() {
        let arg_place = utils::local_to_place(arg, self.analysis.tcx);
        for i in self.check(arg_place, state) {
          let arg_span = self.analysis.body.local_decls()[arg].source_info.span;
          self.relevant_args[i].push(arg_span);
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
    match &statement.kind {
      StatementKind::Assign(box (mutated, _)) => {
        for i in self.check(*mutated, state) {
          self.relevant_locs[i].insert(location);
        }
      }
      _ => {}
    }
    
  }
}

pub fn compute_dependencies(
  results: &Results<'tcx, FlowAnalysis<'mir, 'tcx>>,
  targets: Vec<IndexSet<Location>>,
  direction: Direction,
) -> Vec<(IndexSet<Location>, Vec<Span>)> {
  let analysis = &results.analysis;
  let n = targets.len();
  let mut visitor = FindDependencies {
    analysis,
    targets,
    relevant_locs: vec![IndexSet::new(analysis.location_domain().clone()); n],
    relevant_args: vec![vec![]; n],
    direction,
  };
  results.visit_reachable_with(analysis.body, &mut visitor);
  visitor
    .relevant_locs
    .into_iter()
    .zip(visitor.relevant_args.into_iter())
    .collect::<Vec<_>>()
}

pub fn compute_dependency_ranges(
  results: &Results<'tcx, FlowAnalysis<'mir, 'tcx>>,
  targets: Vec<IndexSet<Location>>,
  direction: Direction,
  spanner: &utils::HirSpanner,
) -> Vec<Vec<Range>> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;
  let deps = compute_dependencies(results, targets, direction);

  let source_map = tcx.sess.source_map();
  deps
    .into_iter()
    .map(|(locs, args)| {
      locs
        .iter()
        .filter_map(|location| {
          let mir_span = body.source_info(*location).span;
          spanner.find_enclosing_hir_span(mir_span)
        })
        .chain(args.into_iter())
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>()
    })
    .collect::<Vec<_>>()
}
