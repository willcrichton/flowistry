use crate::{
  core::{
    indexed::{IndexSetIteratorExt, IndexedDomain},
    indexed_impls::{PlaceIndex, PlaceSet},
    utils,
  },
  flow,
};

use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_middle::mir::*;
use rustc_mir::dataflow::ResultsVisitor;

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum EffectKind {
  MutArg(PlaceIndex),
  Return,
}

pub struct FindEffects<'a, 'mir, 'tcx> {
  analysis: &'a flow::FlowAnalysis<'mir, 'tcx>,
  mut_args: PlaceSet<'tcx>,
  pub effects: HashMap<EffectKind, HashSet<(Place<'tcx>, Location)>>,
}

impl FindEffects<'a, 'mir, 'tcx> {
  pub fn new(analysis: &'a flow::FlowAnalysis<'mir, 'tcx>) -> Self {
    let tcx = analysis.tcx;
    let body = analysis.body;
    let mut_args = body
      .args_iter()
      .map(|local| {
        let place = utils::local_to_place(local, tcx);
        utils::interior_pointers(place, tcx, body, analysis.def_id)
          .into_values()
          .filter(|(_, mutability)| *mutability == Mutability::Mut)
          .map(|(place, _)| {
            let deref_place = tcx.mk_place_deref(place);
            utils::interior_places(deref_place, tcx, body, analysis.def_id).into_iter()
          })
          .flatten()
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
    _state: &Self::FlowState,
    statement: &'mir Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(box (mutated, _input)) => {
        if mutated.local == RETURN_PLACE {
          self
            .effects
            .entry(EffectKind::Return)
            .or_default()
            .insert((*mutated, location));
        } else {
          let mut aliases = self.analysis.aliases.aliases(*mutated);
          aliases.intersect(&self.mut_args);

          let mut it = aliases.iter();
          if let Some(arg) = it.next() {
            let kind = EffectKind::MutArg(self.analysis.place_domain().index(arg));
            self
              .effects
              .entry(kind)
              .or_default()
              .insert((*arg, location));
          }
        }
      }
      _ => {}
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    _state: &Self::FlowState,
    _terminator: &'mir Terminator<'tcx>,
    _location: Location,
  ) {
  }
}
