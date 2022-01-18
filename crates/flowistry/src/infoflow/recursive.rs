use log::{debug, info};
use rustc_middle::{
  mir::*,
  ty::{subst::GenericArgKind, ClosureKind, TyKind},
};
use rustc_mir_dataflow::JoinSemiLattice;

use super::{analysis::FlowAnalysis, BODY_STACK};
use crate::{
  extensions::REACHED_LIBRARY,
  indexed::IndexedDomain,
  infoflow::{mutation::MutationStatus, FlowDomain},
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{self, PlaceExt},
  },
};

impl FlowAnalysis<'_, 'tcx> {
  crate fn recurse_into_call(
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
    if !unsafety.unsafe_blocks.is_empty() {
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

    info!("Recursing into {}", tcx.def_path_debug_str(*def_id));
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id.expect_local());
    let mut recurse_cache = self.recurse_cache.borrow_mut();
    let flow = recurse_cache
      .entry(body_id)
      .or_insert_with(|| super::compute_flow(tcx, body_id, body_with_facts));
    let body = &body_with_facts.body;

    let mut return_state = FlowDomain::new(
      flow.analysis.place_domain(),
      flow.analysis.location_domain(),
    );
    {
      let return_locs = body
        .basic_blocks()
        .iter_enumerated()
        .filter_map(|(bb, data)| match data.terminator().kind {
          TerminatorKind::Return => Some(body.terminator_loc(bb)),
          _ => None,
        });

      for loc in return_locs {
        return_state.join(flow.state_at(loc));
      }
    };

    let parent_domain = self.place_domain();
    let parent_aliases = &self.aliases;
    let child_domain = flow.analysis.place_domain();

    let translate_child_to_parent = |child: Place<'tcx>,
                                     mutated: bool|
     -> Option<Place<'tcx>> {
      if child.local == RETURN_PLACE && child.projection.len() == 0 {
        if child.ty(body.local_decls(), tcx).ty.is_unit() {
          return None;
        }

        if let Some((dst, _)) = destination {
          return Some(*dst);
        }
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
      projection.extend_from_slice(child.projection);
      let parent_arg_projected = Place::make(parent_toplevel_arg.local, &projection, tcx);

      let parent_arg_accessible = {
        let mut sub_places =
          (0 ..= parent_arg_projected.projection.len())
            .rev()
            .map(|i| {
              Place::make(
                parent_arg_projected.local,
                &parent_arg_projected.projection[.. i],
                tcx,
              )
            });

        sub_places
          .find(|sub_place| {
            parent_domain.contains(sub_place)
              && parent_aliases.aliases.row(sub_place).next().is_some()
          })
          .unwrap()
      };

      Some(parent_arg_accessible)
    };

    for child in child_domain.as_vec().iter() {
      if let Some(parent) = translate_child_to_parent(*child, true) {
        let was_return = child.local == RETURN_PLACE;
        // > 1 because arguments will always have their synthetic location in their dep set
        let was_mutated = return_state
          .row_set(child)
          .map(|set| set.len() > 1)
          .unwrap_or(false);
        if !was_mutated && !was_return {
          continue;
        }

        let child_deps = return_state.row_set(child).unwrap();
        let parent_deps = return_state
          .rows()
          .filter(|(_, deps)| child_deps.is_superset(deps))
          .filter_map(|(row, _)| {
            Some((
              translate_child_to_parent(*child_domain.value(row), false)?,
              None,
            ))
          })
          .collect::<Vec<_>>();

        debug!(
          "child {child:?} \n  / child_deps {child_deps:?}\n-->\nparent {parent:?}\n   / parent_deps {parent_deps:?}"
        );

        self.transfer_function(
          state,
          parent,
          &parent_deps,
          location,
          if was_return {
            MutationStatus::Definitely
          } else {
            MutationStatus::Possibly
          },
        );
      }
    }

    true
  }
}
