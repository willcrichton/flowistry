use crate::core::{
  aliases::Aliases,
  control_dependencies::ControlDependencies,
  indexed::{IndexMatrix, IndexSet, IndexSetIteratorExt},
  indexed_impls::{build_location_domain, LocationDomain},
  utils::PlaceCollector,
};
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyCtxt,
};
use rustc_mir::dataflow::{Analysis, AnalysisDomain, Forward};
use std::rc::Rc;

pub type FlowDomain<'tcx> = IndexMatrix<Place<'tcx>, Location>;

struct TransferFunction<'a, 'b, 'tcx> {
  analysis: &'a FlowAnalysis<'b, 'tcx>,
  state: &'a mut FlowDomain<'tcx>,
}

impl Visitor<'tcx> for TransferFunction<'a, 'b, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.state.insert(*place, location);

    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    let locations: IndexSet<Location> = collector
      .places
      .into_iter()
      .filter_map(|place| self.state.row(place).map(|set| set.iter()))
      .flatten()
      .collect_indices(self.analysis.location_domain.clone());
    self.state.union_into_row(*place, &locations);

    println!("{:?}", self.state);
  }
}

pub struct FlowAnalysis<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  control_dependencies: &'a ControlDependencies,
  aliases: &'a Aliases<'tcx>,
  location_domain: Rc<LocationDomain>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    aliases: &'a Aliases<'tcx>,
    control_dependencies: &'a ControlDependencies,
  ) -> Self {
    let location_domain = build_location_domain(body);

    FlowAnalysis {
      tcx,
      body,
      aliases,
      location_domain,
      control_dependencies,
    }
  }
}

impl AnalysisDomain<'tcx> for FlowAnalysis<'a, 'tcx> {
  type Domain = FlowDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "FlowAnalysis";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    FlowDomain::new(
      self.aliases.place_domain.clone(),
      self.location_domain.clone(),
    )
  }

  fn initialize_start_block(&self, _: &Body<'tcx>, _: &mut Self::Domain) {}
}

impl Analysis<'tcx> for FlowAnalysis<'a, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_statement(statement, location);
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
