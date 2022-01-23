use std::{cell::RefCell, rc::Rc};

use log::debug;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyCtxt,
};
use rustc_mir_dataflow::{Analysis, AnalysisDomain, Forward};

use super::{
  mutation::{ModularMutationVisitor, MutationStatus},
  FlowResults,
};
use crate::{
  extensions::{is_extension_active, ContextMode, MutabilityMode},
  indexed::{
    impls::{LocationDomain, LocationSet, PlaceDomain},
    IndexMatrix, IndexSetIteratorExt,
  },
  mir::{
    aliases::Aliases,
    control_dependencies::ControlDependencies,
    utils::{OperandExt, PlaceExt},
  },
};

pub type FlowDomain<'tcx> = IndexMatrix<Place<'tcx>, Location>;

pub struct FlowAnalysis<'a, 'tcx> {
  pub tcx: TyCtxt<'tcx>,
  pub def_id: DefId,
  pub body: &'a Body<'tcx>,
  pub control_dependencies: ControlDependencies,
  pub aliases: Aliases<'tcx>,
  pub location_domain: Rc<LocationDomain>,
  crate recurse_cache: RefCell<HashMap<BodyId, FlowResults<'a, 'tcx>>>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'a Body<'tcx>,
    aliases: Aliases<'tcx>,
    control_dependencies: ControlDependencies,
    location_domain: Rc<LocationDomain>,
  ) -> Self {
    let recurse_cache = RefCell::new(HashMap::default());
    FlowAnalysis {
      tcx,
      def_id,
      body,
      aliases,
      location_domain,
      control_dependencies,
      recurse_cache,
    }
  }

  pub fn place_domain(&self) -> &Rc<PlaceDomain<'tcx>> {
    &self.aliases.place_domain
  }

  pub fn location_domain(&self) -> &Rc<LocationDomain> {
    &self.location_domain
  }

  crate fn transfer_function(
    &self,
    state: &mut FlowDomain<'tcx>,
    mutated: Place<'tcx>,
    inputs: &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
    location: Location,
    mutation_status: MutationStatus,
  ) {
    debug!("  Applying mutation to {mutated:?} with inputs {inputs:?}");
    let place_domain = self.place_domain();
    let location_domain = self.location_domain();

    let all_aliases = &self.aliases;
    let mutated_aliases = all_aliases
      .aliases
      .row_set(mutated)
      .unwrap_or_else(|| panic!("No aliases for mutated {mutated:?}"));

    // Clear sub-places of mutated place (if sound to do so)
    if matches!(mutation_status, MutationStatus::Definitely) && mutated_aliases.len() == 1
    {
      let mutated_direct = mutated_aliases.iter().next().unwrap();
      for sub in all_aliases.children.row(*mutated_direct) {
        state.clear_row(sub);
      }
    }

    let mut input_location_deps = LocationSet::new(location_domain);
    input_location_deps.insert(location);

    let add_deps = |place: Place<'tcx>, location_deps: &mut LocationSet| {
      for place in place.place_and_refs_in_projection(self.tcx) {
        for alias in all_aliases.aliases.row(place) {
          if let Some(deps) = state.row_set(alias) {
            location_deps.union(&deps);
          }
        }
      }
    };

    // Add deps of mutated to include provenance of mutated pointers
    add_deps(mutated, &mut input_location_deps);

    // Add deps of all inputs
    let mut children = Vec::new();
    for (place, elem) in inputs.iter() {
      add_deps(*place, &mut input_location_deps);

      // If the input is associated to a specific projection of the mutated
      // place, then save that input's dependencies with the projection
      if let Some(elem) = elem {
        let mut projection = mutated.projection.to_vec();
        projection.push(*elem);
        let mut child_deps = LocationSet::new(location_domain);
        add_deps(*place, &mut child_deps);
        children.push((
          Place::make(mutated.local, &projection, self.tcx),
          child_deps,
        ));
      }
    }

    // Add control dependencies
    let controlled_by = self.control_dependencies.dependent_on(location.block);
    let body = self.body;
    for block in controlled_by.into_iter().flat_map(|set| set.iter()) {
      input_location_deps.insert(body.terminator_loc(block));

      // Include dependencies of the switch's operand
      let terminator = body.basic_blocks()[block].terminator();
      if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind {
        if let Some(discr_place) = discr.to_place() {
          add_deps(discr_place, &mut input_location_deps);
        }
      }
    }

    if children.len() > 0 {
      // In the special case of mutated = aggregate { x: .., y: .. }
      // then we ensure that deps(mutated.x) != deps(mutated)
      for (child, deps) in children {
        state.union_into_row(child, &deps);
      }
      state.union_into_row(mutated, &input_location_deps);
    } else {
      // Union dependencies into all conflicting places of the mutated place
      let mut mutable_conflicts = all_aliases.conflicts(mutated).to_owned();

      // Remove any conflicts that aren't actually mutable, e.g. if x : &T ends up
      // as an alias of y: &mut T. See test function_lifetime_alias_mut for an example.
      let ignore_mut =
        is_extension_active(|mode| mode.mutability_mode == MutabilityMode::IgnoreMut);
      if !ignore_mut {
        let body = self.body;
        let tcx = self.tcx;
        mutable_conflicts = mutable_conflicts
          .iter()
          .filter(|place| {
            place.iter_projections().all(|(sub_place, _)| {
              let ty = sub_place.ty(body.local_decls(), tcx).ty;
              !matches!(ty.ref_mutability(), Some(Mutability::Not))
            })
          })
          .collect_indices(place_domain);
      };

      debug!("  Mutated conflicting places: {mutable_conflicts:?}");
      debug!("    with deps {input_location_deps:?}");

      for place in mutable_conflicts.iter() {
        state.union_into_row(place, &input_location_deps);
      }
    }
  }
}

impl AnalysisDomain<'tcx> for FlowAnalysis<'a, 'tcx> {
  type Domain = FlowDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "FlowAnalysis";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    FlowDomain::new(self.place_domain(), self.location_domain())
  }

  fn initialize_start_block(&self, body: &Body<'tcx>, state: &mut Self::Domain) {
    for arg in self.place_domain().all_args(body) {
      state.insert(arg, self.location_domain().arg_to_location(arg));
    }
  }
}

impl Analysis<'tcx> for FlowAnalysis<'a, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    ModularMutationVisitor::new(
      self.tcx,
      self.body,
      self.def_id,
      |mutated: Place<'tcx>,
       inputs: &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
       location: Location,
       mutation_status: MutationStatus| {
        self.transfer_function(state, mutated, inputs, location, mutation_status)
      },
    )
    .visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &Terminator<'tcx>,
    location: Location,
  ) {
    if matches!(terminator.kind, TerminatorKind::Call { .. })
      && is_extension_active(|mode| mode.context_mode == ContextMode::Recurse)
      && self.recurse_into_call(state, &terminator.kind, location)
    {
      return;
    }

    ModularMutationVisitor::new(
      self.tcx,
      self.body,
      self.def_id,
      |mutated: Place<'tcx>,
       inputs: &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
       location: Location,
       mutation_status: MutationStatus| {
        self.transfer_function(state, mutated, inputs, location, mutation_status)
      },
    )
    .visit_terminator(terminator, location);
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _return_places: rustc_mir_dataflow::CallReturnPlaces<'_, 'tcx>,
  ) {
  }
}
