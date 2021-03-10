use rustc_index::bit_set::{BitSet, HybridBitSet};
use rustc_middle::mir::{
  self,
  visit::{PlaceContext, Visitor},
  *,
};
use rustc_mir::dataflow::{Analysis, AnalysisDomain, Backward};
use std::collections::HashSet;

pub type SliceSet = HashSet<(Local, Location)>;

// #[derive(Clone, Debug, PartialEq, Eq)]
// struct RelevanceDomain {
//   relevant: BitSet<Local>
// }

// impl RelevanceDomain {
//   fn bottom_value<'tcx>(body: &Body<'tcx>) -> Self {
//     let relevant = BitSet::new_empty(body.local_decls().len());
//     RelevanceDomain { relevant }
//   }
// }

// impl JoinSemiLattice for RelevanceDomain {
//   fn join(&mut self, other: &Self) -> bool {
//       self.relevant.join(&other.relevant)
//   }
// }

// impl<C> DebugWithContext<C> for RelevanceDomain {
//   fn fmt_with(&self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     self.relevant.fmt_with(ctxt, f)
//   }

//   fn fmt_diff_with(&self, old: &Self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//       self.relevant.fmt_diff_with(old.relevant, ctxt, f)
//   }
// }

pub type RelevanceDomain = BitSet<Local>;

struct CollectLocals {
  locals: HybridBitSet<Local>,
}

impl<'tcx> Visitor<'tcx> for CollectLocals {
  fn visit_local(&mut self, local: &Local, _context: PlaceContext, _location: Location) {
    self.locals.insert(*local);
  }
}

struct TransferFunction<'a> {
  analysis: &'a RelevanceAnalysis,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'tcx> Visitor<'tcx> for TransferFunction<'a> {
  // fn visit_statement(&mut self, stmt: &mir::Statement<'tcx>, location: Location) {
  //   self.super_statement(stmt, location);

  //   // When we reach a `StorageDead` statement, we can assume that any pointers to this memory
  //   // are now invalid.
  //   if let StatementKind::StorageDead(local) = stmt.kind {
  //     self.gen_kill.kill(local);
  //   }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let lvalue = place.local;

    // Kill defined variables
    let defined_relevant = self.state.remove(lvalue);

    // Add used variables if killed was relevant
    if defined_relevant {
      let mut collector = CollectLocals {
        locals: HybridBitSet::new_empty(self.state.domain_size()),
      };
      collector.visit_rvalue(rvalue, location);
      self.state.union(&collector.locals);
    }
  }

  fn visit_local(&mut self, local: &Local, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&(*local, location)) {
      self.state.insert(*local);
    }
  }

  fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_rvalue(rvalue, location);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    self.super_terminator(terminator, location);
  }
}

pub struct RelevanceAnalysis {
  pub slice_set: SliceSet,
}

impl<'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnslysis";

  fn bottom_value(&self, body: &mir::Body<'tcx>) -> Self::Domain {
    BitSet::new_empty(body.local_decls().len())
    //RelevanceDomain::bottom_value(body)
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {
    // TODO?
  }
}

impl<'tcx> Analysis<'tcx> for RelevanceAnalysis {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &mir::Statement<'tcx>,
    location: Location,
  ) {
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
