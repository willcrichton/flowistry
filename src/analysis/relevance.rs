use crate::config::EvalMode;

use super::{
  aliases::Aliases,
  borrow_ranges::BorrowRanges,
  place_index::PlaceSet,
  place_index::{PlaceIndex, PlaceIndices},
};
use log::debug;
use rustc_data_structures::graph::dominators::Dominators;
use rustc_middle::{
  mir::{
    self,
    borrows::BorrowSet,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{TyCtxt, TyKind},
};
use rustc_mir::{
  borrow_check::{borrow_conflicts_with_place, AccessDepth, PlaceConflictBias},
  dataflow::{
    fmt::{DebugWithAdapter, DebugWithContext},
    Analysis, AnalysisDomain, Backward, JoinSemiLattice, Results, ResultsRefCursor,
  },
};
use std::{cell::RefCell, collections::HashSet, fmt};

pub type SliceSet = HashSet<Location>;

// Previous strategy of representing path relevance as a bool didn't seem to work out
// with out dataflow framework handles start/exit states and join? Adding a third unknown
// state as bottom rather than defaulting to false seemed to work
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Relevant {
  Yes,
  No,
  Unknown,
}

impl JoinSemiLattice for Relevant {
  fn join(&mut self, other: &Self) -> bool {
    let state = match (*self, *other) {
      (Relevant::Yes, _) | (_, Relevant::Yes) => Relevant::Yes,
      (Relevant::No, _) | (_, Relevant::No) => Relevant::No,
      _ => Relevant::Unknown,
    };
    if state != *self {
      *self = state;
      true
    } else {
      false
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelevanceDomain {
  pub places: PlaceSet,
  pub statement_relevant: bool,
  pub path_relevant: Relevant,
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let places_changed = self.places.join(&other.places);
    let path_relevant_changed = self.path_relevant.join(&other.path_relevant);
    places_changed || path_relevant_changed
  }
}

impl DebugWithContext<RelevanceAnalysis<'_, '_, '_>> for RelevanceDomain {
  fn fmt_with(
    &self,
    ctxt: &RelevanceAnalysis<'_, '_, '_>,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    self.places.fmt_with(ctxt.place_indices, f)?;
    write!(
      f,
      " {:?}, {:?}",
      self.statement_relevant, self.path_relevant
    )
  }
}

struct CollectPlaceIndices<'a, 'tcx> {
  places: PlaceSet,
  place_indices: &'a PlaceIndices<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for CollectPlaceIndices<'a, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.places.insert(self.place_indices.index(place));
  }
}

struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  analysis: &'a RelevanceAnalysis<'b, 'mir, 'tcx>,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'b, 'mir, 'tcx> TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn add_relevant(&mut self, places: &PlaceSet) {
    self.state.places.union(places);
    self.state.statement_relevant = true;

    let current_block = self.analysis.current_block.borrow();
    let preds = &self.analysis.body.predecessors()[*current_block];
    let dominates_all_preds = preds.iter().all(|pred_bb| {
      self
        .analysis
        .post_dominators
        .is_dominated_by(*pred_bb, *current_block)
    });
    if !dominates_all_preds {
      self.state.path_relevant = Relevant::Yes;
    }
  }

  fn any_relevant(&mut self, possibly_mutated: &PlaceSet) -> bool {
    possibly_mutated.iter().any(|mutated_place| {
      self.state.places.iter().any(|relevant_place| {
        self
          .analysis
          .place_index_is_part(mutated_place, relevant_place)
          || self
            .analysis
            .place_index_is_part(relevant_place, mutated_place)
      })
    })
  }

  fn check_mutation(&mut self, place: Place<'tcx>, input_places: &PlaceSet) {
    macro_rules! fmt_places {
      ($places:expr) => {
        DebugWithAdapter {
          this: &$places,
          ctxt: self.analysis.place_indices,
        }
      };
    }

    debug!("checking {:?}", place);
    let (possibly_mutated, pointers_to_mutated) = self.analysis.places_and_pointers(place);
    debug!(
      "  relevant {:?}, possibly_mutated {:?}, pointers_to_mutated {:?}",
      fmt_places!(self.state.places),
      fmt_places!(possibly_mutated),
      fmt_places!(pointers_to_mutated)
    );

    let any_relevant_mutated = self.any_relevant(&possibly_mutated);

    if any_relevant_mutated {
      // strong update
      if possibly_mutated.count() == 1 {
        debug!("  deleting {:?}", fmt_places!(possibly_mutated));
        let definitely_mutated = possibly_mutated.iter().next().unwrap();
        let to_delete = self
          .state
          .places
          .iter()
          .filter(|relevant_place| {
            self
              .analysis
              .place_index_is_part(*relevant_place, definitely_mutated)
          })
          .collect::<Vec<_>>();
        for i in to_delete {
          self.state.places.remove(i);
        }
        debug!("  after deletion: {:?}", fmt_places!(self.state.places));
      }

      debug!(
        "  adding relevant places {:?} and pointers to possibly mutated {:?}",
        fmt_places!(input_places),
        fmt_places!(pointers_to_mutated)
      );
      self.add_relevant(input_places);
      self.add_relevant(&pointers_to_mutated);
    }
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    self.state.statement_relevant = false;
    match &statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {}
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let mut collector = CollectPlaceIndices {
      places: self.analysis.place_indices.empty_set(),
      place_indices: self.analysis.place_indices,
    };
    collector.visit_rvalue(rvalue, location);

    self.check_mutation(*place, &collector.places);
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&location) {
      let mut indices = self.analysis.place_indices.empty_set();
      indices.insert(self.analysis.place_indices.index(place));
      self.add_relevant(&indices);
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    // Ignore FalseEdge nodes since they can trip up the soundness of path_relevance wrt the post-dominator tree.
    // eg a FalseEdge node always post-dominates a while-loop condition so it would set path_relevant to false,
    // but while-body state gets propagated through the FalseEdge node which cause the while-condition to be incorrectly
    // marked as irrelevant when it is relevant to the while-body
    match &terminator.kind {
      TerminatorKind::FalseEdge { .. }  => { return; }
      _ => {}
    }

    self.state.statement_relevant = false;

    debug!(
      "checking terminator {:?} in context {:?}",
      terminator.kind, self.state.places
    );

    match &terminator.kind {
      TerminatorKind::Call {
        args, destination, ..
      } => {
        debug!("A");
        let input_places = args
          .iter()
          .filter_map(|arg| match arg {
            Operand::Move(place) | Operand::Copy(place) => Some(*place),
            Operand::Constant(_) => None,
          })
          .collect::<Vec<_>>();
        let input_places_set = self.analysis.place_indices.vec_to_set(&input_places);

        let any_mut_ptrs_to_relevant = input_places.iter().any(|arg| {
          let (places, _) = self.analysis.places_and_pointers(*arg);
          self.any_relevant(&places)
        });

        if any_mut_ptrs_to_relevant {
          self.add_relevant(&input_places_set);
        }

        if let Some((dst, _)) = destination {
          self.check_mutation(*dst, &input_places_set);
        }
      }

      TerminatorKind::SwitchInt { discr, .. } => {
        if self.state.path_relevant == Relevant::Yes {
          match discr {
            Operand::Move(place) | Operand::Copy(place) => {
              let (places, _) = self.analysis.places_and_pointers(*place);
              self.add_relevant(&places);
            }
            Operand::Constant(_) => {}
          }
        }
      }

      _ => {}
    }

    self.state.path_relevant = if self.state.statement_relevant {
      Relevant::Yes
    } else {
      Relevant::No
    };
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  slice_set: SliceSet,
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  borrow_ranges: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, BorrowRanges<'mir, 'tcx>>>,
  place_indices: &'a PlaceIndices<'tcx>,
  aliases: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, Aliases<'a, 'mir, 'tcx>>>,
  post_dominators: Dominators<BasicBlock>,
  current_block: RefCell<BasicBlock>,
  eval_mode: EvalMode
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub fn new(
    slice_set: SliceSet,
    tcx: TyCtxt<'tcx>,
    body: &'mir Body<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    borrow_ranges: &'a Results<'tcx, BorrowRanges<'mir, 'tcx>>,
    place_indices: &'a PlaceIndices<'tcx>,
    aliases: &'a Results<'tcx, Aliases<'a, 'mir, 'tcx>>,
    post_dominators: Dominators<BasicBlock>,
    eval_mode: EvalMode
  ) -> Self {
    let borrow_ranges = RefCell::new(ResultsRefCursor::new(body, &borrow_ranges));
    let aliases = RefCell::new(ResultsRefCursor::new(body, aliases));
    let current_block = RefCell::new(body.basic_blocks().indices().next().unwrap());
    RelevanceAnalysis {
      slice_set,
      tcx,
      body,
      borrow_set,
      borrow_ranges,
      place_indices,
      aliases,
      post_dominators,
      current_block,
      eval_mode
    }
  }

  fn place_index_is_part(&self, part_place: PlaceIndex, whole_place: PlaceIndex) -> bool {
    self.place_is_part(
      self.place_indices.lookup(part_place),
      self.place_indices.lookup(whole_place),
    )
  }

  fn place_is_part(&self, part_place: Place<'tcx>, whole_place: Place<'tcx>) -> bool {
    // borrow_conflicts_with_place considers it a bug if borrow_place is behind immutable deref, so special case this
    // see places_conflict.rs:234-236
    {
      let access_place = part_place;
      let borrow_place = whole_place;
      if borrow_place.projection.len() > access_place.projection.len() {
        for (i, _elem) in borrow_place.projection[access_place.projection.len()..]
          .iter()
          .enumerate()
        {
          let proj_base = &borrow_place.projection[..access_place.projection.len() + i];
          let base_ty = Place::ty_from(borrow_place.local, proj_base, self.body, self.tcx).ty;
          if let TyKind::Ref(_, _, Mutability::Not) = base_ty.kind() {
            return false;
          }
        }
      }
    }

    borrow_conflicts_with_place(
      self.tcx,
      self.body,
      whole_place,
      BorrowKind::Mut {
        allow_two_phase_borrow: true,
      },
      part_place.as_ref(),
      AccessDepth::Deep,
      PlaceConflictBias::Overlap,
    )
  }

  fn places_and_pointers(&self, place: Place<'tcx>) -> (PlaceSet, PlaceSet) {
    let borrow_ranges = self.borrow_ranges.borrow();
    let borrow_ranges = borrow_ranges.get();

    let aliases = self.aliases.borrow();
    let aliases = aliases.get();

    let mut places = self.place_indices.empty_set();
    let mut pointers = self.place_indices.empty_set();
    places.insert(self.place_indices.index(&place));

    for i in borrow_ranges.iter() {
      let borrow = &self.borrow_set[i];

      // Ignore immutable borrows
      if self.eval_mode == EvalMode::Standard && borrow.kind.to_mutbl_lossy() != Mutability::Mut {
        continue;
      }

      // fixed an issue in progs like 
      //   _1 = &mut 2; _2 = &mut (*1); 
      // where places_and_pointers((*1)) is queried, and (*1) conflicts with _2 and recurse on (*1)
      // TODO: is exact equality the correct guard or something more general?
      if borrow.borrowed_place == place {
        continue;
      }

      let borrow_aliases = aliases.iter_enumerated().filter_map(|(local, borrows)| {
        if borrows.contains(i) {
          Some(local)
        } else {
          None
        }
      });

      let part_of_aliases = borrow_aliases.filter(|alias| {
        self.place_is_part(
          place,
          Place {
            local: *alias,
            projection: self.tcx.intern_place_elems(&[]),
          },
        )
      }).collect::<Vec<_>>();

      if self.place_is_part(place, borrow.assigned_place) || part_of_aliases.len() > 0 {
        places.insert(self.place_indices.index(&borrow.borrowed_place));
        pointers.insert(self.place_indices.index(&place));
        pointers.insert(self.place_indices.index(&borrow.assigned_place));
        
        debug!("place {:?} recursing on borrow {:?}", place, borrow);
        if part_of_aliases.len() > 0 {
          debug!("  because part of aliases {:?}", part_of_aliases)
        } else {
          debug!("  because part of borrow {:?}", borrow.assigned_place)
        };
        
        let (sub_places, sub_pointers) = self.places_and_pointers(borrow.borrowed_place);
        places.union(&sub_places);
        pointers.union(&sub_pointers);
      }
    }

    (places, pointers)
  }

  fn seek_results(&self, location: Location) {
    *self.current_block.borrow_mut() = location.block;
    self
      .borrow_ranges
      .borrow_mut()
      .seek_before_primary_effect(location);
    self
      .aliases
      .borrow_mut()
      .seek_before_primary_effect(location);
  }
}

impl<'a, 'mir, 'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    RelevanceDomain {
      places: self.place_indices.empty_set(),
      statement_relevant: false,
      path_relevant: Relevant::Unknown,
    }
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {}
}

impl<'a, 'mir, 'tcx> Analysis<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &mir::Statement<'tcx>,
    location: Location,
  ) {
    self.seek_results(location);
    TransferFunction {
      state,
      analysis: self,
    }
    .visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &mir::Terminator<'tcx>,
    location: Location,
  ) {
    self.seek_results(location);
    TransferFunction {
      state,
      analysis: self,
    }
    .visit_terminator(terminator, location);
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _func: &mir::Operand<'tcx>,
    _args: &[mir::Operand<'tcx>],
    _return_place: mir::Place<'tcx>,
  ) {
  }
}
