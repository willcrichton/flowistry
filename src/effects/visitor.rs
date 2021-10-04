use crate::{
  core::{
    indexed::{IndexSetIteratorExt, IndexedDomain},
    indexed_impls::{PlaceIndex, PlaceSet},
    utils,
  },
  flow,
};
use log::debug;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_middle::mir::*;
use rustc_mir_dataflow::ResultsVisitor;

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
        let ptrs = utils::interior_pointers(place, tcx, body, analysis.def_id);
        debug!("interior_pointers: {:?}", ptrs);

        ptrs
          .into_values()
          .map(|places| places.into_iter())
          .flatten()
          .filter(|(_, mutability)| *mutability == Mutability::Mut)
          .map(|(place, _)| {
            let deref_place = tcx.mk_place_deref(place);
            utils::interior_places(deref_place, tcx, body, analysis.def_id).into_iter()
          })
          .flatten()
      })
      .flatten()
      .collect_indices(analysis.place_domain().clone());
    debug!("mut_args: {:#?}", mut_args);

    FindEffects {
      analysis,
      mut_args,
      effects: HashMap::default(),
    }
  }

  pub fn add_effect(&mut self, mutated: Place<'tcx>, location: Location) {
    if mutated.local == RETURN_PLACE {
      self
        .effects
        .entry(EffectKind::Return)
        .or_default()
        .insert((mutated, location));
    } else {
      let mut aliases = self.analysis.aliases.aliases(mutated);
      debug!(
        "Checking for effect on {:?} (aliases {:?})",
        mutated, aliases
      );

      aliases.intersect(&self.mut_args);

      for arg in aliases.iter() {
        let kind = EffectKind::MutArg(self.analysis.place_domain().index(arg));
        debug!("Mutation on {:?} adding arg effect on {:?}", mutated, arg);
        self
          .effects
          .entry(kind)
          .or_default()
          .insert((*arg, location));
      }
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
      StatementKind::Assign(box (mutated, _)) => self.add_effect(*mutated, location),
      _ => {}
    }
  }

  fn visit_terminator_after_primary_effect(
    &mut self,
    _state: &Self::FlowState,
    terminator: &'mir Terminator<'tcx>,
    location: Location,
  ) {
    match &terminator.kind {
      TerminatorKind::Call {
        args, destination, ..
      } => {
        if let Some((destination, _)) = destination {
          self.add_effect(*destination, location);
        }

        for (_, mut_ptr) in utils::arg_mut_ptrs(
          &utils::arg_places(args),
          self.analysis.tcx,
          self.analysis.body,
          self.analysis.def_id,
        ) {
          self.add_effect(mut_ptr, location);
        }
      }

      _ => {}
    }
  }
}
