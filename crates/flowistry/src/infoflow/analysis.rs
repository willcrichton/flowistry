use std::{cell::RefCell, rc::Rc};

use log::{debug, trace};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyCtxt,
};
use rustc_mir_dataflow::{Analysis, AnalysisDomain, Forward};
use rustc_utils::{
  mir::control_dependencies::ControlDependencies, BodyExt, OperandExt, PlaceExt,
};

use super::{
  mutation::{ModularMutationVisitor, Mutation, MutationStatus},
  FlowResults,
};
use crate::{
  extensions::{is_extension_active, ContextMode, MutabilityMode},
  indexed::{
    impls::{LocationOrArg, LocationOrArgDomain, LocationOrArgSet},
    IndexMatrix, IndexedDomain,
  },
  mir::aliases::Aliases,
};

/// Represents the information flows at a given instruction. See [`FlowResults`] for a high-level explanation of this datatype.
///
/// `FlowDomain` represents $\Theta$ that maps from places $p$ to dependencies $\kappa$. To efficiently represent $\kappa$, a set of locations,
/// we use the bit-set data structures in [`rustc_index::bit_set`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_index/bit_set/index.html).
/// However instead of using those datatypes directly, we provide several abstractions in the [`indexed`](crate::indexed)
/// module that have a more ergonomic interface and more efficient implementation than their `bit_set` counterparts.
///
/// The [`IndexMatrix`] maps from a [`Place`] to a [`LocationOrArgSet`] via the [`IndexMatrix::row_set`] method. The [`LocationOrArgSet`] is an
/// [`IndexSet`](crate::indexed::IndexSet) of locations (or arguments, see note below), which wraps a
/// [`rustc_index::bit_set::HybridBitSet`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_index/bit_set/enum.HybridBitSet.html) and
/// has roughly the same API. The main difference is that `HybridBitSet` operates only on values that implement the
/// [`rustc_index::vec::Idx`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_index/vec/trait.Idx.html) trait (usually created via
/// the [`rustc_index::newtype_index`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_index/macro.newtype_index.html) macro). The `indexed`
/// module has a concept of [`IndexedDomain`] to represent the mapping from a set of values to the indexes those values --- [`LocationOrArgDomain`]
/// is the implementation for locations.
///
///
/// # Note: arguments as dependencies
/// Because function arguments are never initialized, there is no "root" location for argument places. This fact poses a problem for
/// information flow analysis: an instruction `bb[0]: _2 = _1` (where `_1` is an argument) would set $\Theta(\verb|_2|) = \Theta(\verb|_1|) \cup \\{\verb|bb0\[0\]|\\}\$.
/// However, $\Theta(\verb|_1|)$ would be empty, so it would be imposible to determine that `_2` depends on `_1`. To solve this issue, we
/// enrich the domain of locations with arguments, using the [`LocationOrArg`] type. Any dependency can be on *either* a location or an argument.
pub type FlowDomain<'tcx> = IndexMatrix<Place<'tcx>, LocationOrArg>;

/// Data structure that holds context for performing the information flow analysis.
pub struct FlowAnalysis<'a, 'tcx> {
  pub tcx: TyCtxt<'tcx>,
  pub def_id: DefId,
  pub body: &'a Body<'tcx>,
  pub control_dependencies: ControlDependencies<BasicBlock>,
  pub aliases: Aliases<'a, 'tcx>,
  pub(crate) recurse_cache: RefCell<HashMap<BodyId, FlowResults<'a, 'tcx>>>,
}

impl<'a, 'tcx> FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'a Body<'tcx>,
    aliases: Aliases<'a, 'tcx>,
  ) -> Self {
    let recurse_cache = RefCell::new(HashMap::default());
    let control_dependencies = body.control_dependencies();
    debug!("Control dependencies: {control_dependencies:?}");
    FlowAnalysis {
      tcx,
      def_id,
      body,
      aliases,
      control_dependencies,
      recurse_cache,
    }
  }

  pub fn location_domain(&self) -> &Rc<LocationOrArgDomain> {
    self.aliases.location_domain()
  }

  pub(crate) fn transfer_function(
    &self,
    state: &mut FlowDomain<'tcx>,
    Mutation {
      mutated,
      inputs,
      location,
      status,
    }: Mutation<'_, 'tcx>,
  ) {
    debug!("  Applying mutation to {mutated:?} with inputs {inputs:?}");
    let location_domain = self.location_domain();

    let all_aliases = &self.aliases;
    let mutated_aliases = all_aliases.aliases(mutated);
    trace!("    Mutated aliases: {mutated_aliases:?}");
    assert!(!mutated_aliases.is_empty());

    // Clear sub-places of mutated place (if sound to do so)
    if matches!(status, MutationStatus::Definitely) && mutated_aliases.len() == 1 {
      let mutated_direct = mutated_aliases.iter().next().unwrap();
      for sub in all_aliases.children(*mutated_direct).iter() {
        state.clear_row(all_aliases.normalize(*sub));
      }
    }

    let mut input_location_deps = LocationOrArgSet::new(location_domain);
    input_location_deps.insert(location);

    let add_deps = |place: Place<'tcx>, location_deps: &mut LocationOrArgSet| {
      let reachable_values = all_aliases.reachable_values(place, Mutability::Not);
      let provenance = place.refs_in_projection().flat_map(|(place_ref, _)| {
        all_aliases
          .aliases(Place::from_ref(place_ref, self.tcx))
          .iter()
      });
      for relevant in reachable_values.iter().chain(provenance) {
        let deps = state.row_set(all_aliases.normalize(*relevant));
        trace!("    For relevant {relevant:?} for input {place:?} adding deps {deps:?}");
        location_deps.union(&deps);
      }
    };

    // Add deps of mutated to include provenance of mutated pointers
    add_deps(mutated, &mut input_location_deps);

    // Add deps of all inputs
    for place in inputs.iter() {
      add_deps(*place, &mut input_location_deps);
    }

    // Add control dependencies
    let controlled_by = self.control_dependencies.dependent_on(location.block);
    let body = self.body;
    for block in controlled_by.into_iter().flat_map(|set| set.iter()) {
      input_location_deps.insert(body.terminator_loc(block));

      // Include dependencies of the switch's operand
      let terminator = body.basic_blocks[block].terminator();
      if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind {
        if let Some(discr_place) = discr.as_place() {
          add_deps(discr_place, &mut input_location_deps);
        }
      }
    }

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
        .copied()
        .collect::<HashSet<_>>();
    };

    debug!("  Mutated conflicting places: {mutable_conflicts:?}");
    debug!("    with deps {input_location_deps:?}");

    for place in mutable_conflicts.into_iter() {
      state.union_into_row(all_aliases.normalize(place), &input_location_deps);
    }
  }
}

impl<'a, 'tcx> AnalysisDomain<'tcx> for FlowAnalysis<'a, 'tcx> {
  type Domain = FlowDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "FlowAnalysis";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    FlowDomain::new(self.location_domain())
  }

  fn initialize_start_block(&self, _body: &Body<'tcx>, state: &mut Self::Domain) {
    for (arg, loc) in self.aliases.all_args() {
      for place in self.aliases.conflicts(arg) {
        debug!(
          "arg={arg:?} / place={place:?} / loc={:?}",
          self.location_domain().value(loc)
        );
        state.insert(self.aliases.normalize(*place), loc);
      }
    }
  }
}

impl<'a, 'tcx> Analysis<'tcx> for FlowAnalysis<'a, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    ModularMutationVisitor::new(&self.aliases, |mutation| {
      self.transfer_function(state, mutation)
    })
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

    ModularMutationVisitor::new(&self.aliases, |mutation| {
      self.transfer_function(state, mutation)
    })
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
