use crate::points_to::{PlacePrim, PointsToAnalysis, PointsToDomain};
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{subst::Subst, ParamEnv, TyCtxt, TyKind},
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Backward, JoinSemiLattice, ResultsRefCursor,
};
use std::{cell::RefCell, collections::HashSet, fmt};
use log::debug;

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

    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();

    let possibly_assigned = pointer_analysis.possible_prims(*place);

    let relevant_and_assigned = &self.state.places & &possibly_assigned;
    self.state.relevant = !relevant_and_assigned.is_empty();

    if self.state.relevant {
      debug!("relevant assignment to {:?}", place);

      if possibly_assigned.len() == 1 {
        let definitely_assigned = possibly_assigned.iter().next().unwrap();
        debug!("deleting {:?}", definitely_assigned);
        self.state.places = &self.state.places - &self.analysis.sub_places(definitely_assigned);
      }

      let mut collector = CollectPlaces {
        places: HashSet::new(),
        pointer_analysis,
      };
      collector.visit_rvalue(rvalue, location);

      debug!(
        "collected from rvalue {:?}, got places {:?}",
        rvalue, collector.places
      );

      let newly_relevant = collector
        .places
        .into_iter()
        .map(|place| {
          let sub_places = self.analysis.sub_places(&place);
          //println!("prim {:?}, sub_places {:?}", place, sub_places);
          sub_places
        })
        .fold(HashSet::new(), |s1, s2| &s1 | &s2);
      self.state.places = &self.state.places | &newly_relevant;
    }
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();
    let prims = pointer_analysis.possible_prims(*place);
    let overlap = &prims & &self.analysis.slice_set;
    if !overlap.is_empty() {
      self.state.places = &self.state.places | &prims;
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    let pointer_analysis = self.analysis.pointer_analysis.borrow();
    let pointer_analysis = pointer_analysis.get();

    match &terminator.kind {
      TerminatorKind::Call { func, args, destination, .. } => {
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
                      let relevant_and_assigned = &self.state.places & &possibly_mutated;
                      !relevant_and_assigned.is_empty()
                    }
                    _ => unimplemented!("{:?}", arg),
                  }
                } else {
                  false
                }
              });

            if any_relevant_mutable_inputs {
              self.state.relevant = true;
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
          let prims = pointer_analysis.possible_prims(*dst_place);
          println!("dst_place {:?} (prims {:?})", dst_place, prims);
          let overlap = &self.state.places & &prims;
          if !overlap.is_empty() {
            self.state.places = &self.state.places | &prims;
            self.state.relevant = true;
          }
        }
      }
      _ => {}
    }
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub slice_set: SliceSet,
  pub pointer_analysis: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, PointsToAnalysis<'mir, 'tcx>>>,
  pub tcx: TyCtxt<'tcx>,
  pub body: &'mir Body<'tcx>,
  pub module: DefId,
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  fn sub_places(&self, place: &PlacePrim) -> HashSet<PlacePrim> {
    place.sub_places(self.body.local_decls(), self.tcx, self.module)
  }
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
