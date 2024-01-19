use std::{borrow::Cow, iter, rc::Rc};

use df::JoinSemiLattice;
use either::Either;
use flowistry_pdg::{CallString, GlobalLocation, RichLocation};
use itertools::Itertools;
use log::{debug, trace};
use petgraph::graph::DiGraph;
use rustc_abi::FieldIdx;
use rustc_ast::Mutability;
use rustc_borrowck::consumers::{
  places_conflict, BodyWithBorrowckFacts, PlaceConflictBias,
};
use rustc_data_structures::graph::{self as rustc_graph, WithStartNode};
use rustc_hash::{FxHashMap, FxHashSet};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_index::IndexVec;
use rustc_middle::{
  mir::{
    visit::Visitor, AggregateKind, BasicBlock, Body, Location, Operand, Place, PlaceElem,
    Rvalue, Statement, StatementKind, Terminator, TerminatorKind, RETURN_PLACE
  },
  ty::{GenericArg, GenericArgsRef, List, ParamEnv, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{self as df};
use rustc_utils::{
  mir::{borrowck_facts, control_dependencies::ControlDependencies},
  BodyExt, PlaceExt,
};

use super::graph::{DepEdge, DepGraph, DepNode};
use crate::{
  infoflow::mutation::{ModularMutationVisitor, MutationStatus},
  mir::placeinfo::PlaceInfo,
  pdg::utils::{self, FnResolution},
};

type CallFilter<'tcx> = Box<dyn Fn(FnResolution<'tcx>, CallString) -> bool + 'tcx>;

/// Top-level parameters to PDG construction.
#[derive(Clone)]
pub struct PdgParams<'tcx> {
  tcx: TyCtxt<'tcx>,
  root: FnResolution<'tcx>,
  call_filter: Option<Rc<CallFilter<'tcx>>>,
  false_call_edges: bool,
}

impl<'tcx> PdgParams<'tcx> {
  /// Must provide the [`TyCtxt`] and the [`LocalDefId`] of the function that is the root of the PDG.
  pub fn new(tcx: TyCtxt<'tcx>, root: LocalDefId) -> Self {
    PdgParams {
      tcx,
      root: FnResolution::Partial(root.to_def_id()),
      call_filter: None,
      false_call_edges: false,
    }
  }

  /// Provide an optional call filter.
  ///
  /// A call filter is a user-provided callback which determines whether the PDG generator will inspect
  /// a call site. For example, in the code:
  ///
  /// ```
  /// fn incr(x: i32) -> i32 { x + 1 }
  /// fn main() {
  ///   let a = 0;
  ///   let b = incr(a);
  /// }
  /// ```
  ///
  /// When inspecting the call `incr(a)`, the call filter will be called with `f(incr, [main])`.
  /// The first argument is the function being called, and the second argument is the call string.
  ///
  /// For example, you could apply a hard limit on call string length like this:
  ///
  /// ```
  /// # #![feature(rustc_private)]
  /// # extern crate rustc_middle;
  /// # use flowistry::pdg::PdgParams;
  /// # use rustc_middle::ty::TyCtxt;
  /// # const THRESHOLD: usize = 5;
  /// # fn f<'tcx>(tcx: TyCtxt<'tcx>, params: PdgParams<'tcx>) -> PdgParams<'tcx> {
  /// params.with_call_filter(|_, cs| cs.len() <= THRESHOLD)
  /// # }
  /// ```
  ///
  /// Or you could prevent inspection of specific functions based on their [`DefId`]:
  ///
  /// ```
  /// # #![feature(rustc_private)]
  /// # extern crate rustc_middle;
  /// # use flowistry::pdg::PdgParams;
  /// # use rustc_middle::ty::TyCtxt;
  /// # fn f<'tcx>(tcx: TyCtxt<'tcx>, params: PdgParams<'tcx>) -> PdgParams<'tcx> {
  /// params.with_call_filter(move |f, _| {
  ///   let name = tcx.opt_item_name(f.def_id());
  ///   !matches!(name.as_ref().map(|sym| sym.as_str()), Some("no_inline"))
  /// })
  /// # }
  /// ```
  pub fn with_call_filter(
    self,
    f: impl Fn(FnResolution<'tcx>, CallString) -> bool + 'tcx,
  ) -> Self {
    PdgParams {
      call_filter: Some(Rc::new(Box::new(f))),
      ..self
    }
  }

  /// Enable PDG generation to insert false edges to mutable references passed to function calls.
  ///
  /// This is a special case used by Paralegal.
  pub fn with_false_call_edges(self) -> Self {
    PdgParams {
      false_call_edges: true,
      ..self
    }
  }
}

#[derive(PartialEq, Eq, Default, Clone)]
pub struct PartialGraph<'tcx> {
  edges: FxHashSet<(DepNode<'tcx>, DepNode<'tcx>, DepEdge)>,
  last_mutation: FxHashMap<Place<'tcx>, FxHashSet<RichLocation>>,
}

impl<'tcx> df::JoinSemiLattice for PartialGraph<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    let b1 = utils::hashset_join(&mut self.edges, &other.edges);
    let b2 = utils::hashmap_join(
      &mut self.last_mutation,
      &other.last_mutation,
      utils::hashset_join,
    );
    b1 || b2
  }
}

struct CallingContext<'tcx> {
  call_string: CallString,
  param_env: ParamEnv<'tcx>,
  call_stack: Vec<DefId>,
}

pub struct GraphConstructor<'tcx> {
  tcx: TyCtxt<'tcx>,
  params: PdgParams<'tcx>,
  body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
  body: Cow<'tcx, Body<'tcx>>,
  def_id: LocalDefId,
  place_info: PlaceInfo<'tcx>,
  control_dependencies: ControlDependencies<BasicBlock>,
  body_assignments: utils::BodyAssignments,
  calling_context: Option<CallingContext<'tcx>>,
  start_loc: FxHashSet<RichLocation>,
}

macro_rules! trylet {
  ($p:pat = $e:expr, $($arg:tt)*) => {
    let $p = $e else {
      trace!($($arg)*);
      return None;
    };
  }
}

impl<'tcx> GraphConstructor<'tcx> {
  /// Creates a [`GraphConstructor`] at the root of the PDG.
  pub fn root(params: PdgParams<'tcx>) -> Self {
    GraphConstructor::new(params, None)
  }

  /// Creates [`GraphConstructor`] for a function resolved as `fn_resolution` in a given `calling_context`.
  fn new(params: PdgParams<'tcx>, calling_context: Option<CallingContext<'tcx>>) -> Self {
    let tcx = params.tcx;
    let def_id = params.root.def_id().expect_local();
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    let param_env = match &calling_context {
      Some(cx) => cx.param_env,
      None => ParamEnv::reveal_all(),
    };
    let body =
      utils::try_monomorphize(tcx, params.root, param_env, &body_with_facts.body);
    debug!("{}", body.to_string(tcx).unwrap());

    let place_info = PlaceInfo::build(tcx, def_id.to_def_id(), body_with_facts);
    let control_dependencies = body.control_dependencies();

    let mut start_loc = FxHashSet::default();
    start_loc.insert(RichLocation::Start);

    let body_assignments = utils::find_body_assignments(&body);

    GraphConstructor {
      tcx,
      params,
      body_with_facts,
      body,
      place_info,
      control_dependencies,
      start_loc,
      def_id,
      calling_context,
      body_assignments,
    }
  }

  /// Creates a [`GlobalLocation`] at the current function.
  fn make_global_loc(&self, location: impl Into<RichLocation>) -> GlobalLocation {
    GlobalLocation {
      function: self.def_id,
      location: location.into(),
    }
  }

  /// Creates a [`CallString`] with the current function at the root,
  /// with the rest of the string provided by the [`CallingContext`].
  fn make_call_string(&self, location: impl Into<RichLocation>) -> CallString {
    let global_loc = self.make_global_loc(location);
    match &self.calling_context {
      Some(cx) => cx.call_string.push(global_loc),
      None => CallString::single(global_loc),
    }
  }

  fn make_dep_node(
    &self,
    place: Place<'tcx>,
    location: impl Into<RichLocation>,
  ) -> DepNode<'tcx> {
    DepNode::new(place, self.make_call_string(location), self.tcx, &self.body)
  }

  /// Returns all pairs of `(src, edge)`` such that the given `location` is control-dependent on `edge`
  /// with input `src`.
  fn find_control_inputs(&self, location: Location) -> Vec<(DepNode<'tcx>, DepEdge)> {
    match self.control_dependencies.dependent_on(location.block) {
      Some(ctrl_deps) => ctrl_deps
        .iter()
        .filter_map(|block| {
          let ctrl_loc = self.body.terminator_loc(block);
          let Terminator {
            kind: TerminatorKind::SwitchInt { discr, .. },
            ..
          } = self.body.stmt_at(ctrl_loc).unwrap_right()
          else {
            return None;
          };
          let ctrl_place = discr.place()?;
          let at = self.make_call_string(ctrl_loc);
          let src = DepNode::new(ctrl_place, at, self.tcx, &self.body);
          let edge = DepEdge::control(at);
          Some((src, edge))
        })
        .collect_vec(),
      None => Vec::new(),
    }
  }

  /// Returns the aliases of `place`. See [`PlaceInfo::aliases`] for details.
  fn aliases(&self, place: Place<'tcx>) -> impl Iterator<Item = Place<'tcx>> + '_ {
    // MASSIVE HACK ALERT:
    // The issue is that monomorphization erases regions, due to how it's implemented in rustc.
    // However, Flowistry's alias analysis uses regions to figure out aliases.
    // To workaround this incompatibility, when we receive a monomorphized place, we try to
    // recompute its type in the context of the original region-containing body as far as possible.
    //
    // For example, say _2: (&'0 impl Foo,) in the original body and _2: (&(i32, i32),) in the monomorphized body.
    // Say we ask for aliases (*(_2.0)).0. Then we will retype ((*_2.0).0).0 and receive back (*_2.0: &'0 impl Foo).
    // We can ask for the aliases in the context of the original body, receiving e.g. {_1}.
    // Then we reproject the aliases with the remaining projection, to create {_1.0}.
    //
    // This is a massive hack bc it's inefficient and I'm not certain that it's sound.
    let place_retyped = utils::retype_place(
      place,
      self.tcx,
      &self.body_with_facts.body,
      self.def_id.to_def_id(),
    );
    self.place_info.aliases(place_retyped).iter().map(|alias| {
      let mut projection = alias.projection.to_vec();
      projection.extend(&place.projection[place_retyped.projection.len() ..]);
      Place::make(alias.local, &projection, self.tcx)
    })
  }

  /// Returns all nodes `src` such that `src` is:
  /// 1. Part of the value of `input`
  /// 2. The most-recently modified location for `src`
  fn find_data_inputs(
    &self,
    state: &PartialGraph<'tcx>,
    input: Place<'tcx>,
  ) -> Vec<DepNode<'tcx>> {
    // Include all sources of indirection (each reference in the chain) as relevant places.
    let provenance = input
      .refs_in_projection()
      .map(|(place_ref, _)| Place::from_ref(place_ref, self.tcx));
    let inputs = iter::once(input).chain(provenance);

    inputs
      // **POINTER-SENSITIVITY:**
      // If `input` involves indirection via dereferences, then resolve it to the direct places it could point to.
      .flat_map(|place| self.aliases(place))
      .flat_map(|alias| {
        // **FIELD-SENSITIVITY:**
        // Find all places that have been mutated which conflict with `alias.`
        let conflicts = state
          .last_mutation
          .keys()
          .filter(move |place| {
            if place.is_indirect() && place.is_arg(&self.body) {
              // HACK: `places_conflict` seems to consider it a bug is `borrow_place`
              // includes a dereference, which should only happen if `borrow_place`
              // is an argument. So we special case that condition and just compare for local equality.
              //
              // TODO: this is not field-sensitive!
              place.local == alias.local
            } else {
              places_conflict(
                self.tcx,
                &self.body,
                **place,
                alias,
                PlaceConflictBias::Overlap,
              )
            }
          })
          .map(|place| (*place, &state.last_mutation[place]));

        // Special case: if the `alias` is an un-mutated argument, then include it as a conflict
        // coming from the special start location.
        let alias_last_mut = if alias.is_arg(&self.body) {
          Some((alias, &self.start_loc))
        } else {
          None
        };

        // For each `conflict`` last mutated at the locations `last_mut`:
        conflicts
          .chain(alias_last_mut)
          .flat_map(|(conflict, last_mut_locs)| {
            // For each last mutated location:
            last_mut_locs.iter().map(move |last_mut_loc| {
              // Return <CONFLICT> @ <LAST_MUT_LOC> as an input node.
              let at = self.make_call_string(*last_mut_loc);
              DepNode::new(conflict, at, self.tcx, &self.body)
            })
          })
      })
      .collect()
  }

  /// Returns all nodes `dst` such that `dst` is an alias of `mutated`.
  ///
  /// Also updates the last-mutated location for `dst` to the given `location`.
  fn find_and_update_outputs(
    &self,
    state: &mut PartialGraph<'tcx>,
    mutated: Place<'tcx>,
    status: MutationStatus,
    location: Location,
  ) -> Vec<DepNode<'tcx>> {
    // **POINTER-SENSITIVITY:**
    // If `mutated` involves indirection via dereferences, then resolve it to the direct places it could point to.
    let aliases = self.aliases(mutated).collect_vec();

    // **FIELD-SENSITIVITY:** we do NOT deal with fields on *writes* (in this function),
    // only on *reads* (in `add_input_to_op`).

    // For each mutated `dst`:
    aliases
      .iter()
      .map(|dst| {
        // Create a destination node for (DST @ CURRENT_LOC).
        let dst_node =
          DepNode::new(*dst, self.make_call_string(location), self.tcx, &self.body);

        // **STRONG UPDATES:**
        // If the mutated place has no aliases AND the mutation definitely occurs,
        // then clear all previous locations of mutation.
        let dst_mutations = state.last_mutation.entry(*dst).or_default();
        if aliases.len() == 1 && matches!(status, MutationStatus::Definitely) {
          dst_mutations.clear();
        }

        // Register that `dst` is mutated at the current location.
        dst_mutations.insert(RichLocation::Location(location));

        dst_node
      })
      .collect()
  }

  /// Update the PDG with arrows from `inputs` to `mutated` at `location`.
  fn apply_mutation(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    mutated: Either<Place<'tcx>, DepNode<'tcx>>,
    inputs: Either<Vec<Place<'tcx>>, DepNode<'tcx>>,
    status: MutationStatus,
  ) {
    trace!("Applying mutation to {mutated:?} with inputs {inputs:?}");

    let ctrl_inputs = self.find_control_inputs(location);

    let data_inputs = match inputs {
      Either::Left(places) => places
        .into_iter()
        .flat_map(|input| self.find_data_inputs(state, input))
        .collect::<Vec<_>>(),
      Either::Right(node) => vec![node],
    };
    trace!("  Data inputs: {data_inputs:?}");

    let outputs = match mutated {
      Either::Left(place) => self.find_and_update_outputs(state, place, status, location),
      Either::Right(node) => vec![node],
    };
    trace!("  Outputs: {outputs:?}");

    // Add data dependencies: data_input -> output
    let data_edge = DepEdge::data(self.make_call_string(location));
    for data_input in data_inputs {
      for output in &outputs {
        trace!("Adding edge {data_input:?} -> {output:?}");
        state.edges.insert((data_input, *output, data_edge));
      }
    }

    // Add control dependencies: ctrl_input -> output
    for (ctrl_input, edge) in &ctrl_inputs {
      for output in &outputs {
        state.edges.insert((*ctrl_input, *output, *edge));
      }
    }
  }

  /// Given the arguments to a `Future::poll` call, walk back through the
  /// body to find the original future being polled, and get the arguments to the future.
  fn find_async_args<'a>(
    &'a self,
    args: &'a [Operand<'tcx>],
  ) -> Option<&'a [Operand<'tcx>]> {
    let get_def_for_op = |op: &Operand<'tcx>| -> Option<Location> {
      trylet!(Some(place) = op.place(), "Arg is not a place");
      trylet!(Some(local) = place.as_local(), "Place is not a local");
      trylet!(
        Some(locs) = &self.body_assignments.get(&local),
        "Local has no assignments"
      );
      debug_assert!(locs.len() == 1);
      Some(locs[0])
    };

    trylet!(
      Either::Right(Terminator {
        kind: TerminatorKind::Call {
          args: new_pin_args,
          ..
        },
        ..
      }) = &self.body.stmt_at(get_def_for_op(&args[0])?),
      "Pinned assignment is not a call"
    );
    debug_assert!(new_pin_args.len() == 1);

    let future_aliases = self
      .aliases(self.tcx.mk_place_deref(new_pin_args[0].place().unwrap()))
      .collect_vec();
    debug_assert!(future_aliases.len() == 1);
    let future = *future_aliases.first().unwrap();

    trylet!(
      Either::Left(Statement {
        kind: StatementKind::Assign(box (_, Rvalue::Use(future2))),
        ..
      }) = &self.body.stmt_at(get_def_for_op(&Operand::Move(future))?),
      "Assignment to pin::new input is not a statement"
    );

    trylet!(
      Either::Right(Terminator {
        kind: TerminatorKind::Call {
          args: into_future_args,
          ..
        },
        ..
      }) = &self.body.stmt_at(get_def_for_op(future2)?),
      "Assignment to alias of pin::new input is not a call"
    );

    trylet!(
      Either::Right(Terminator {
        kind: TerminatorKind::Call { args, .. },
        ..
      }) = &self.body.stmt_at(get_def_for_op(&into_future_args[0])?),
      "Assignment to into_future input is not a call"
    );

    Some(args)
  }

  /// Resolve a function [`Operand`] to a specific [`DefId`] and generic arguments if possible.
  fn operand_to_def_id(
    &self,
    func: &Operand<'tcx>,
  ) -> Option<(DefId, &'tcx List<GenericArg<'tcx>>)> {
    match func {
      Operand::Constant(func) => match func.literal.ty().kind() {
        TyKind::FnDef(def_id, generic_args) => Some((*def_id, generic_args)),
        ty => {
          trace!("Bailing from handle_call because func is literal with type: {ty:?}");
          None
        }
      },
      Operand::Copy(place) | Operand::Move(place) => {
        // TODO: control-flow analysis to deduce fn for inlined closures
        trace!("Bailing from handle_call because func is place {place:?}");
        None
      }
    }
  }

  fn fmt_fn(&self, def_id: DefId) -> String {
    self.tcx.def_path_str(def_id)
  }

  /// Attempt to inline a call to a function, returning None if call is not inline-able.
  fn handle_call(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    func: &Operand<'tcx>,
    args: &[Operand<'tcx>],
    destination: Place<'tcx>,
  ) -> Option<()> {
    // Note: my comments here will use "child" to refer to the callee and
    // "parent" to refer to the caller, since the words are most visually distinct.

    let tcx = self.tcx;

    let (called_def_id, generic_args) = self.operand_to_def_id(func)?;
    trace!("Resolved call to function: {}", self.fmt_fn(called_def_id));

    // Handle async functions at the time of polling, not when the future is created.
    if tcx.asyncness(called_def_id).is_async() {
      trace!("  Bailing because func is async");
      return Some(());
    }

    // Monomorphize the called function with the known generic_args.
    let param_env = tcx.param_env(self.def_id);
    let resolved_fn =
      utils::try_resolve_function(self.tcx, called_def_id, param_env, generic_args);
    let resolved_def_id = resolved_fn.def_id();
    if called_def_id != resolved_def_id {
      let (called, resolved) = (self.fmt_fn(called_def_id), self.fmt_fn(resolved_def_id));
      trace!("  `{called}` monomorphized to `{resolved}`",);
    }

    // Don't inline recursive calls.
    if let Some(cx) = &self.calling_context {
      if cx.call_stack.contains(&resolved_def_id) {
        trace!("  Bailing due to recursive call");
        return None;
      }
    }

    enum CallKind {
      /// A standard function call like `f(x)`.
      Direct,
      /// A call to a function variable, like `fn foo(f: impl Fn()) { f() }`
      Indirect,
      /// A poll to an async function, like `f.await`.
      AsyncPoll,
    }
    // Determine the type of call-site.
    let (call_kind, args) = match tcx.def_path_str(called_def_id).as_str() {
      "std::ops::Fn::call" => (CallKind::Indirect, args),
      "std::future::Future::poll" => {
        let args = self.find_async_args(args)?;
        (CallKind::AsyncPoll, args)
      }
      def_path => {
        if resolved_def_id.is_local() {
          (CallKind::Direct, args)
        } else {
          trace!("  Bailing because func is non-local: `{def_path}`");
          return None;
        }
      }
    };
    trace!("  Handling call!");

    let call_string = self.make_call_string(location);
    if let Some(call_filter) = &self.params.call_filter {
      if !call_filter(resolved_fn, call_string) {
        trace!("  Bailing because user callback said to bail");
        return None;
      }
    }

    // Recursively generate the PDG for the child function.
    let params = PdgParams {
      root: resolved_fn,
      ..self.params.clone()
    };
    let call_stack = match &self.calling_context {
      Some(cx) => {
        let mut stack = cx.call_stack.clone();
        stack.push(resolved_def_id);
        stack
      }
      None => vec![resolved_def_id],
    };
    let calling_context = CallingContext {
      call_string,
      param_env,
      call_stack,
    };
    let child_constructor = GraphConstructor::new(params, Some(calling_context));
    let child_graph = child_constructor.construct_partial();

    // A helper to translate an argument (or return) in the child into a place in the parent.
    let parent_body = &self.body;
    let translate_to_parent = |child: Place<'tcx>| -> Option<Place<'tcx>> {
      trace!("  Translating child place: {child:?}");
      let (parent_place, child_projection) = if child.local == RETURN_PLACE {
        (destination, &child.projection[..])
      } else {
        match call_kind {
          // Map arguments to the argument array
          CallKind::Direct => (
            args[child.local.as_usize() - 1].place()?,
            &child.projection[..],
          ),
          // Map arguments to projections of the future, the poll's first argument
          CallKind::AsyncPoll => {
            if child.local.as_usize() == 1 {
              let PlaceElem::Field(idx, _) = child.projection[0] else {
                panic!("Unexpected non-projection of async context")
              };
              (args[idx.as_usize()].place()?, &child.projection[1 ..])
            } else {
              return None;
            }
          }
          // Map closure captures to the first argument.
          // Map formal parameters to the second argument.
          CallKind::Indirect => {
            if child.local.as_usize() == 1 {
              (args[0].place()?, &child.projection[..])
            } else {
              let tuple_arg = args[1].place()?;
              let _projection = child.projection.to_vec();
              let field = FieldIdx::from_usize(child.local.as_usize() - 2);
              let field_ty = tuple_arg.ty(parent_body.as_ref(), tcx).field_ty(tcx, field);
              (
                tuple_arg.project_deeper(&[PlaceElem::Field(field, field_ty)], tcx),
                &child.projection[..],
              )
            }
          }
        }
      };

      let parent_place_projected = parent_place.project_deeper(child_projection, tcx);
      trace!("    Translated to: {parent_place_projected:?}");
      Some(utils::retype_place(
        parent_place_projected,
        self.tcx,
        parent_body,
        self.def_id.to_def_id(),
      ))
    };

    // Find every reference to a parent-able node in the child's graph.
    let is_arg = |node: &DepNode<'tcx>| {
      node.at.leaf().function == child_constructor.def_id
          && (node.place.local == RETURN_PLACE
          || node.place.is_arg(&child_constructor.body))
    };
    // An attempt at getting immutable arguments to connect
    let parentable_srcs = if self.params.false_call_edges {
      let dummy_state = PartialGraph::default();
      let constructor_ref = &child_constructor;
      Either::Right(child_constructor.body.args_iter()
          .map(|local| Place::from(local))
          .flat_map(move |place| constructor_ref.find_data_inputs(&dummy_state, place)))
    } else {
      Either::Left(child_graph
          .edges
          .iter()
          .map(|(src, _, _)| *src)
          .filter(is_arg)
          .filter(|node| node.at.leaf().location.is_start()))
    };
    let parentable_dsts = child_graph
      .edges
      .iter()
      .map(|(_, dst, _)| *dst)
      .filter(is_arg)
      .filter(|node| node.at.leaf().location.is_end());

    // For each source node CHILD that is parentable to PLACE,
    // add an edge from PLACE -> CHILD.
    for child_src in parentable_srcs {
      if let Some(parent_place) = translate_to_parent(child_src.place) {
        self.apply_mutation(
          state,
          location,
          Either::Right(child_src),
          Either::Left(vec![parent_place]),
          MutationStatus::Possibly,
        );
      }
    }

    // For each destination node CHILD that is parentable to PLACE,
    // add an edge from CHILD -> PLACE.
    //
    // PRECISION TODO: for a given child place, we only want to connect
    // the *last* nodes in the child function to the parent, not *all* of them.
    for child_dst in parentable_dsts {
      if let Some(parent_place) = translate_to_parent(child_dst.place) {
        self.apply_mutation(
          state,
          location,
          Either::Left(parent_place),
          Either::Right(child_dst),
          MutationStatus::Possibly,
        );
      }
    }

    state.edges.extend(child_graph.edges);

    trace!("  Inlined {}", self.fmt_fn(resolved_def_id));

    Some(())
  }

  fn visit_basic_block(&self, block: BasicBlock, state: &mut PartialGraph<'tcx>) {
    macro_rules! visitor {
      () => {
        ModularMutationVisitor::new(&self.place_info, |location, mutations| {
          for mutation in mutations {
            self.apply_mutation(
              state,
              location,
              Either::Left(mutation.mutated),
              Either::Left(mutation.inputs),
              mutation.status,
            );
          }
        })
      };
    }

    // For each statement, register any mutations contained in the statement.
    let block_data = &&self.body.basic_blocks[block];
    for (statement_index, statement) in block_data.statements.iter().enumerate() {
      let location = Location {
        statement_index,
        block,
      };

      visitor!().visit_statement(statement, location);
    }

    let terminator = self.body.basic_blocks[block].terminator();
    let terminator_loc = self.body.terminator_loc(block);
    match &terminator.kind {
      // Special case: if the current block is a SwitchInt, then other blocks could be control-dependent on it.
      // We need to create a node for the value of the discriminant at this point, so control-dependent mutations
      // can use it as a source.
      TerminatorKind::SwitchInt { discr, .. } => {
        if let Some(place) = discr.place() {
          self.apply_mutation(
            state,
            terminator_loc,
            Either::Left(place),
            Either::Left(vec![place]),
            MutationStatus::Possibly,
          );
        }
      }

      // Special case: need to deal with context-sensitivity for function calls.
      TerminatorKind::Call {
        func,
        args,
        destination,
        ..
      } => {
        if self
          .handle_call(state, terminator_loc, func, args, *destination)
          .is_none()
        {
          visitor!().visit_terminator(terminator, terminator_loc)
        }
      }

      // Fallback: call the visitor
      _ => visitor!().visit_terminator(terminator, terminator_loc),
    }
  }

  fn async_generator(body: &Body<'tcx>) -> (LocalDefId, GenericArgsRef<'tcx>, Location) {
    let block = BasicBlock::from_usize(0);
    let location = Location {
      block,
      statement_index: body.basic_blocks[block].statements.len() - 1,
    };
    let stmt = body
      .stmt_at(location)
      .expect_left("Async fn should have a statement");
    let StatementKind::Assign(box (
      _,
      Rvalue::Aggregate(box AggregateKind::Generator(def_id, generic_args, _), _args),
    )) = &stmt.kind
    else {
      panic!("Async fn should assign to a generator")
    };
    (def_id.expect_local(), generic_args, location)
  }

  fn construct_partial(&self) -> PartialGraph<'tcx> {
    if self.tcx.asyncness(self.def_id).is_async() {
      let (generator_def_id, generic_args, location) = Self::async_generator(&self.body);
      let param_env = self.tcx.param_env(self.def_id);
      let generator_fn = utils::try_resolve_function(
        self.tcx,
        generator_def_id.to_def_id(),
        param_env,
        generic_args,
      );
      let params = PdgParams {
        root: generator_fn,
        ..self.params.clone()
      };
      let call_string = self.make_call_string(location);
      let call_stack = match &self.calling_context {
        Some(cx) => cx.call_stack.clone(),
        None => vec![],
      };
      let calling_context = CallingContext {
        param_env,
        call_string,
        call_stack,
      };
      return GraphConstructor::new(params, Some(calling_context)).construct_partial();
    }

    let bb_graph = &self.body.basic_blocks;
    let blocks =
      rustc_graph::iterate::reverse_post_order(bb_graph, bb_graph.start_node());

    let empty = PartialGraph::default();
    let mut domains = IndexVec::from_elem_n(empty.clone(), bb_graph.len());

    if self.params.false_call_edges {
      let start_domain = &mut domains[0_u32.into()];
      for arg in self.body.args_iter() {
        let place = Place::from(arg);
        for mutation in self.find_data_inputs(start_domain, place) {
          start_domain.last_mutation
              .entry(mutation.place)
              .or_default()
              .insert(RichLocation::Start);
        }
        // for child in self.place_info.children(place).iter().copied() {
        //   let ty = child.ty(self.body.as_ref(), self.tcx);
        //   if !ty.ty.is_mutable_ptr() {
        //     continue;
        //   }
        //   let target = child.project_deeper(&[PlaceElem::Deref], self.tcx);
        //   let initial = start_domain.last_mutation.entry(target).or_default();
        //   initial.insert(RichLocation::Start);
        // }
      }
    }

    for block in blocks {
      for parent in bb_graph.predecessors()[block].iter() {
        let (child, parent) = domains.pick2_mut(block, *parent);
        child.join(parent);
      }

      self.visit_basic_block(block, &mut domains[block]);
    }

    let mut final_state = empty;

    let all_returns = self.body.all_returns().map(|ret| ret.block).collect_vec();
    let has_return = !all_returns.is_empty();
    let blocks = if has_return {
      all_returns.clone()
    } else {
      self.body.basic_blocks.indices().collect_vec()
    };
    for block in blocks {
      final_state.join(&domains[block]);
    }

    if has_return {
      for block in all_returns {
        let return_state = &domains[block];
        for (place, locations) in &return_state.last_mutation {
          if place.local == RETURN_PLACE || self.is_ptr_argument_mutation(*place) {
            for location in locations {
              let src = self.make_dep_node(*place, *location);
              let dst = self.make_dep_node(*place, RichLocation::End);
              let edge =
                DepEdge::data(self.make_call_string(self.body.terminator_loc(block)));
              final_state.edges.insert((src, dst, edge));
            }
          }
        }
      }
    }

    final_state
  }

  fn is_ptr_argument_mutation(&self, place: Place<'tcx>) -> bool {
    place.is_arg(&self.body)
    //     && place.iter_projections().any(|(place_ref, projection)|
    //       projection == PlaceElem::Deref && place_ref.ty(self.body.as_ref(), self.tcx).ty.is_mutable_ptr()
    // )
  }

  fn domain_to_petgraph(self, domain: PartialGraph<'tcx>) -> DepGraph<'tcx> {
    let mut graph: DiGraph<DepNode<'tcx>, DepEdge> = DiGraph::new();
    let mut nodes = FxHashMap::default();
    macro_rules! add_node {
      ($n:expr) => {
        *nodes.entry($n).or_insert_with(|| graph.add_node($n))
      };
    }
    for (src, dst, kind) in domain.edges {
      let src_idx = add_node!(src);
      let dst_idx = add_node!(dst);
      graph.add_edge(src_idx, dst_idx, kind);
    }

    DepGraph::new(graph)
  }

  pub fn construct(self) -> DepGraph<'tcx> {
    let partial = self.construct_partial();
    self.domain_to_petgraph(partial)
  }
}
