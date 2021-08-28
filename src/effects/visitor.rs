use crate::{
  core::{
    indexed::IndexSetIteratorExt,
    indexed_impls::{LocationSet, PlaceSet},
    utils::{self, PlaceRelation},
  },
  flow,
};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_middle::mir::*;
use rustc_mir::dataflow::ResultsVisitor;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum EffectKind {
  MutArg(usize),
  Return,
}

pub struct FindEffects<'a, 'mir, 'tcx> {
  analysis: &'a flow::FlowAnalysis<'mir, 'tcx>,
  mut_args: PlaceSet<'tcx>,
  pub effects: HashMap<EffectKind, Vec<(Location, LocationSet)>>,
}

impl FindEffects<'a, 'mir, 'tcx> {
  pub fn new(analysis: &'a flow::FlowAnalysis<'mir, 'tcx>) -> Self {
    let tcx = analysis.tcx;
    let body = analysis.body;
    let mut_args = body
      .args_iter()
      .map(|local| {
        let place = utils::local_to_place(local, tcx);
        utils::interior_pointers(place, tcx, body)
          .into_values()
          .filter(|(_, mutability)| *mutability == Mutability::Mut)
          .map(|(place, _)| tcx.mk_place_deref(place))
      })
      .flatten()
      .collect_indices(analysis.place_domain().clone());

    FindEffects {
      analysis,
      mut_args,
      effects: HashMap::default(),
    }
  }
}

impl ResultsVisitor<'mir, 'tcx> for FindEffects<'_, 'mir, 'tcx> {
  type FlowState = flow::FlowDomain<'tcx>;

  fn visit_statement_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(box (mutated, _input)) => {
        let aliases = self.analysis.aliases.loans(*mutated);
        let mutated = self
          .mut_args
          .iter()
          .filter(|arg| {
            aliases
              .iter()
              .any(|alias| PlaceRelation::of(**arg, *alias).overlaps())
          })
          .collect::<Vec<_>>();

        if mutated.len() > 0 {
          for arg in mutated {
            let arg_index = arg.local.as_usize() - 1;
            let kind = EffectKind::MutArg(arg_index);
            let deps = state.row_set(*arg).unwrap().to_owned();
            self
              .effects
              .entry(kind)
              .or_insert_with(Vec::new)
              .push((location, deps));
          }
        }
      }
      _ => {}
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    state: &Self::FlowState,
    terminator: &'mir Terminator<'tcx>,
    location: Location,
  ) {
    match &terminator.kind {
      TerminatorKind::Return => {
        let return_place = utils::local_to_place(RETURN_PLACE, self.analysis.tcx);
        let deps = state.row_set(return_place).unwrap().to_owned();

        // Span of MIR return statements is assigned to the end "}" of a function, so
        // instead we search for the closest assignment to _0
        let closest_assign = deps
          .iter()
          .cloned()
          .filter(|loc| loc.block == location.block)
          .max_by_key(|loc| loc.statement_index);

        self
          .effects
          .entry(EffectKind::Return)
          .or_insert_with(Vec::new)
          .push((closest_assign.unwrap(), deps));
      }
      _ => {}
    }
  }
}
