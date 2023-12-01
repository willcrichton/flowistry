use rustc_abi::FieldIdx;
use rustc_hash::FxHashMap;
use rustc_hir::def_id::DefId;
use rustc_index::IndexVec;
use rustc_middle::mir::{
  AggregateKind, BasicBlock, Body, CallReturnPlaces, HasLocalDecls, Local, Location,
  Operand, Place, Rvalue, Statement, StatementKind, Terminator, TerminatorEdges,
};
use rustc_mir_dataflow::{
  fmt::DebugWithContext, lattice::FlatSet, Analysis, AnalysisDomain, Forward,
  JoinSemiLattice,
};

use super::graph::LocationOrStart;

pub type Fields<'tcx> = IndexVec<FieldIdx, Operand<'tcx>>;

#[derive(Debug, Clone)]
pub enum Value<'tcx> {
  FunctionDef { def_id: DefId, env: Fields<'tcx> },
  Tuple(Fields<'tcx>),
}

impl PartialEq for Value<'_> {
  fn eq(&self, other: &Self) -> bool {
    use Value::*;
    match (self, other) {
      (
        FunctionDef { def_id, .. },
        FunctionDef {
          def_id: other_def_id,
          ..
        },
      ) => def_id == other_def_id,
      (Tuple(places), Tuple(other_places)) => places == other_places,
      _ => false,
    }
  }
}

impl Eq for Value<'_> {}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ValueDomain<'tcx>(IndexVec<Local, FlatSet<Value<'tcx>>>);

impl<'tcx> ValueDomain<'tcx> {
  pub fn value(&self, local: Local) -> Option<&Value<'tcx>> {
    match &self.0[local] {
      FlatSet::Elem(value) => Some(value),
      _ => None,
    }
  }
}

impl<C> DebugWithContext<C> for ValueDomain<'_> {}

impl JoinSemiLattice for ValueDomain<'_> {
  fn join(&mut self, other: &Self) -> bool {
    self.0.join(&other.0)
  }
}

pub type ArgValues<'tcx> = FxHashMap<Local, Value<'tcx>>;

pub struct ValueAnalysis<'tcx> {
  arg_values: ArgValues<'tcx>,
}

impl<'tcx> ValueAnalysis<'tcx> {
  pub fn new(arg_values: ArgValues<'tcx>) -> Self {
    Self { arg_values }
  }
}

impl<'tcx> AnalysisDomain<'tcx> for ValueAnalysis<'tcx> {
  type Domain = ValueDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "ValueAnalysis";

  fn bottom_value(&self, body: &Body<'tcx>) -> Self::Domain {
    ValueDomain(IndexVec::from_elem_n(
      FlatSet::Bottom,
      body.local_decls().len(),
    ))
  }

  fn initialize_start_block(&self, _body: &Body<'tcx>, state: &mut Self::Domain) {
    for (local, value) in &self.arg_values {
      state.0[*local] = FlatSet::Elem(value.clone());
    }
  }
}

// TODO: rewrite this using the ValueAnalysis framework in rustc.
impl<'tcx> Analysis<'tcx> for ValueAnalysis<'tcx> {
  fn apply_statement_effect(
    &mut self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(box (lhs, rhs)) => {
        if let Some(local) = lhs.as_local() {
          let value = match rhs {
            Rvalue::Aggregate(box AggregateKind::Closure(def_id, _), env) => {
              Value::FunctionDef {
                def_id: *def_id,
                env: env.clone(),
              }
            }
            Rvalue::Aggregate(box AggregateKind::Tuple, fields) => {
              Value::Tuple(fields.clone())
            }
            _ => return,
          };
          state.0[local].join(&FlatSet::Elem(value));
        }
      }
      _ => {
        // todo
      }
    }
  }

  fn apply_terminator_effect<'mir>(
    &mut self,
    state: &mut Self::Domain,
    terminator: &'mir Terminator<'tcx>,
    location: Location,
  ) -> TerminatorEdges<'mir, 'tcx> {
    terminator.edges()
  }

  fn apply_call_return_effect(
    &mut self,
    state: &mut Self::Domain,
    block: BasicBlock,
    return_places: CallReturnPlaces<'_, 'tcx>,
  ) {
  }
}
