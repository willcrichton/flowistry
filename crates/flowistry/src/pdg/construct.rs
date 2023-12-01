use std::cell::RefCell;

use df::{lattice::FlatSet, Analysis, JoinSemiLattice};
use itertools::Itertools;
use log::{debug, trace};
use petgraph::graph::DiGraph;
use rustc_abi::FieldIdx;
use rustc_borrowck::consumers::{
  places_conflict, BodyWithBorrowckFacts, PlaceConflictBias,
};
use rustc_data_structures::graph::{self as rustc_graph, WithStartNode};
use rustc_hash::{FxHashMap, FxHashSet};
use rustc_hir::{
  def_id::{DefId, LocalDefId},
  BodyId,
};
use rustc_index::IndexVec;
use rustc_middle::{
  mir::{
    visit::Visitor, BasicBlock, Body, HasLocalDecls, Local, Location, Operand, Place,
    PlaceElem, ProjectionElem, TerminatorKind, RETURN_PLACE,
  },
  ty::{ParamEnv, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{self as df};
use rustc_utils::{
  mir::{borrowck_facts, control_dependencies::ControlDependencies},
  BodyExt, PlaceExt,
};

use super::{
  graph::{CallString, DepEdge, DepGraph, DepNode, GlobalLocation, LocationOrStart},
  value::{ArgValues, Value, ValueAnalysis},
};
use crate::{
  infoflow::mutation::{ModularMutationVisitor, Mutation, MutationStatus},
  mir::placeinfo::PlaceInfo,
  pdg::value::Fields,
};

#[derive(PartialEq, Eq, Default, Clone)]
pub struct PartialGraph<'tcx> {
  edges: FxHashSet<(DepNode<'tcx>, DepNode<'tcx>, DepEdge)>,
  last_mutation: FxHashMap<Place<'tcx>, FxHashSet<LocationOrStart>>,
}

impl<'tcx> df::JoinSemiLattice for PartialGraph<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    let orig_len = self.edges.len();
    self.edges.extend(&other.edges);
    let changed1 = self.edges.len() != orig_len;

    let mut changed2 = false;
    for (place, other_locs) in &other.last_mutation {
      let self_locs = self.last_mutation.entry(*place).or_default();
      let orig_len = self_locs.len();
      self_locs.extend(other_locs);
      changed2 |= orig_len != self_locs.len();
    }

    changed1 || changed2
  }
}

pub struct GraphConstructor<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  place_info: PlaceInfo<'a, 'tcx>,
  control_dependencies: ControlDependencies<BasicBlock>,
  start_loc: FxHashSet<LocationOrStart>,
  def_id: LocalDefId,
  values: RefCell<
    df::ResultsCursor<
      'a,
      'tcx,
      ValueAnalysis<'tcx>,
      df::Results<'tcx, ValueAnalysis<'tcx>>,
    >,
  >,
}

impl<'a, 'tcx> GraphConstructor<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
    arg_values: ArgValues<'tcx>,
  ) -> Self {
    let body = &body_with_facts.body;
    debug!("{}", body.to_string(tcx).unwrap());

    let def_id = tcx.hir().body_owner_def_id(body_id);
    let place_info = PlaceInfo::build(tcx, def_id.to_def_id(), body_with_facts);
    let control_dependencies = body.control_dependencies();

    let mut start_loc = FxHashSet::default();
    start_loc.insert(LocationOrStart::Start);

    let values = RefCell::new(
      ValueAnalysis::new(arg_values)
        .into_engine(tcx, body)
        .iterate_to_fixpoint()
        .into_results_cursor(body),
    );

    GraphConstructor {
      tcx,
      body_with_facts,
      place_info,
      control_dependencies,
      start_loc,
      def_id,
      values,
    }
  }

  fn body(&self) -> &Body<'tcx> {
    &self.body_with_facts.body
  }

  fn make_global_loc(&self, location: impl Into<LocationOrStart>) -> GlobalLocation {
    GlobalLocation {
      function: self.def_id,
      location: location.into(),
    }
  }

  fn make_root_call_string(&self, location: impl Into<LocationOrStart>) -> CallString {
    CallString::new(vec![self.make_global_loc(location)])
  }

  fn add_input_to_op(
    &self,
    state: &mut PartialGraph<'tcx>,
    input: Place<'tcx>,
    op: DepNode<'tcx>,
  ) {
    // **POINTER-SENSITIVITY:**
    // If `input` involves indirection via dereferences, then resolve it to the direct places it could point to.
    let aliases = self.place_info.aliases(input);

    // Include all sources of indirection (each reference in the chain) as relevant places.
    let provenance = input.refs_in_projection().flat_map(|(place_ref, _)| {
      self
        .place_info
        .aliases(Place::from_ref(place_ref, self.tcx))
        .iter()
    });

    // For each input `alias`:
    for alias in aliases.iter().chain(provenance).copied() {
      // **FIELD-SENSITIVITY:**
      // Find all places that have been mutated which conflict with `alias.`
      let conflicts = state
        .last_mutation
        .keys()
        .filter(|place| {
          places_conflict(
            self.tcx,
            self.body(),
            **place,
            alias,
            PlaceConflictBias::Overlap,
          )
        })
        .map(|place| (*place, &state.last_mutation[place]));

      // Special case: if the `alias` is an un-mutated argument, then include it as a conflict
      // coming from the special start location.
      let alias_last_mut = if alias.is_arg(self.body()) {
        Some((alias, &self.start_loc))
      } else {
        None
      };

      // For each `conflict`` last mutated at the locations `last_mut`:
      for (conflict, last_mut) in conflicts.chain(alias_last_mut) {
        // For each last mutated location:
        for loc in last_mut {
          // Add an edge from (CONFLICT @ LAST_MUT_LOC) -> OP.
          let input_node = DepNode::Place {
            place: conflict,
            at: self.make_root_call_string(*loc),
          };
          state.edges.insert((input_node, op, DepEdge::Data));
        }
      }
    }
  }

  fn add_op_to_mutated(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    op_node: DepNode<'tcx>,
    mutated: Place<'tcx>,
    status: MutationStatus,
  ) {
    // **POINTER-SENSITIVITY:**
    // If `mutated` involves indirection via dereferences, then resolve it to the direct places it could point to.
    let dsts = self.place_info.aliases(mutated);

    // **FIELD-SENSITIVITY:** we do NOT deal with fields on *writes* (in this function),
    // only on *reads* (in `add_input_to_op`).

    // For each mutated `dst`:
    for dst in dsts {
      // Add an edge from OP -> (DST @ CURRENT_LOC).
      let dst_node = DepNode::Place {
        place: *dst,
        at: self.make_root_call_string(location),
      };
      state.edges.insert((op_node, dst_node, DepEdge::Data));

      // **STRONG UPDATES:**
      // If the mutated place has no aliases AND the mutation definitely occurs,
      // then clear all previous locations of mutation.
      let dst_mutations = state.last_mutation.entry(*dst).or_default();
      if dsts.len() == 1 && matches!(status, MutationStatus::Definitely) {
        dst_mutations.clear();
      }

      // Register that `dst` is mutated at the current location.
      dst_mutations.insert(LocationOrStart::Location(location));
    }
  }

  fn apply_mutations(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    mutations: Vec<Mutation<'tcx>>,
  ) {
    let op_node = DepNode::Op(self.make_root_call_string(location));

    // **CONTROL-DEPENDENCE:**
    // Add control edges from blocks CTRL -> OP where OP is control-dependent on CTRL.
    if let Some(ctrl_deps) = self.control_dependencies.dependent_on(location.block) {
      let ctrl_edges = ctrl_deps.iter().map(|block| {
        let ctrl_node =
          DepNode::Op(self.make_root_call_string(self.body().terminator_loc(block)));
        (ctrl_node, op_node, DepEdge::Control)
      });
      state.edges.extend(ctrl_edges);
    }

    // **DATA-DEPENDENCE:**
    // For each mutation, create a data chain for INPUT -> OP -> MUTATED.
    for mutation in mutations {
      // Add data edges for each INPUT -> OP.
      for input in &mutation.inputs {
        self.add_input_to_op(state, *input, op_node);
      }

      // Add a data edge for OP -> MUTATED.
      self.add_op_to_mutated(state, location, op_node, mutation.mutated, mutation.status);
    }
  }

  fn move_child_projection_to_parent(
    &self,
    parent_place: Place<'tcx>,
    parent_body: &Body<'tcx>,
    tcx: TyCtxt<'tcx>,
    child_projection: &[PlaceElem<'tcx>],
    parent_param_env: ParamEnv<'tcx>,
  ) -> Place<'tcx> {
    trace!("Moving child projection {child_projection:?} onto parent {parent_place:?}");

    let mut projection = parent_place.projection.to_vec();
    let mut ty = parent_place.ty(parent_body.local_decls(), tcx);

    for elem in child_projection.iter() {
      // Don't continue if we reach a private field
      if let ProjectionElem::Field(field, _) = elem {
        if let Some(adt_def) = ty.ty.ty_adt_def() {
          let field = adt_def.all_fields().nth(field.as_usize()).unwrap();
          if !field.vis.is_accessible_from(self.def_id, self.tcx) {
            break;
          }
        }
      }

      trace!(
        "Projecting {:?}.{projection:?} : {:?} with {elem:?}",
        parent_place.local,
        ty.ty,
      );
      ty = ty.projection_ty_core(
        tcx,
        parent_param_env,
        &elem,
        |_, field, _| ty.field_ty(tcx, field),
        |_, ty| ty,
      );
      let elem = match elem {
        ProjectionElem::Field(field, _) => ProjectionElem::Field(*field, ty.ty),
        elem => *elem,
      };
      projection.push(elem);
    }

    let parent_place_projected = Place::make(parent_place.local, &projection, tcx);
    parent_place_projected
  }

  fn handle_call(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    func: &Operand<'tcx>,
    args: &[Operand<'tcx>],
    destination: Place<'tcx>,
  ) -> bool {
    // Note: my comments here will use "child" to refer to the callee and
    // "parent" to refer to the caller, since the words are most visually distinct.

    let tcx = self.tcx;

    // Figure out which function the `func` is referring to, if possible.
    let child_def_id = match func {
      Operand::Constant(func) => match func.literal.ty().kind() {
        TyKind::FnDef(def_id, _) => *def_id,
        ty => {
          trace!("Bailing from handle_call because func is literal with type: {ty:?}");
          return false;
        }
      },
      Operand::Copy(place) | Operand::Move(place) => {
        // TODO: control-flow analysis to deduce fn for inlined closures
        trace!("Bailing from handle_call because func is place {place:?}");
        return false;
      }
    };

    let mut value_analysis = self.values.borrow_mut();
    value_analysis.seek_before_primary_effect(location);
    let values = value_analysis.get();

    // Only consider functions defined in the current crate.
    // We can't access their bodies otherwise.
    // LONG-TERM TODO: could load a serialized version of their graphs.
    enum CallType<'a, 'tcx> {
      Direct(&'a [Operand<'tcx>]),
      Indirect(&'a Fields<'tcx>, &'a Fields<'tcx>),
    }
    let (child_local_def_id, args) = match child_def_id.as_local() {
      Some(local_def_id) => (local_def_id, CallType::Direct(args)),
      None => {
        if tcx.def_path_str(child_def_id) == "std::ops::Fn::call" {
          let Some(func_place) = args[0].place() else {
            trace!("Func argument is not a place: {:?}", args[0]);
            return false;
          };

          let Some(arg_place) = args[1].place() else {
            trace!("Arg argument is not a place: {:?}", args[1]);
            return false;
          };

          let Some(arg_local) = arg_place.as_local() else {
            trace!("Arg argument is not a local: {arg_place:?}");
            return false;
          };

          let aliases = self.place_info.aliases(self.tcx.mk_place_deref(func_place));
          if aliases.len() != 1 {
            trace!("More than one alias for func place: {func_place:?}");
            return false;
          }

          let Some(borrowed_local) = aliases.iter().next().unwrap().as_local() else {
            trace!("Borrowed func is not a local");
            return false;
          };

          let Some(Value::FunctionDef { def_id, env }) = values.value(borrowed_local)
          else {
            trace!("Borrowed func does not have a known value: {borrowed_local:?}");
            return false;
          };

          let Some(local_def_id) = def_id.as_local() else {
            trace!("Borrowed func is not a local function");
            return false;
          };

          let Some(Value::Tuple(args)) = values.value(arg_local) else {
            trace!("Arg local does not have a value");
            return false;
          };

          (local_def_id, CallType::Indirect(env, args))
        } else {
          trace!("Bailing from handle_call because func is non-local: {child_def_id:?}");
          return false;
        }
      }
    };

    // Get the input facts about the child function.
    let child_body_id = tcx.hir().body_owned_by(child_local_def_id);
    let child_body_with_facts =
      borrowck_facts::get_body_with_borrowck_facts(tcx, child_local_def_id);
    let child_body = &child_body_with_facts.body;
    debug!("{}", child_body.to_string(self.tcx).unwrap());

    // Summarize known facts about arguments.
    let args_iter = match args {
      CallType::Direct(args) => args.iter().zip(child_body.args_iter()).collect_vec(),
      CallType::Indirect(_, args) => args
        .iter()
        .zip(child_body.args_iter().skip(1))
        .collect_vec(),
    };
    let arg_values = args_iter
      .into_iter()
      .filter_map(|(op, child_local)| {
        let parent_place = op.place()?;
        let parent_local = parent_place.as_local()?;
        let def = values.value(parent_local)?;
        let child_def = match def {
          Value::FunctionDef { def_id, env } => {
            return None;
            let child_env =
              IndexVec::from_iter(env.iter_enumerated().map(|(idx, op)| match op {
                Operand::Constant(_) => op.clone(),
                Operand::Copy(place) | Operand::Move(place) => {
                  // TODO
                  todo!()
                }
              }));
            Value::FunctionDef {
              def_id: *def_id,
              env: child_env,
            }
          }
          Value::Tuple(places) => return None,
        };
        Some((child_local, child_def))
      })
      .collect::<FxHashMap<_, _>>();

    // Recursively generate the PDG for the child function.
    let mut child_graph =
      GraphConstructor::new(tcx, child_body_id, child_body_with_facts, arg_values)
        .construct_partial();

    // Add the location of this call to the child's call string.
    let call_loc = self.make_global_loc(location);
    let extend_call_string = |node: DepNode<'tcx>| -> DepNode<'tcx> {
      let extend = |cs: CallString| -> CallString {
        CallString::new(cs.iter().chain([call_loc]).collect::<Vec<_>>())
      };
      match node {
        DepNode::Place { place, at } => DepNode::Place {
          place,
          at: extend(at),
        },
        DepNode::Op(loc) => DepNode::Op(extend(loc)),
      }
    };
    child_graph.edges = child_graph
      .edges
      .into_iter()
      .map(|(src, dst, kind)| (extend_call_string(src), extend_call_string(dst), kind))
      .collect();

    // A helper to translate an argument (or return) in the child into a place in the parent.
    // The major complexity is translating *projections* from the child to the parent.
    let parent_body = self.body();
    let parent_param_env = tcx.param_env(self.def_id);
    let translate_to_parent = |child: Place<'tcx>| -> Option<Place<'tcx>> {
      let (parent_place, child_suffix) = if child.local == RETURN_PLACE {
        (destination, &child.projection[..])
      } else {
        match args {
          CallType::Direct(args) => (
            args[child.local.as_usize() - 1].place()?,
            &child.projection[..],
          ),
          CallType::Indirect(env, args) => {
            if child.local.as_usize() == 1 {
              if child.projection.len() < 2 {
                return None;
              }
              debug_assert!(
                matches!(child.projection[0], PlaceElem::Deref),
                "child: {child:?}"
              );
              let PlaceElem::Field(idx, _) = child.projection[1] else {
                panic!("Unexpected non-projection of closure environment")
              };
              (env[idx].place()?, &child.projection[2 ..])
            } else {
              (
                args[FieldIdx::from_usize(child.local.as_usize() - 2)].place()?,
                &child.projection[..],
              )
            }
          }
        }
      };
      Some(self.move_child_projection_to_parent(
        parent_place,
        parent_body,
        tcx,
        child_suffix,
        parent_param_env,
      ))
    };

    // Find every reference to a parent-able node in the child's graph.
    let is_arg = |child_node: &DepNode<'tcx>| match child_node {
      DepNode::Place { place, .. } => {
        place.local == RETURN_PLACE || place.is_arg(child_body)
      }
      _ => false,
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
    for src in parentable_srcs {
      let child_place = src.expect_place();
      if let Some(parent_place) = translate_to_parent(child_place) {
        self.add_input_to_op(state, parent_place, src);
      }
    }

    // For each destination node CHILD that is parentable to PLACE,
    // add an edge from CHILD -> PLACE.
    //
    // PRECISION TODO: for a given child place, we only want to connect
    // the *last* nodes in the child function to the parent, not *all* of them.
    for dst in parentable_dsts {
      let child_place = dst.expect_place();
      if let Some(parent_place) = translate_to_parent(child_place) {
        self.add_op_to_mutated(
          state,
          location,
          dst,
          parent_place,
          // PRECISION TODO: if `child_place` is guaranteed to have been overwritten,
          // then we can make this into a `Definitely`
          MutationStatus::Possibly,
        );
      }
    }

    state.edges.extend(child_graph.edges);

    true
  }

  fn visit_basic_block(&self, block: BasicBlock, state: &mut PartialGraph<'tcx>) {
    macro_rules! visitor {
      () => {
        ModularMutationVisitor::new(&self.place_info, |location, mutations| {
          self.apply_mutations(state, location, mutations)
        })
      };
    }

    // For each statement, register any mutations contained in the statement.
    let block_data = &self.body().basic_blocks[block];
    for (statement_index, statement) in block_data.statements.iter().enumerate() {
      let location = Location {
        statement_index,
        block,
      };
      visitor!().visit_statement(statement, location);
    }

    let terminator = self.body().basic_blocks[block].terminator();
    let terminator_loc = self.body().terminator_loc(block);
    match &terminator.kind {
      // Special case: if the current block is a SwitchInt, then other blocks could be control-dependent on it.
      // We need to register that the SwitchInt's input is a dependency of the switch operation.
      TerminatorKind::SwitchInt { discr, .. } => {
        if let Some(place) = discr.place() {
          self.add_input_to_op(
            state,
            place,
            DepNode::Op(self.make_root_call_string(terminator_loc)),
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
        if !self.handle_call(state, terminator_loc, func, args, *destination) {
          visitor!().visit_terminator(terminator, terminator_loc)
        }
      }

      // Fallback: call the visitor
      _ => visitor!().visit_terminator(terminator, terminator_loc),
    }
  }

  fn domain_to_petgraph(&self, domain: PartialGraph<'tcx>) -> DepGraph<'tcx> {
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

    DepGraph { graph }
  }

  fn construct_partial(&self) -> PartialGraph<'tcx> {
    let bb_graph = &self.body().basic_blocks;
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

    let mut all_returns = self.body().all_returns();
    let Some(first_return) = all_returns.next() else {
      todo!("what to do with ! type blocks?")
    };
    for other_return in all_returns {
      let (first, other) = domains.pick2_mut(first_return.block, other_return.block);
      first.join(other);
    }

    domains[first_return.block].clone()
  }

  pub fn construct(&self) -> DepGraph<'tcx> {
    self.domain_to_petgraph(self.construct_partial())
  }
}
