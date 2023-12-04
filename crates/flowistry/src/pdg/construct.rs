use std::cell::RefCell;

use df::{Analysis, JoinSemiLattice};
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
    visit::Visitor, AggregateKind, BasicBlock, Body, HasLocalDecls, Local, Location,
    Operand, Place, PlaceElem, ProjectionElem, Rvalue, Statement, StatementKind,
    Terminator, TerminatorKind, RETURN_PLACE,
  },
  ty::{EarlyBinder, GenericArg, List, ParamEnv, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{self as df};
use rustc_utils::{
  mir::{borrowck_facts, control_dependencies::ControlDependencies},
  BodyExt, PlaceExt,
};

use super::{
  graph::{CallString, DepEdge, DepGraph, DepNode, GlobalLocation, LocationOrStart},
  value::{ArgValues, Value, ValueAnalysis, ValueDomain},
};
use crate::{
  infoflow::mutation::{ModularMutationVisitor, MutationStatus},
  mir::placeinfo::PlaceInfo,
  pdg::{
    graph::DepEdgeKind,
    utils::{try_monomorphize, FnResolution},
    value::Fields,
  },
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

#[derive(Clone)]
pub struct CallingContext<'tcx> {
  arg_values: ArgValues<'tcx>,
  call_string: CallString,
  param_env: ParamEnv<'tcx>,
}

impl CallingContext<'_> {
  pub fn empty() -> Self {
    CallingContext {
      arg_values: Default::default(),
      call_string: CallString::new(Vec::new()),
      param_env: ParamEnv::empty(),
    }
  }
}

type BodyAssignments = FxHashMap<Local, Vec<Location>>;

fn find_body_assignments(body: &Body<'_>) -> BodyAssignments {
  body
    .all_locations()
    .filter_map(|location| match body.stmt_at(location) {
      Either::Left(Statement {
        kind: StatementKind::Assign(box (lhs, _)),
        ..
      }) => Some((lhs.as_local()?, location)),
      Either::Right(Terminator {
        kind: TerminatorKind::Call { destination, .. },
        ..
      }) => Some((destination.as_local()?, location)),
      _ => None,
    })
    .into_group_map()
    .into_iter()
    .collect()
}

pub struct GraphConstructor<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  body: &'tcx Body<'tcx>,
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
  calling_context: CallingContext<'tcx>,
  body_assignments: BodyAssignments,
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
  pub fn new(
    tcx: TyCtxt<'tcx>,
    fn_resolution: FnResolution<'tcx>,
    calling_context: CallingContext<'tcx>,
  ) -> Self {
    let def_id = fn_resolution.def_id().expect_local();
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    let mut body = body_with_facts.body.clone();

    if let FnResolution::Final(inst) = fn_resolution {
      body = inst.subst_mir_and_normalize_erasing_regions(
        tcx,
        calling_context.param_env,
        EarlyBinder::bind(body),
      );
    }

    let body = tcx.arena.alloc(body);
    debug!("{}", body.to_string(tcx).unwrap());

    let place_info = PlaceInfo::build(tcx, def_id.to_def_id(), body_with_facts);
    let control_dependencies = body.control_dependencies();

    let mut start_loc = FxHashSet::default();
    start_loc.insert(LocationOrStart::Start);

    let values = RefCell::new(
      ValueAnalysis::new(calling_context.arg_values.clone())
        .into_engine(tcx, body)
        .iterate_to_fixpoint()
        .into_results_cursor(body),
    );

    let body_assignments = find_body_assignments(&body);

    GraphConstructor {
      tcx,
      body_with_facts,
      body,
      place_info,
      control_dependencies,
      start_loc,
      def_id,
      values,
      calling_context,
      body_assignments,
    }
  }

  fn body(&self) -> &Body<'tcx> {
    &self.body
  }

  fn make_global_loc(&self, location: impl Into<LocationOrStart>) -> GlobalLocation {
    GlobalLocation {
      function: self.def_id,
      location: location.into(),
    }
  }

  fn make_call_string(&self, location: impl Into<LocationOrStart>) -> CallString {
    self
      .calling_context
      .call_string
      .extend(self.make_global_loc(location))
  }

  fn find_control_inputs(&self, location: Location) -> Vec<(DepNode<'tcx>, DepEdge)> {
    match self.control_dependencies.dependent_on(location.block) {
      Some(ctrl_deps) => ctrl_deps
        .iter()
        .filter_map(|block| {
          let ctrl_loc = self.body().terminator_loc(block);
          let Terminator {
            kind: TerminatorKind::SwitchInt { discr, .. },
            ..
          } = self.body().stmt_at(ctrl_loc).unwrap_right()
          else {
            return None;
          };
          let ctrl_place = discr.place()?;
          let at = self.make_call_string(ctrl_loc);
          let src = DepNode {
            place: ctrl_place,
            at,
          };
          let edge = DepEdge {
            kind: DepEdgeKind::Control,
            at,
          };
          Some((src, edge))
        })
        .collect_vec(),
      None => Vec::new(),
    }
  }

  fn find_data_inputs(
    &self,
    state: &mut PartialGraph<'tcx>,
    input: Place<'tcx>,
  ) -> Vec<DepNode<'tcx>> {
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
    aliases
      .iter()
      .chain(provenance)
      .flat_map(|alias| {
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
              *alias,
              PlaceConflictBias::Overlap,
            )
          })
          .map(|place| (*place, &state.last_mutation[place]));

        // Special case: if the `alias` is an un-mutated argument, then include it as a conflict
        // coming from the special start location.
        let alias_last_mut = if alias.is_arg(self.body()) {
          Some((*alias, &self.start_loc))
        } else {
          None
        };

        // For each `conflict`` last mutated at the locations `last_mut`:
        conflicts
          .chain(alias_last_mut)
          .flat_map(|(conflict, last_mut)| {
            // For each last mutated location:
            last_mut.iter().map(move |loc| {
              // Add an edge from (CONFLICT @ LAST_MUT_LOC) -> OP.
              DepNode {
                place: conflict,
                at: self.make_call_string(*loc),
              }
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
    let dsts = self.place_info.aliases(mutated);

    // **FIELD-SENSITIVITY:** we do NOT deal with fields on *writes* (in this function),
    // only on *reads* (in `add_input_to_op`).

    // For each mutated `dst`:
    dsts
      .iter()
      .map(|dst| {
        // Add an edge from OP -> (DST @ CURRENT_LOC).
        let dst_node = DepNode {
          place: *dst,
          at: self.make_call_string(location),
        };

        // **STRONG UPDATES:**
        // If the mutated place has no aliases AND the mutation definitely occurs,
        // then clear all previous locations of mutation.
        let dst_mutations = state.last_mutation.entry(*dst).or_default();
        if dsts.len() == 1 && matches!(status, MutationStatus::Definitely) {
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
    // **CONTROL-DEPENDENCE:**
    // Add control edges from blocks CTRL -> OP where OP is control-dependent on CTRL.
    let ctrl_inputs = self.find_control_inputs(location);

    // **DATA-DEPENDENCE:**
    // For each mutation, create a data chain for INPUT -> OP -> MUTATED.
    let data_inputs = match inputs {
      Either::Left(places) => places
        .into_iter()
        .flat_map(|input| self.find_data_inputs(state, input))
        .collect::<Vec<_>>(),
      Either::Right(node) => vec![node],
    };
    let outputs = match mutated {
      Either::Left(place) => self.find_and_update_outputs(state, place, status, location),
      Either::Right(node) => vec![node],
    };

    let data_edge = DepEdge {
      kind: DepEdgeKind::Data,
      at: self.make_call_string(location),
    };
    for data_input in data_inputs {
      for output in &outputs {
        state.edges.insert((data_input, *output, data_edge));
      }
    }

    for (ctrl_input, edge) in &ctrl_inputs {
      for output in &outputs {
        state.edges.insert((*ctrl_input, *output, *edge));
      }
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
          let field = adt_def
            .all_fields()
            .nth(field.as_usize())
            .unwrap_or_else(|| {
              panic!("ADT for {:?} does not have field {field:?}", ty.ty);
            });
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
        elem,
        |_, field, _| match ty.ty.kind() {
          TyKind::Closure(_, args) => {
            let upvar_tys = args.as_closure().upvar_tys();
            upvar_tys.iter().nth(field.as_usize()).unwrap()
          }
          _ => ty.field_ty(tcx, field),
        },
        |_, ty| ty,
      );
      let elem = match elem {
        ProjectionElem::Field(field, _) => ProjectionElem::Field(*field, ty.ty),
        elem => *elem,
      };
      projection.push(elem);
    }

    Place::make(parent_place.local, &projection, tcx)
  }

  fn find_indirect_call<'b>(
    &'b self,
    args: &'b [Operand<'tcx>],
    values: &'b ValueDomain<'tcx>,
  ) -> Option<(
    &'b IndexVec<FieldIdx, Operand<'tcx>>,
    LocalDefId,
    &'b IndexVec<FieldIdx, Operand<'tcx>>,
  )> {
    trylet!(
      Some(func_place) = args[0].place(),
      "Func argument is not a place: {:?}",
      args[0]
    );
    trylet!(
      Some(arg_place) = args[1].place(),
      "Arg argument is not a place: {:?}",
      args[1]
    );
    trylet!(
      Some(arg_local) = arg_place.as_local(),
      "Arg argument is not a local: {arg_place:?}"
    );
    let aliases = self.place_info.aliases(self.tcx.mk_place_deref(func_place));
    if aliases.len() != 1 {
      trace!("More than one alias for func place: {func_place:?}");
      return None;
    }
    trylet!(
      Some(borrowed_local) = aliases.iter().next().unwrap().as_local(),
      "Borrowed func is not a local"
    );
    trylet!(
      Some(Value::FunctionDef { def_id, env }) = values.value(borrowed_local),
      "Borrowed func does not have a known value: {borrowed_local:?}",
    );
    trylet!(
      Some(local_def_id) = def_id.as_local(),
      "Borrowed func is not a local function",
    );
    trylet!(
      Some(Value::Tuple(args)) = values.value(arg_local),
      "Arg local does not have a value"
    );
    Some((env, local_def_id, args))
  }

  fn find_async_future<'b>(
    &'b self,
    args: &'b [Operand<'tcx>],
  ) -> Option<(&'b Operand<'tcx>, &'b [Operand<'tcx>])> {
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
      }) = self.body().stmt_at(get_def_for_op(&args[0])?),
      "Pinned assignment is not a call"
    );
    debug_assert!(new_pin_args.len() == 1);

    let future_aliases = self
      .place_info
      .aliases(self.tcx.mk_place_deref(new_pin_args[0].place().unwrap()));
    debug_assert!(future_aliases.len() == 1);
    let future = *future_aliases.iter().next().unwrap();

    trylet!(
      Either::Left(Statement {
        kind: StatementKind::Assign(box (_, Rvalue::Use(future2))),
        ..
      }) = self.body().stmt_at(get_def_for_op(&Operand::Move(future))?),
      "Assignment to pin::new input is not a statement"
    );

    trylet!(
      Either::Right(Terminator {
        kind: TerminatorKind::Call {
          args: into_future_args,
          ..
        },
        ..
      }) = self.body().stmt_at(get_def_for_op(future2)?),
      "Assignment to alias of pin::new input is not a call"
    );

    trylet!(
      Either::Right(Terminator {
        kind: TerminatorKind::Call { func, args, .. },
        ..
      }) = self.body().stmt_at(get_def_for_op(&into_future_args[0])?),
      "Assignment to into_future input is not a call"
    );

    Some((func, args))
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
    let (child_def_id, generic_args) = self.resolve_func(func)?;

    if tcx.asyncness(child_def_id).is_async() {
      trace!("Bailing from handle_call because func is async");
      return Some(());
    }

    let param_env = tcx.param_env(self.def_id);
    let child_fn = try_monomorphize(self.tcx, child_def_id, param_env, generic_args);
    let child_def_id = child_fn.def_id();

    let mut value_analysis = self.values.borrow_mut();
    value_analysis.seek_before_primary_effect(location);
    let values = value_analysis.get();

    // Only consider functions defined in the current crate.
    // We can't access their bodies otherwise.
    // LONG-TERM TODO: could load a serialized version of their graphs.
    enum CallKind<'a, 'tcx> {
      Direct(&'a [Operand<'tcx>]),
      Indirect(&'a Fields<'tcx>, &'a Fields<'tcx>),
      AsyncPoll(&'a [Operand<'tcx>]),
    }
    let (_child_local_def_id, args) = match child_def_id.as_local() {
      Some(local_def_id) => (local_def_id, CallKind::Direct(args)),
      None => match tcx.def_path_str(child_def_id).as_str() {
        "std::ops::Fn::call" => {
          let (env, local_def_id, args) = self.find_indirect_call(args, values)?;
          (local_def_id, CallKind::Indirect(env, args))
        }
        "std::future::Future::poll" => {
          let (_func, _args) = self.find_async_future(args)?;
          todo!()
          // let local_def_id = self.resolve_func(func)?.as_local()?;
          // let _body_id = self.tcx.hir().body_owned_by(local_def_id);
          // let body_with_facts =
          //   borrowck_facts::get_body_with_borrowck_facts(self.tcx, local_def_id);
          // let (generator_def_id, _) = Self::async_generator(&body_with_facts.body);
          // (generator_def_id, CallKind::AsyncPoll(args))
        }
        def_path => {
          trace!("Bailing from handle_call because func is non-local: `{def_path}`");
          return None;
        }
      },
    };

    // Summarize known facts about arguments.
    let child_args = |n| (0 .. n).map(|i| Local::from_usize(i + 1));
    let args_iter = match args {
      CallKind::Direct(args) | CallKind::AsyncPoll(args) => {
        args.iter().zip(child_args(args.len())).collect_vec()
      }
      CallKind::Indirect(_, args) => args
        .iter()
        .zip(child_args(args.len() + 1).skip(1))
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
            // TODO: issue is that converting closure environment into child places
            // is ill-typed due to generics. Eg (x: [<closure>]).0 is defined in the parent,
            // but (x: impl Fn).0 is not defined in the child.
            if !env.is_empty() {
              return None;
            }

            let child_env: IndexVec<FieldIdx, Operand<'_>> =
              IndexVec::from_iter(env.iter_enumerated().map(|(idx, op)| match op {
                Operand::Constant(_) => op.clone(),
                Operand::Copy(place) | Operand::Move(place) => {
                  let ty =
                    tcx.erase_regions(place.ty(self.body().local_decls(), self.tcx));
                  let child_place = Place::make(
                    child_local,
                    &[PlaceElem::Deref, PlaceElem::Field(idx, ty.ty)],
                    self.tcx,
                  );
                  Operand::Move(child_place)
                }
              }));
            Value::FunctionDef {
              def_id: *def_id,
              env: child_env,
            }
          }
          Value::Tuple(_places) => return None,
        };
        Some((child_local, child_def))
      })
      .collect::<FxHashMap<_, _>>();

    let call_string = self.make_call_string(location);
    let calling_context = CallingContext {
      arg_values,
      call_string,
      param_env,
    };

    // Recursively generate the PDG for the child function.
    let child_constructor = GraphConstructor::new(tcx, child_fn, calling_context);
    let child_graph = child_constructor.construct_partial();

    // A helper to translate an argument (or return) in the child into a place in the parent.
    // The major complexity is translating *projections* from the child to the parent.
    let parent_body = self.body();
    let translate_to_parent = |child: Place<'tcx>| -> Option<Place<'tcx>> {
      let (parent_place, child_suffix) = if child.local == RETURN_PLACE {
        (destination, &child.projection[..])
      } else {
        match args {
          CallKind::Direct(args) => (
            args[child.local.as_usize() - 1].place()?,
            &child.projection[..],
          ),
          CallKind::AsyncPoll(args) => {
            if child.local.as_usize() == 1 {
              let PlaceElem::Field(idx, _) = child.projection[0] else {
                panic!("Unexpected non-projection of async context")
              };
              (args[idx.as_usize()].place()?, &child.projection[1 ..])
            } else {
              return None;
            }
          }
          CallKind::Indirect(env, args) => {
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
        param_env,
      ))
    };

    // Find every reference to a parent-able node in the child's graph.
    let is_arg = |DepNode { place, .. }: &DepNode<'tcx>| {
      place.local == RETURN_PLACE || place.is_arg(child_constructor.body())
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

  fn async_generator(body: &Body) -> (LocalDefId, Location) {
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
      Rvalue::Aggregate(box AggregateKind::Generator(def_id, _, _), _args),
    )) = &stmt.kind
    else {
      panic!("Async fn should assign to a generator")
    };
    (def_id.expect_local(), location)
  }

  fn construct_partial(&self) -> PartialGraph<'tcx> {
    if self.tcx.asyncness(self.def_id).is_async() {
      let (local_def_id, _location) = Self::async_generator(self.body());
      let calling_context = CallingContext::empty();
      return GraphConstructor::new(
        self.tcx,
        // TODO
        FnResolution::Partial(local_def_id.to_def_id()),
        calling_context,
      )
      .construct_partial();
    }

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
