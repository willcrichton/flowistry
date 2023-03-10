use log::{debug, info};
use rustc_middle::{
  mir::*,
  ty::{subst::GenericArgKind, ClosureKind, TyKind},
};
use rustc_mir_dataflow::JoinSemiLattice;

use super::{analysis::FlowAnalysis, BODY_STACK};
use crate::{
  extensions::REACHED_LIBRARY,
  infoflow::{
    mutation::{Mutation, MutationStatus},
    FlowDomain,
  },
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{self, PlaceExt},
  },
};

impl<'tcx> FlowAnalysis<'_, 'tcx> {
  pub(crate) fn recurse_into_call(
    &self,
    state: &mut FlowDomain<'tcx>,
    call: &TerminatorKind<'tcx>,
    location: Location,
  ) -> bool {
    let tcx = self.tcx;
    let (func, parent_args, destination) = match call {
      TerminatorKind::Call {
        func,
        args,
        destination,
        ..
      } => (func, args, destination),
      _ => unreachable!(),
    };
    debug!("Checking whether can recurse into {func:?}");

    let func = match func.constant() {
      Some(func) => func,
      None => {
        debug!("  Func is not constant");
        return false;
      }
    };

    let def_id = match func.literal.ty().kind() {
      TyKind::FnDef(def_id, _) => def_id,
      _ => {
        debug!("  Func is not a FnDef");
        return false;
      }
    };

    // If a function returns never (fn () -> !) then there are no exit points,
    // so we can't analyze effects on exit
    let fn_sig = tcx.fn_sig(*def_id);
    if fn_sig.skip_binder().output().is_never() {
      debug!("  Func returns never");
      return false;
    }

    let node = match tcx.hir().get_if_local(*def_id) {
      Some(node) => node,
      None => {
        debug!("  Func is not in local crate");
        REACHED_LIBRARY.get(|reached_library| {
          if let Some(reached_library) = reached_library {
            *reached_library.borrow_mut() = true;
          }
        });
        return false;
      }
    };

    let body_id = match node.body_id() {
      Some(body_id) => body_id,
      None => {
        debug!("  Func does not have a BodyId");
        return false;
      }
    };

    let unsafety = tcx.unsafety_check_result(def_id.expect_local());
    if !unsafety.used_unsafe_blocks.is_empty() {
      debug!("  Func contains unsafe blocks");
      return false;
    }

    let parent_arg_places = utils::arg_places(parent_args);
    let any_closure_inputs = parent_arg_places.iter().any(|(_, place)| {
      let ty = place.ty(self.body.local_decls(), tcx).ty;
      ty.walk().any(|arg| match arg.unpack() {
        GenericArgKind::Type(ty) => match ty.kind() {
          TyKind::Closure(_, substs) => matches!(
            substs.as_closure().kind(),
            ClosureKind::FnOnce | ClosureKind::FnMut
          ),
          _ => false,
        },
        _ => false,
      })
    });
    if any_closure_inputs {
      debug!("  Func has closure inputs");
      return false;
    }

    let recursive = BODY_STACK.with(|body_stack| {
      let body_stack = body_stack.borrow();
      body_stack.iter().any(|visited_id| *visited_id == body_id)
    });
    if recursive {
      debug!("  Func is a recursive call");
      return false;
    }

    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id.expect_local());
    let mut recurse_cache = self.recurse_cache.borrow_mut();
    let flow = recurse_cache.entry(body_id).or_insert_with(|| {
      info!("Recursing into {}", tcx.def_path_debug_str(*def_id));
      super::compute_flow(tcx, body_id, body_with_facts)
    });
    let body = &body_with_facts.body;

    let mut return_state = FlowDomain::new(flow.analysis.location_domain());
    {
      let return_locs = body
        .basic_blocks
        .iter_enumerated()
        .filter_map(|(bb, data)| match data.terminator().kind {
          TerminatorKind::Return => Some(body.terminator_loc(bb)),
          _ => None,
        });

      for loc in return_locs {
        return_state.join(flow.state_at(loc));
      }
    };

    let translate_child_to_parent = |child: Place<'tcx>,
                                     mutated: bool|
     -> Option<Place<'tcx>> {
      if child.local == RETURN_PLACE && child.projection.len() == 0 {
        if child.ty(body.local_decls(), tcx).ty.is_unit() {
          return None;
        }

        return Some(*destination);
      }

      if !child.is_arg(body) || (mutated && !child.is_indirect()) {
        return None;
      }

      // For example, say we're calling f(_5.0) and child = (*_1).1 where
      // .1 is private to parent. Then:
      //    parent_toplevel_arg = _5.0
      //    parent_arg_projected = (*_5.0).1
      //    parent_arg_accessible = (*_5.0)

      let parent_toplevel_arg = parent_arg_places
        .iter()
        .find(|(j, _)| child.local.as_usize() - 1 == *j)
        .map(|(_, place)| place)?;

      let mut projection = parent_toplevel_arg.projection.to_vec();
      let mut ty = parent_toplevel_arg.ty(self.body.local_decls(), tcx);
      let parent_param_env = tcx.param_env(self.def_id);
      log::debug!("Adding child {child:?} to parent {parent_toplevel_arg:?}");
      for elem in child.projection.iter() {
        ty = ty.projection_ty_core(
          tcx,
          parent_param_env,
          &elem,
          |_, field, _| ty.field_ty(tcx, field),
          |_, ty| ty,
        );
        let elem = match elem {
          ProjectionElem::Field(field, _) => ProjectionElem::Field(field, ty.ty),
          elem => elem,
        };
        projection.push(elem);
      }

      let parent_arg_projected = Place::make(parent_toplevel_arg.local, &projection, tcx);
      Some(parent_arg_projected)
    };

    for (child, _) in return_state.rows() {
      if let Some(parent) = translate_child_to_parent(child, true) {
        let was_return = child.local == RETURN_PLACE;
        // > 1 because arguments will always have their synthetic location in their dep set
        let was_mutated = return_state.row_set(child).len() > 1;
        if !was_mutated && !was_return {
          continue;
        }

        let child_deps = return_state.row_set(child);
        let parent_deps = return_state
          .rows()
          .filter(|(_, deps)| child_deps.is_superset(deps))
          .filter_map(|(row, _)| translate_child_to_parent(row, false))
          .collect::<Vec<_>>();

        debug!(
          "child {child:?} \n  / child_deps {child_deps:?}\n-->\nparent {parent:?}\n   / parent_deps {parent_deps:?}"
        );

        self.transfer_function(state, Mutation {
          mutated: parent,
          inputs: &parent_deps,
          location,
          status: if was_return {
            MutationStatus::Definitely
          } else {
            MutationStatus::Possibly
          },
        });
      }
    }

    true
  }
}
