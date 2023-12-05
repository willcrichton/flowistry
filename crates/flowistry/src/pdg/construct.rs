use std::{borrow::Cow, iter};

use df::JoinSemiLattice;
use either::Either;
use itertools::Itertools;
use log::{debug, trace};
use petgraph::graph::DiGraph;
use rustc_abi::FieldIdx;
use rustc_borrowck::consumers::{
  places_conflict, BodyWithBorrowckFacts, PlaceConflictBias,
};
use rustc_data_structures::graph::{self as rustc_graph, WithStartNode};
use rustc_hash::{FxHashMap, FxHashSet};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_index::IndexVec;
use rustc_middle::{
  mir::{
    visit::Visitor, AggregateKind, BasicBlock, Body, Local, Location, Operand, Place,
    PlaceElem, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
    RETURN_PLACE,
  },
  ty::{GenericArg, GenericArgsRef, List, ParamEnv, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{self as df};
use rustc_utils::{
  mir::{borrowck_facts, control_dependencies::ControlDependencies},
  BodyExt, PlaceExt,
};

use super::graph::{
  CallString, DepEdge, DepGraph, DepNode, GlobalLocation, LocationOrStart,
};
use crate::{
  infoflow::mutation::{ModularMutationVisitor, MutationStatus},
  mir::placeinfo::PlaceInfo,
  pdg::utils::{self, try_resolve_function, FnResolution},
};

#[derive(PartialEq, Eq, Default, Clone)]
pub struct PartialGraph<'tcx> {
  edges: FxHashSet<(DepNode<'tcx>, DepNode<'tcx>, DepEdge)>,
  last_mutation: FxHashMap<Place<'tcx>, FxHashSet<LocationOrStart>>,
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

#[derive(Clone)]
struct CallingContext<'tcx> {
  call_string: CallString,
  param_env: ParamEnv<'tcx>,
}

impl CallingContext<'_> {
  pub fn empty() -> Self {
    CallingContext {
      call_string: CallString::new(Vec::new()),
      param_env: ParamEnv::empty(),
    }
  }
}

pub struct GraphConstructor<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  body: Cow<'tcx, Body<'tcx>>,
  place_info: PlaceInfo<'a, 'tcx>,
  control_dependencies: ControlDependencies<BasicBlock>,
  start_loc: FxHashSet<LocationOrStart>,
  def_id: LocalDefId,
  calling_context: CallingContext<'tcx>,
  body_assignments: utils::BodyAssignments,
}

macro_rules! trylet {
  ($p:pat = $e:expr, $($arg:tt)*) => {
    let $p = $e else {
      trace!($($arg)*);
      return None;
    };
  }
}

impl<'a, 'tcx> GraphConstructor<'a, 'tcx> {
  /// Creates a [`GraphConstructor`] assuming that `def_id` is the root of the PDG.
  pub fn root(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> Self {
    Self::new(
      tcx,
      FnResolution::Partial(def_id.to_def_id()),
      CallingContext::empty(),
    )
  }

  /// Creates [`GraphConstructor`] for a function resolved as `fn_resolution` in a given `calling_context`.
  fn new(
    tcx: TyCtxt<'tcx>,
    fn_resolution: FnResolution<'tcx>,
    calling_context: CallingContext<'tcx>,
  ) -> Self {
    let def_id = fn_resolution.def_id().expect_local();
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    let body = utils::try_monomorphize(
      tcx,
      fn_resolution,
      calling_context.param_env,
      &body_with_facts.body,
    );
    debug!("{}", body.to_string(tcx).unwrap());

    let place_info = PlaceInfo::build(tcx, def_id.to_def_id(), body_with_facts);
    let control_dependencies = body.control_dependencies();

    let mut start_loc = FxHashSet::default();
    start_loc.insert(LocationOrStart::Start);

    let body_assignments = utils::find_body_assignments(&body);

    GraphConstructor {
      tcx,
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
  fn make_global_loc(&self, location: impl Into<LocationOrStart>) -> GlobalLocation {
    GlobalLocation {
      function: self.def_id,
      location: location.into(),
    }
  }

  /// Creates a [`CallString`] with the current function at the root,
  /// with the rest of the string provided by the [`CallingContext`].
  fn make_call_string(&self, location: impl Into<LocationOrStart>) -> CallString {
    self
      .calling_context
      .call_string
      .extend(self.make_global_loc(location))
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
    state: &mut PartialGraph<'tcx>,
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
            places_conflict(
              self.tcx,
              &self.body,
              **place,
              alias,
              PlaceConflictBias::Overlap,
            )
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
        dst_mutations.insert(LocationOrStart::Location(location));

        dst_node
      })
      .collect()
  }

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

    let data_edge = DepEdge::data(self.make_call_string(location));
    for data_input in data_inputs {
      for output in &outputs {
        trace!("Adding edge {data_input:?} -> {output:?}");
        state.edges.insert((data_input, *output, data_edge));
      }
    }

    for (ctrl_input, edge) in &ctrl_inputs {
      for output in &outputs {
        state.edges.insert((*ctrl_input, *output, *edge));
      }
    }
  }

  fn find_async_args<'b>(
    &'b self,
    args: &'b [Operand<'tcx>],
  ) -> Option<&'b [Operand<'tcx>]> {
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
    let future = *future_aliases.iter().next().unwrap();

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

  fn resolve_func(
    &self,
    func: &Operand<'tcx>,
  ) -> Option<(DefId, &'tcx List<GenericArg<'tcx>>)> {
    // Figure out which function the `func` is referring to, if possible.
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

  fn format_def_id(&self, def_id: DefId) -> String {
    self.tcx.def_path_str(def_id)
  }

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

    // Figure out which function the `func` is referring to, if possible.
    let (called_def_id, generic_args) = self.resolve_func(func)?;
    trace!(
      "Resolved call to function: {}",
      self.format_def_id(called_def_id)
    );

    if tcx.asyncness(called_def_id).is_async() {
      trace!("  Bailing from handle_call because func is async");
      return Some(());
    }

    let param_env = tcx.param_env(self.def_id);
    let resolved_fn =
      try_resolve_function(self.tcx, called_def_id, param_env, generic_args);
    let resolved_def_id = resolved_fn.def_id();
    if called_def_id != resolved_def_id {
      trace!(
        "  `{}` monomorphized to `{}`",
        self.format_def_id(called_def_id),
        self.format_def_id(resolved_def_id)
      );
    }

    // let mut value_analysis = self.values.borrow_mut();
    // value_analysis.seek_before_primary_effect(location);
    // let values = value_analysis.get();

    // Only consider functions defined in the current crate.
    // We can't access their bodies otherwise.
    // LONG-TERM TODO: could load a serialized version of their graphs.
    enum CallKind {
      Direct,
      Indirect,
      AsyncPoll,
    }

    // Note: monomorphization will resolve `poll(future)` directly to `<generator@future>`,
    // so we look for direct calls to an async generator.

    let (call_kind, args) = match tcx.def_path_str(called_def_id).as_str() {
      "std::ops::Fn::call" => (CallKind::Indirect, args),
      "std::future::Future::poll" => {
        let args = self.find_async_args(args)?;
        (CallKind::AsyncPoll, args)
      }
      def_path => match resolved_def_id.as_local() {
        Some(_local_def_id) => (CallKind::Direct, args),
        None => {
          trace!("  Bailing because func is non-local: `{def_path}`");
          return None;
        }
      },
    };

    trace!("  Handling call!");

    let call_string = self.make_call_string(location);
    let calling_context = CallingContext {
      call_string,
      param_env,
    };

    // Recursively generate the PDG for the child function.
    let child_constructor = GraphConstructor::new(tcx, resolved_fn, calling_context);
    let child_graph = child_constructor.construct_partial();

    // A helper to translate an argument (or return) in the child into a place in the parent.
    // The major complexity is translating *projections* from the child to the parent.
    let parent_body = &self.body;
    let translate_to_parent = |child: Place<'tcx>| -> Option<Place<'tcx>> {
      trace!("  Translating child place: {child:?}");
      let (parent_place, child_projection) = if child.local == RETURN_PLACE {
        (destination, &child.projection[..])
      } else {
        match call_kind {
          CallKind::Direct => (
            args[child.local.as_usize() - 1].place()?,
            &child.projection[..],
          ),
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
      node.at.root().function == child_constructor.def_id
        && (node.place.local == RETURN_PLACE
          || node.place.is_arg(&child_constructor.body))
    };
    let parentable_srcs = child_graph
      .edges
      .iter()
      .map(|(src, _, _)| *src)
      .filter(is_arg);
    let parentable_dsts = child_graph
      .edges
      .iter()
      .map(|(_, dst, _)| *dst)
      .filter(is_arg);

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

    trace!("  Inlined {}", self.format_def_id(resolved_def_id));

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
      let (generator_def_id, generic_args, _location) = Self::async_generator(&self.body);
      let param_env = self.tcx.param_env(self.def_id);
      let generator_fn = try_resolve_function(
        self.tcx,
        generator_def_id.to_def_id(),
        param_env,
        generic_args,
      );
      let calling_context = CallingContext::empty();
      return GraphConstructor::new(self.tcx, generator_fn, calling_context)
        .construct_partial();
    }

    let bb_graph = &self.body.basic_blocks;
    let blocks =
      rustc_graph::iterate::reverse_post_order(bb_graph, bb_graph.start_node());

    let bot = PartialGraph::default();
    let mut domains = IndexVec::<BasicBlock, _>::from_elem_n(bot, bb_graph.len());
    for block in blocks {
      for parent in bb_graph.predecessors()[block].iter() {
        let (child, parent) = domains.pick2_mut(block, *parent);
        child.join(parent);
      }

      self.visit_basic_block(block, &mut domains[block]);
    }

    let mut all_returns = self.body.all_returns();
    let Some(first_return) = all_returns.next() else {
      todo!("what to do with ! type blocks?")
    };
    for other_return in all_returns {
      let (first, other) = domains.pick2_mut(first_return.block, other_return.block);
      first.join(other);
    }

    domains[first_return.block].clone()
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
