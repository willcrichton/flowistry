use super::points_to::{PlacePrim, PointsToAnalysis, PointsToDomain};
use log::{debug};
use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{TyCtxt, TyKind},
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

#[derive(Clone, PartialEq, Eq)]
pub struct RelevanceDomain {
  pub places: HashSet<PlacePrim>,
  pub statement_relevant: bool,
  pub path_relevant: bool,
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let orig_path_relevant = self.path_relevant;
    let orig_len = self.places.len();

    self.places = &self.places | &other.places;
    self.path_relevant = self.path_relevant || other.path_relevant;

    orig_len != self.places.len() || orig_path_relevant != self.path_relevant
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
    let aliases = self.pointer_analysis.possible_prims(*place);
    self.places = &self.places | &aliases;
  }

  fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
    match rvalue {
      // special case for &*x == x
      // TODO: does this need to be a special case?
      Rvalue::Ref(_, _, place) => match place.projection.len() {
        0 => self.super_rvalue(rvalue, location),
        1 => {
          if let ProjectionElem::Deref = place.projection[0] {
            self.places.insert(PlacePrim::local(place.local));
          } else {
            unimplemented!("{:?}", rvalue);
          }
        }
        _ => unimplemented!("{:?}", rvalue),
      },
      _ => self.super_rvalue(rvalue, location),
    }
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
        self.analysis.contains_prim(relevant_prim, mutated_prim)
          || self.analysis.contains_prim(mutated_prim, relevant_prim)
      })
    })
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    match statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {
        self.state.statement_relevant = false;
      }
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);
    self.state.statement_relevant = false;

    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();

    let possibly_mutated = pointer_analysis.possible_prims(*place);

    debug!("checking assign {:?} = {:?} in context {:?}", place, rvalue, self.state.places);
    if self.any_relevant_mutated(&possibly_mutated) {
      debug!("  relevant assignment to {:?}", place);

      self.state.statement_relevant = true;
      self.state.path_relevant = true;

      if possibly_mutated.len() == 1 {
        let definitely_mutated = possibly_mutated.iter().next().unwrap();

        // if mutating x.0 and x is relevant, this should return false 
        // since x.0 is not in the relevant set, but it is a relevant mutation
        self.state.places.remove(definitely_mutated);

        debug!("  deleting {:?}, remaining {:?}", definitely_mutated, self.state.places);
      }

      let mut collector = CollectPlaces {
        places: HashSet::new(),
        pointer_analysis,
      };
      collector.visit_rvalue(rvalue, location);

      debug!(
        "  collected from rvalue {:?}, got places {:?}",
        rvalue, collector.places
      );

      self.state.places = &self.state.places | &collector.places;
      debug!("  new places: {:?}", self.state.places);
    }
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&location) {
      let pointer_analysis = self.analysis.pointer_analysis.borrow();
      let pointer_analysis = pointer_analysis.get();
      let prims = pointer_analysis.possible_prims(*place);
      self.state.places = &self.state.places | &prims;
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();
    self.state.statement_relevant = false;

    self.state.path_relevant = match &terminator.kind {
      TerminatorKind::SwitchInt { discr, .. } => {
        if self.state.path_relevant {
          match discr {
            Operand::Move(place) | Operand::Copy(place) => {
              let relevant_to_control = pointer_analysis.possible_prims(*place);
              self.state.places = &self.state.places | &relevant_to_control;
            }
            _ => unimplemented!("{:?}", discr),
          };
        }

        false
      }

      TerminatorKind::Call {
        func,
        args,
        destination,
        ..
      } => {
        let tcx = self.analysis.tcx;
        let func_ty = func.ty(self.analysis.body.local_decls(), tcx);
        match func_ty.kind() {
          TyKind::FnDef(_, _) => {
            let sig = func_ty.fn_sig(tcx).skip_binder();

            let any_relevant_mutable_inputs =
              sig.inputs().iter().zip(args.iter()).any(|(ty, arg)| {
                if let TyKind::Ref(_, _, Mutability::Mut) = ty.kind() {
                  match arg {
                    Operand::Move(place) => {
                      let possibly_mutated = pointer_analysis.points_to(*place);
                      self.any_relevant_mutated(&possibly_mutated)
                    }
                    _ => unimplemented!("{:?}", arg),
                  }
                } else {
                  false
                }
              });

            if any_relevant_mutable_inputs {
              self.state.statement_relevant = true;

              for arg in args.iter() {
                match arg {
                  Operand::Move(place) => {
                    let prim = PlacePrim::local(place.as_local().expect(&format!("{:?}", place)));
                    self.state.places.insert(prim);
                  }
                  _ => unimplemented!("{:?}", arg),
                }
              }
            }
          }
          _ => unimplemented!("{:?}", func_ty),
        };

        if let Some((dst_place, _)) = destination {
          let possibly_mutated = pointer_analysis.possible_prims(*dst_place);
          if self.any_relevant_mutated(&possibly_mutated) {
            let input_places: HashSet<_> = args
              .iter()
              .map(|arg| match arg {
                Operand::Move(place) => pointer_analysis.possible_prims(*place).clone(),
                _ => unimplemented!("{:?}", arg),
              })
              .flatten()
              .collect();

            self.state.places = &self.state.places | &input_places;
            self.state.statement_relevant = true;
          }
        }

        self.state.statement_relevant
      }
      _ => false,
    };
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub slice_set: SliceSet,
  pub pointer_analysis: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, PointsToAnalysis<'mir, 'tcx>>>,
  pub tcx: TyCtxt<'tcx>,
  pub body: &'mir Body<'tcx>,
  pub module: DefId,
  pub sub_places: RefCell<HashMap<PlacePrim, HashSet<PlacePrim>>>,
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub fn new(
    slice_set: SliceSet,
    tcx: TyCtxt<'tcx>,
    body_id: &BodyId,
    body: &'mir Body<'tcx>,
    results: &'a Results<'tcx, PointsToAnalysis<'mir, 'tcx>>,
  ) -> Self {
    let pointer_analysis = RefCell::new(ResultsRefCursor::new(body, &results));
    let module = tcx.parent_module(body_id.hir_id).to_def_id();
    let sub_places = RefCell::new(HashMap::new());
    RelevanceAnalysis {
      slice_set,
      pointer_analysis,
      tcx,
      body,
      module,
      sub_places,
    }
  }

  fn contains_prim(&self, parent: &PlacePrim, child: &PlacePrim) -> bool {
    self
      .sub_places
      .borrow_mut()
      .entry(parent.clone())
      .or_insert_with(|| parent.sub_places(self.body.local_decls(), self.tcx, self.module))
      .contains(child)
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
      path_relevant: true,
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
