use rustc_middle::{mir::*, ty::TyCtxt};
use rustc_mir::dataflow::{Analysis, AnalysisDomain, Forward};

use crate::core::{
  aliases::Aliases, control_dependencies::ControlDependencies, indexed::IndexMatrix,
};

pub type FlowDomain<'tcx> = IndexMatrix<Place<'tcx>, Location>;

pub struct FlowAnalysis<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  control_dependencies: &'a ControlDependencies,
  aliases: &'a Aliases<'tcx>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    aliases: &'a Aliases<'tcx>,
    control_dependencies: &'a ControlDependencies,
  ) -> Self {
    FlowAnalysis {
      tcx,
      body,
      aliases,
      control_dependencies,
    }
  }
}

impl AnalysisDomain<'tcx> for FlowAnalysis<'a, 'tcx> {
  type Domain = FlowDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "FlowAnalysis";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    // FlowDomain::new(self.aliases.place_domain.clone())
    todo!()
  }

  fn initialize_start_block(&self, _: &Body<'tcx>, _: &mut Self::Domain) {}
}

impl Analysis<'tcx> for FlowAnalysis<'a, 'tcx> {
  fn apply_statement_effect(
    &self,
    _state: &mut Self::Domain,
    _statement: &Statement<'tcx>,
    _location: Location,
  ) {
    // todo!()
  }

  fn apply_terminator_effect(
    &self,
    _state: &mut Self::Domain,
    _terminator: &Terminator<'tcx>,
    _location: Location,
  ) {
    // todo!()
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _func: &Operand<'tcx>,
    _args: &[Operand<'tcx>],
    _return_place: Place<'tcx>,
  ) {
  }
}
