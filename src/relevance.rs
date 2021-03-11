use crate::points_to::{PlacePrim, PointsToAnalysis, PointsToDomain};
use rustc_middle::mir::{
  self,
  visit::{PlaceContext, Visitor},
  *,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Backward, JoinSemiLattice, ResultsRefCursor,
};
use std::{cell::RefCell, collections::HashSet, fmt};

pub type SliceSet = HashSet<PlacePrim>;

#[derive(Clone, PartialEq, Eq)]
pub struct RelevanceDomain {
  pub places: HashSet<PlacePrim>,
  pub relevant: bool,
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let orig_len = self.places.len();
    self.places = &self.places | &other.places;
    orig_len != self.places.len()
  }
}

impl fmt::Debug for RelevanceDomain {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "({:?}, {:?})", self.places, self.relevant)
  }
}

impl<C> DebugWithContext<C> for RelevanceDomain {}

struct CollectPlaces<'a> {
  places: HashSet<PlacePrim>,
  points_to: &'a PointsToDomain,
}

impl<'a, 'tcx> Visitor<'tcx> for CollectPlaces<'a> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    let aliases = self.points_to.points_to(*place);
    self.places = &self.places | &aliases;
  }
}

struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  analysis: &'a RelevanceAnalysis<'b, 'mir, 'tcx>,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    match statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {
        self.state.relevant = false;
      }
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let points_to = self.analysis.points_to.borrow();
    let points_to = points_to.get();

    let possibly_assigned = points_to.points_to(*place);

    let relevant_and_assigned = &self.state.places & &possibly_assigned;
    self.state.relevant = !relevant_and_assigned.is_empty();

    if self.state.relevant {
      if possibly_assigned.len() == 1 {
        assert!(self
          .state
          .places
          .remove(possibly_assigned.iter().next().unwrap()));
      }

      let mut collector = CollectPlaces {
        places: HashSet::new(),
        points_to,
      };
      collector.visit_rvalue(rvalue, location);
      self.state.places = &self.state.places | &collector.places;
    }
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    let points_to = self.analysis.points_to.borrow();
    let points_to = points_to.get();
    let prims = points_to.points_to(*place);
    let overlap = &prims & &self.analysis.slice_set;
    if !overlap.is_empty() {
      self.state.places = &self.state.places | &prims;
    }
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub slice_set: SliceSet,
  pub points_to: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, PointsToAnalysis>>,
}

impl<'a, 'mir, 'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    RelevanceDomain {
      places: HashSet::new(),
      relevant: false,
    }
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {
    // TODO?
  }
}

impl<'a, 'mir, 'tcx> Analysis<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &mir::Statement<'tcx>,
    location: Location,
  ) {
    self
      .points_to
      .borrow_mut()
      .seek_before_primary_effect(location);

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
