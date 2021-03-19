use super::points_to::{NonlocalDecls, PlacePrim, PointsToAnalysis, PointsToDomain};
use log::debug;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Backward, JoinSemiLattice, Results,
  ResultsRefCursor,
};
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
  fmt,
};

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

#[derive(Clone, PartialEq, Eq)]
pub struct RelevanceDomain {
  pub places: HashSet<PlacePrim>,
  pub statement_relevant: bool,
  pub path_relevant: Relevant,
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let orig_len = self.places.len();
    self.places = &self.places | &other.places;

    let path_relevant_joined = self.path_relevant.join(&other.path_relevant);

    orig_len != self.places.len() || path_relevant_joined
  }
}

impl fmt::Debug for RelevanceDomain {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "({:?}, {:?}, {:?})",
      self.places, self.statement_relevant, self.path_relevant
    )
  }
}

impl<C> DebugWithContext<C> for RelevanceDomain {}

struct CollectPlaces<'a> {
  places: HashSet<PlacePrim>,
  pointer_analysis: &'a PointsToDomain,
}

impl<'a, 'tcx> Visitor<'tcx> for CollectPlaces<'a> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    let (aliases, pointers) = self.pointer_analysis.possible_prims_and_pointers(*place);
    self.places = &(&self.places | &aliases) | &pointers;
  }
}

struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  analysis: &'a RelevanceAnalysis<'b, 'mir, 'tcx>,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'b, 'mir, 'tcx> TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn any_relevant_mutated(&mut self, possibly_mutated: &HashSet<PlacePrim>) -> bool {
    self.state.places.iter().any(|relevant_prim| {
      possibly_mutated.iter().any(|mutated_prim| {
        self
          .analysis
          .sub_places(relevant_prim)
          .contains(mutated_prim)
          || self
            .analysis
            .sub_places(mutated_prim)
            .contains(relevant_prim)
      })
    })
  }

  fn add_relevant(&mut self, places: &HashSet<PlacePrim>) {
    self.state.places = &self.state.places | places;
    self.state.statement_relevant = true;
    self.state.path_relevant = Relevant::Yes;
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    self.state.statement_relevant = false;
    match statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {}
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();

    let (possibly_mutated, pointers_to_possibly_mutated) =
      pointer_analysis.possible_prims_and_pointers(*place);

    debug!(
      "checking assign {:?} = {:?} in context {:?} (possibly mutated {:?})",
      place, rvalue, self.state.places, possibly_mutated
    );
    if self.any_relevant_mutated(&possibly_mutated) {
      debug!("  relevant assignment to {:?}", place);

      // TODO: better checking for strong udpates
      if possibly_mutated.len() == 1 {
        let definitely_mutated = possibly_mutated.iter().next().unwrap();

        // if mutating x.0 and x is relevant, this should return false
        // since x.0 is not in the relevant set, but it is a relevant mutation
        self.state.places.remove(definitely_mutated);

        debug!(
          "  deleting {:?}, remaining {:?}",
          definitely_mutated, self.state.places
        );
      }

      let mut collector = CollectPlaces {
        places: HashSet::new(),
        pointer_analysis,
      };
      collector.visit_rvalue(rvalue, location);

      debug!(
        "  collected from rvalue {:?}, got places {:?} and pointers {:?}",
        rvalue, collector.places, pointers_to_possibly_mutated
      );

      self.add_relevant(&collector.places);
      self.add_relevant(&pointers_to_possibly_mutated);

      debug!("  new places: {:?}", self.state.places);
    }
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&location) {
      let pointer_analysis = self.analysis.pointer_analysis.borrow();
      let pointer_analysis = pointer_analysis.get();
      let prims = pointer_analysis.possible_prims(*place);
      self.add_relevant(&prims);
    }
  }

  fn visit_local(&mut self, local: &Local, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&location) {
      let mut prims = HashSet::new();
      prims.insert(PlacePrim::local(*local));
      self.add_relevant(&prims);
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();
    self.state.statement_relevant = false;

    debug!(
      "checking terminator {:?} in context {:?}",
      terminator.kind, self.state.places
    );

    match &terminator.kind {
      TerminatorKind::SwitchInt { discr, .. } => {
        if self.state.path_relevant == Relevant::Yes {
          match discr {
            Operand::Move(place) | Operand::Copy(place) => {
              let relevant_to_control = pointer_analysis.possible_prims(*place);
              self.add_relevant(&relevant_to_control);
            }
            Operand::Constant(_) => {}
          };
        }
      }

      TerminatorKind::Call {
        args, destination, ..
      } => {
        let any_relevant_mutable_inputs = args.iter().any(|arg| match arg {
          Operand::Move(place) | Operand::Copy(place) => {
            let input_prims = pointer_analysis.possible_prims(*place);
            input_prims.iter().any(|input_prim| {
              self.state.places.iter().any(|relevant| {
                debug!(
                  "  comparing relevant {:?} and mutated {:?}",
                  relevant, input_prim
                );
                self.analysis.points_to_prim(input_prim, relevant)
                  || self.analysis.points_to_prim(relevant, input_prim)
              })
            })
          }
          Operand::Constant(_) => false,
        });

        if any_relevant_mutable_inputs {
          let relevant_inputs = args
            .iter()
            .filter_map(|arg| match arg {
              Operand::Move(place) | Operand::Copy(place) => {
                let prim = PlacePrim::local(place.as_local().expect(&format!("{:?}", place)));
                Some(prim)
              }
              Operand::Constant(_) => None,
            })
            .collect::<HashSet<_>>();
          self.add_relevant(&relevant_inputs);
        }

        if let Some((dst_place, _)) = destination {
          let possibly_mutated = pointer_analysis.possible_prims(*dst_place);
          if self.any_relevant_mutated(&possibly_mutated) {
            let input_places: HashSet<_> = args
              .iter()
              .map(|arg| match arg {
                Operand::Move(place) | Operand::Copy(place) => {
                  pointer_analysis.possible_prims(*place).clone()
                }
                Operand::Constant(_) => HashSet::new(),
              })
              .flatten()
              .collect();

            self.add_relevant(&input_places);
          }
        }
      }
      _ => {}
    };

    self.state.path_relevant = if self.state.statement_relevant {
      Relevant::Yes
    } else {
      Relevant::No
    };
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  slice_set: SliceSet,
  pointer_analysis: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, PointsToAnalysis<'mir, 'tcx>>>,
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  module: DefId,
  sub_places: RefCell<HashMap<PlacePrim, HashSet<PlacePrim>>>,
  nonlocal_decls: NonlocalDecls<'tcx>,
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub fn new(
    slice_set: SliceSet,
    tcx: TyCtxt<'tcx>,
    module: DefId,
    body: &'mir Body<'tcx>,
    results: &'a Results<'tcx, PointsToAnalysis<'mir, 'tcx>>,
    nonlocal_decls: NonlocalDecls<'tcx>,
  ) -> Self {
    let pointer_analysis = RefCell::new(ResultsRefCursor::new(body, &results));
    let sub_places = RefCell::new(HashMap::new());
    RelevanceAnalysis {
      slice_set,
      pointer_analysis,
      tcx,
      body,
      module,
      sub_places,
      nonlocal_decls,
    }
  }

  fn sub_places(&self, prim: &PlacePrim) -> HashSet<PlacePrim> {
    self
      .sub_places
      .borrow_mut()
      .entry(prim.clone())
      .or_insert_with(|| {
        prim.sub_places(
          self.body.local_decls(),
          &self.nonlocal_decls,
          self.tcx,
          self.module,
        )
      })
      .clone()
  }

  fn points_to_prim(&self, parent: &PlacePrim, child: &PlacePrim) -> bool {
    let pointer_analysis = self.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();

    self.sub_places(parent).iter().any(|parent_sub| {
      if parent_sub == child {
        true
      } else {
        pointer_analysis
          .mutably_points_to(parent_sub)
          .map(|pointed_prims| {
            pointed_prims
              .iter()
              .any(|pointed_prim| self.points_to_prim(pointed_prim, child))
          })
          .unwrap_or(false)
      }
    })
  }
}

impl<'a, 'mir, 'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    RelevanceDomain {
      places: HashSet::new(),
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
    self
      .pointer_analysis
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
    self
      .pointer_analysis
      .borrow_mut()
      .seek_before_primary_effect(location);

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
