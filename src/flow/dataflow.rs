use crate::core::{
  aliases::Aliases,
  control_dependencies::ControlDependencies,
  indexed::{IndexMatrix, IndexSet, IndexSetIteratorExt, IndexedDomain},
  indexed_impls::{build_location_domain, LocationDomain},
  utils::{self, PlaceCollector, PlaceRelation},
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

impl TransferFunction<'_, '_, 'tcx> {
  fn apply_mutation(
    &mut self,
    mutated: Place<'tcx>,
    inputs: &[Place<'tcx>],
    location: Location,
    definitely_mutated: bool,
  ) {
    let place_domain = &self.analysis.aliases.place_domain;
    let location_domain = &self.analysis.location_domain;

    let mut locations: IndexSet<Location> = inputs
      .iter()
      .map(|place| self.state.row_indices(*place))
      .flatten()
      .collect_indices(location_domain.clone());
    locations.insert(location);

    let aliases = self.analysis.aliases.loans(mutated);
    for alias in aliases.iter() {
      if definitely_mutated && aliases.len() == 1 {
        // TODO: need to clear bits, but this requires
      }

      let conflicting_places = place_domain
        .iter_enumerated()
        .filter(|(_, place)| PlaceRelation::of(**place, *alias).overlaps());
      for (_, conflict) in conflicting_places {
        self.state.union_into_row(*conflict, &locations);
      }
    }
  }
}

impl Visitor<'tcx> for TransferFunction<'a, 'b, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    self.apply_mutation(*place, &collector.places, location, true);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    let tcx = self.analysis.tcx;

    match &terminator.kind {
      TerminatorKind::SwitchInt { discr, .. } => {
        for place_idx in self.state.rows() {
          let effect_dependent = self.state.row(place_idx).any(|effect_loc| {
            self
              .analysis
              .control_dependencies
              .is_dependent(effect_loc.block, location.block)
          });

          if effect_dependent {
            if let Some(place) = utils::operand_to_place(discr) {
              if let Some(place_deps) = self.state.row_set(place).map(|s| s.to_owned()) {
                self.state.union_into_row(place_idx, &place_deps);
              }
            }
            self.state.insert(place_idx, location);
          }
        }
      }

      TerminatorKind::Call {
        /*func,*/ // TODO: deal with func
        args,
        destination,
        ..
      } => {
        let arg_places = args
          .iter()
          .filter_map(|arg| utils::operand_to_place(arg))
          .collect::<Vec<_>>();

        if let Some((dst_place, _)) = destination {
          self.apply_mutation(*dst_place, &arg_places, location, true);
        }

        let arg_mut_ptrs = arg_places
          .iter()
          .map(|place| {
            utils::interior_pointers(*place, tcx, self.analysis.body)
              .into_iter()
              .filter_map(|(_, (place, mutability))| match mutability {
                Mutability::Mut => Some(place),
                Mutability::Not => None,
              })
              .map(|place| tcx.mk_place_deref(place))
          })
          .flatten()
          .collect::<Vec<_>>();

        for mut_ptr in arg_mut_ptrs {
          self.apply_mutation(mut_ptr, &arg_places, location, false);
        }
      }

      _ => {}
    }
  }
}

pub struct FlowAnalysis<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  control_dependencies: ControlDependencies,
  aliases: Aliases<'tcx>,
  pub location_domain: Rc<LocationDomain>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    aliases: Aliases<'tcx>,
    control_dependencies: ControlDependencies,
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
    state: &mut Self::Domain,
    terminator: &Terminator<'tcx>,
    location: Location,
  ) {
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_terminator(terminator, location);
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
