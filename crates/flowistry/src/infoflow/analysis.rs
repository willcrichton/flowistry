use std::{cell::RefCell, rc::Rc};

use log::{debug, trace};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::{def_id::DefId, BodyId};
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyCtxt,
};
use rustc_mir_dataflow::{Analysis, AnalysisDomain, Forward};
use rustc_utils::{
  mir::control_dependencies::ControlDependencies, BodyExt, OperandExt, PlaceExt,
};
use smallvec::SmallVec;

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
  mir::placeinfo::PlaceInfo,
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
  pub place_info: PlaceInfo<'a, 'tcx>,
  pub(crate) recurse_cache: RefCell<HashMap<BodyId, FlowResults<'a, 'tcx>>>,
}

impl<'a, 'tcx> FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'a Body<'tcx>,
    place_info: PlaceInfo<'a, 'tcx>,
  ) -> Self {
    let recurse_cache = RefCell::new(HashMap::default());
    let control_dependencies = body.control_dependencies();
    debug!("Control dependencies: {control_dependencies:?}");
    FlowAnalysis {
      tcx,
      def_id,
      body,
      place_info,
      control_dependencies,
      recurse_cache,
    }
  }

  pub fn location_domain(&self) -> &Rc<LocationOrArgDomain> {
    self.place_info.location_domain()
  }

  fn influences(&self, place: Place<'tcx>) -> SmallVec<[Place<'tcx>; 8]> {
    let conflicts = self
      .place_info
      .aliases(place)
      .iter()
      .flat_map(|alias| self.place_info.conflicts(*alias));
    let provenance = place.refs_in_projection().flat_map(|(place_ref, _)| {
      self
        .place_info
        .aliases(Place::from_ref(place_ref, self.tcx))
        .iter()
    });
    conflicts.chain(provenance).copied().collect()
  }

  pub fn deps_for(
    &self,
    state: &FlowDomain<'tcx>,
    place: Place<'tcx>,
  ) -> LocationOrArgSet {
    let mut deps = LocationOrArgSet::new(self.location_domain());
    for subplace in self
      .place_info
      .reachable_values(place, Mutability::Not)
      .iter()
      .flat_map(|place| self.influences(*place))
    {
      deps.union(&state.row_set(self.place_info.normalize(subplace)));
    }
    deps
  }

  // This function expects *ALL* the mutations that occur within a given [`Location`] at once.
  pub(crate) fn transfer_function(
    &self,
    state: &mut FlowDomain<'tcx>,
    mutations: Vec<Mutation<'tcx>>,
    location: Location,
  ) {
    debug!("  Applying mutations {mutations:?}");
    let location_domain = self.location_domain();

    // Initialize dependencies to include current location of mutation.
    let mut all_deps = {
      let mut deps = LocationOrArgSet::new(location_domain);
      deps.insert(location);
      vec![deps; mutations.len()]
    };

    // Add every influence on `input` to `deps`.
    let add_deps = |state: &FlowDomain<'tcx>,
                    input,
                    target_deps: &mut LocationOrArgSet| {
      for relevant in self.influences(input) {
        let relevant_deps = state.row_set(self.place_info.normalize(relevant));
        trace!("    For relevant {relevant:?} for input {input:?} adding deps {relevant_deps:?}");
        target_deps.union(&relevant_deps);
      }
    };

    // Register every explicitly provided input as an input.
    for (mt, deps) in mutations.iter().zip(&mut all_deps) {
      for input in &mt.inputs {
        add_deps(state, *input, deps);
      }
    }

    // Add location of every control dependency.
    let controlled_by = self.control_dependencies.dependent_on(location.block);
    let body = self.body;
    for block in controlled_by.into_iter().flat_map(|set| set.iter()) {
      for deps in &mut all_deps {
        deps.insert(body.terminator_loc(block));
      }

      // Include dependencies of the switch's operand.
      let terminator = body.basic_blocks[block].terminator();
      if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind {
        if let Some(discr_place) = discr.as_place() {
          for deps in &mut all_deps {
            add_deps(state, discr_place, deps);
          }
        }
      }
    }

    let ignore_mut =
      is_extension_active(|mode| mode.mutability_mode == MutabilityMode::IgnoreMut);
    for (mt, deps) in mutations.iter().zip(&mut all_deps) {
      // Clear sub-places of mutated place (if sound to do so)
      if matches!(mt.status, MutationStatus::Definitely)
        && self.place_info.aliases(mt.mutated).len() == 1
      {
        for sub in self.place_info.children(mt.mutated).iter() {
          state.clear_row(self.place_info.normalize(*sub));
        }
      }

      // Add deps of mutated to include provenance of mutated pointers
      add_deps(state, mt.mutated, deps);

      let mutable_aliases = self
        .place_info
        .aliases(mt.mutated)
        .iter()
        .filter(|alias| {
          // Remove any conflicts that aren't actually mutable, e.g. if x : &T ends up
          // as an alias of y: &mut T. See test function_lifetime_alias_mut for an example.
          let has_immut = alias.iter_projections().any(|(sub_place, _)| {
            let ty = sub_place.ty(body.local_decls(), self.tcx).ty;
            matches!(ty.ref_mutability(), Some(Mutability::Not))
          });
          !has_immut || ignore_mut
        })
        .collect::<SmallVec<[_; 8]>>();

      debug!("  Mutated places: {mutable_aliases:?}");
      debug!("    with deps {deps:?}");

      for alias in mutable_aliases {
        state.union_into_row(self.place_info.normalize(*alias), deps);
      }
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
    for (arg, loc) in self.place_info.all_args() {
      for place in self.place_info.conflicts(arg) {
        debug!(
          "arg={arg:?} / place={place:?} / loc={:?}",
          self.location_domain().value(loc)
        );
        state.insert(self.place_info.normalize(*place), loc);
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
    ModularMutationVisitor::new(&self.place_info, |_, mutations| {
      self.transfer_function(state, mutations, location)
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

    ModularMutationVisitor::new(&self.place_info, |_, mutations| {
      self.transfer_function(state, mutations, location)
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
