use crate::indexed::{
  impls::{LocationDomain, LocationIndex},
  IndexedDomain,
};
use rustc_data_structures::{graph::WithSuccessors, work_queue::WorkQueue};
use rustc_index::vec::IndexVec;
use rustc_middle::{
  mir::{traversal, Body, Location},
  ty::TyCtxt,
};
use rustc_mir_dataflow::*;
use std::rc::Rc;

pub struct AnalysisResults<'tcx, A: Analysis<'tcx>> {
  pub analysis: A,
  location_domain: Rc<LocationDomain>,
  state: IndexVec<LocationIndex, A::Domain>,
}

impl<'tcx, A: Analysis<'tcx>> AnalysisResults<'tcx, A> {
  pub fn visit_reachable_with<'mir, V>(&self, body: &'mir Body<'tcx>, visitor: &mut V)
  where
    V: ResultsVisitor<'mir, 'tcx, FlowState = A::Domain>,
  {
    for (block, data) in traversal::reachable(body) {
      for statement_index in 0..=data.statements.len() {
        let location = Location {
          block,
          statement_index,
        };
        let loc_index = self.location_domain.index(&location);
        let state = &self.state[loc_index];

        if statement_index == 0 {
          visitor.visit_block_start(state, data, block);
        }

        if statement_index == data.statements.len() {
          visitor.visit_terminator_after_primary_effect(state, data.terminator(), location)
        } else {
          visitor.visit_statement_after_primary_effect(
            state,
            &data.statements[statement_index],
            location,
          )
        }
      }
    }
  }

  pub fn state_at(&self, location: Location) -> &A::Domain {
    &self.state[self.location_domain.index(&location)]
  }
}

pub fn iterate_to_fixpoint<'tcx, A: Analysis<'tcx>>(
  _tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  location_domain: Rc<LocationDomain>,
  analysis: A,
) -> AnalysisResults<'tcx, A> {
  let bottom_value = analysis.bottom_value(body);

  let num_locs = location_domain.num_real_locations();
  let mut state = IndexVec::from_elem_n(bottom_value, num_locs);

  analysis.initialize_start_block(body, &mut state[location_domain.index(&Location::START)]);

  let mut dirty_queue: WorkQueue<LocationIndex> = WorkQueue::with_none(num_locs);
  if A::Direction::is_forward() {
    for (block, data) in traversal::reverse_postorder(body) {
      for statement_index in 0..=data.statements.len() {
        dirty_queue.insert(location_domain.index(&Location {
          block,
          statement_index,
        }));
      }
    }
  }

  let blocks = body.basic_blocks();
  while let Some(loc_index) = dirty_queue.pop() {
    let location = *location_domain.value(loc_index);
    let data = &blocks[location.block];
    let is_terminator = location.statement_index == data.statements.len();

    if is_terminator {
      let terminator = data.terminator();
      analysis.apply_terminator_effect(&mut state[loc_index], terminator, location);
    } else {
      let statement = &data.statements[location.statement_index];
      analysis.apply_statement_effect(&mut state[loc_index], statement, location);
    }

    let next_locs = if is_terminator {
      body
        .successors(location.block)
        .map(|block| Location {
          block,
          statement_index: 0,
        })
        .collect::<Vec<_>>()
    } else {
      vec![location.successor_within_block()]
    };
    for next_loc in next_locs {
      let next_loc_index = location_domain.index(&next_loc);
      let (cur_state, next_state) = state.pick2_mut(loc_index, next_loc_index);
      let changed = next_state.join(cur_state);
      if changed {
        dirty_queue.insert(next_loc_index);
      }
    }
  }

  AnalysisResults {
    analysis,
    location_domain,
    state,
  }
}
