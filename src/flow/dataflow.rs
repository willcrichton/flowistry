use log::{debug, info, trace};
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::{subst::GenericArgKind, ClosureKind, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Forward, JoinSemiLattice, ResultsRefCursor,
};
use std::{fmt, iter, rc::Rc};

use super::BODY_STACK;
use crate::core::{
  aliases::Aliases,
  analysis,
  control_dependencies::ControlDependencies,
  extensions::{is_extension_active, ContextMode, REACHED_LIBRARY},
  indexed::{IndexMatrix, IndexedDomain},
  indexed_impls::{build_location_domain, LocationDomain, LocationSet, PlaceDomain, PlaceSet},
  utils::{self, PlaceCollector},
};

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct FlowDomain<'tcx> {
  pub locations: IndexMatrix<Place<'tcx>, Location>,
  pub places: IndexMatrix<Place<'tcx>, Place<'tcx>>,
}

impl FlowDomain<'tcx> {
  pub fn new(place_domain: Rc<PlaceDomain<'tcx>>, location_domain: Rc<LocationDomain>) -> Self {
    FlowDomain {
      locations: IndexMatrix::new(place_domain.clone(), location_domain),
      places: IndexMatrix::new(place_domain.clone(), place_domain),
    }
  }
}

impl JoinSemiLattice for FlowDomain<'_> {
  fn join(&mut self, other: &Self) -> bool {
    let a = self.locations.join(&other.locations);
    let b = self.places.join(&other.places);
    a || b
  }
}

impl<C> DebugWithContext<C> for FlowDomain<'_> {
  fn fmt_with(&self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.locations.fmt_with(ctxt, f)?;
    self.places.fmt_with(ctxt, f)
  }

  fn fmt_diff_with(&self, old: &Self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.locations.fmt_diff_with(&old.locations, ctxt, f)?;
    self.places.fmt_diff_with(&old.places, ctxt, f)
  }
}

struct TransferFunction<'a, 'b, 'tcx> {
  analysis: &'a FlowAnalysis<'b, 'tcx>,
  state: &'a mut FlowDomain<'tcx>,
}

impl TransferFunction<'_, '_, 'tcx> {
  fn apply_mutation(
    &mut self,
    mutated: Place<'tcx>,
    inputs: &[Place<'tcx>],
    location: Location,
    definitely_mutated: bool,
    mutate_aliases_only: bool,
  ) {
    debug!(
      "Applying mutation to {:?} with inputs {:?}",
      mutated, inputs
    );
    let place_domain = self.analysis.place_domain();
    let location_domain = self.analysis.location_domain();

    let all_aliases = &self.analysis.aliases;
    let mutated_aliases = all_aliases.aliases.row_set(mutated).unwrap();

    // Clear sub-places of mutated place (if sound to do so)
    if definitely_mutated && mutated_aliases.len() == 1 {
      let mutated_direct = mutated_aliases.iter().next().unwrap();
      for sub in all_aliases.subs.row(*mutated_direct) {
        self.state.places.clear_row(sub);
        self.state.locations.clear_row(sub);
      }
    }

    let mut input_location_deps = LocationSet::new(location_domain.clone());
    input_location_deps.insert(location);

    let mut input_place_deps = PlaceSet::new(place_domain.clone());

    let add_deps =
      |place: Place<'tcx>, location_deps: &mut LocationSet, place_deps: &mut PlaceSet<'tcx>| {
        for dep_place in self.analysis.aliases.deps.row(place) {
          if let Some(deps) = self.state.locations.row_set(dep_place) {
            location_deps.union(&deps);
          }

          place_deps.insert(dep_place);
          if let Some(deps) = self.state.places.row_set(dep_place) {
            trace!(
              "  Adding {:?} / dependency {:?} with deps {:?}",
              place,
              dep_place,
              deps
            );
            place_deps.union(&deps);
          }
        }
      };

    // Add deps of mutated to include provenance of mutated pointers
    add_deps(mutated, &mut input_location_deps, &mut input_place_deps);

    // Add deps of all inputs
    for place in inputs.iter() {
      add_deps(*place, &mut input_location_deps, &mut input_place_deps);
    }

    // Add control dependencies
    let controlled_by = self
      .analysis
      .control_dependencies
      .dependent_on(location.block);
    let body = self.analysis.body;
    for block in controlled_by.into_iter().map(|set| set.iter()).flatten() {
      input_location_deps.insert(body.terminator_loc(block));

      let terminator = body.basic_blocks()[block].terminator();
      if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind {
        if let Some(discr_place) = utils::operand_to_place(discr) {
          add_deps(discr_place, &mut input_location_deps, &mut input_place_deps);
        }
      }
    }

    let conflicts = if mutate_aliases_only {
      all_aliases.aliases.row_indices(mutated).collect::<Vec<_>>()
    } else {
      all_aliases.conflicts(mutated).indices().collect::<Vec<_>>()
    };

    // Union dependencies into all conflicting places of the mutated place
    for place in conflicts {
      self
        .state
        .locations
        .union_into_row(place, &input_location_deps);
      self.state.places.union_into_row(place, &input_place_deps);
    }
  }

  fn recurse_into_call(&mut self, call: &TerminatorKind<'tcx>, location: Location) -> bool {
    let tcx = self.analysis.tcx;
    let (func, parent_args, destination) = match call {
      TerminatorKind::Call {
        func,
        args,
        destination,
        ..
      } => (func, args, destination),
      _ => unreachable!(),
    };
    debug!("Checking whether can recurse into {:?}", func);

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
      let ty = place.ty(self.analysis.body.local_decls(), tcx).ty;
      ty.walk(tcx).any(|arg| match arg.unpack() {
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
    let body_with_facts = analysis::get_body_with_borrowck_facts(tcx, def_id.expect_local());
    let flow = &super::compute_flow(tcx, body_id, body_with_facts);
    let body = &body_with_facts.body;

    let mut return_state = FlowDomain::new(
      flow.analysis.place_domain().clone(),
      flow.analysis.location_domain().clone(),
    );
    {
      let return_locs = body
        .basic_blocks()
        .iter_enumerated()
        .filter_map(|(bb, data)| match data.terminator().kind {
          TerminatorKind::Return => Some(body.terminator_loc(bb)),
          _ => None,
        });

      let mut cursor = ResultsRefCursor::new(body, flow);
      for loc in return_locs {
        cursor.seek_after_primary_effect(loc);
        return_state.join(cursor.get());
      }
    };

    let parent_domain = self.analysis.place_domain();
    let child_domain = flow.analysis.place_domain();

    let translate_child_to_parent = |child: Place<'tcx>| -> Option<Place<'tcx>> {
      if child.local == RETURN_PLACE && child.projection.len() == 0 {
        if let Some((dst, _)) = destination {
          return Some(*dst);
        }
      }

      if !utils::is_arg(child, body) {
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
      let parent_arg_projected = utils::mk_place(parent_toplevel_arg.local, &projection, tcx);

      let parent_arg_accessible = {
        let mut sub_places = (0..=parent_arg_projected.projection.len()).rev().map(|i| {
          utils::mk_place(
            parent_arg_projected.local,
            &parent_arg_projected.projection[..i],
            tcx,
          )
        });

        sub_places
          .find(|sub_place| parent_domain.contains(sub_place))
          .unwrap()
      };

      Some(parent_arg_accessible)
    };

    for child in child_domain.as_vec().iter() {
      if let Some(parent) = translate_child_to_parent(*child) {
        let was_return = child.local == RETURN_PLACE;
        let was_mutated = return_state.locations.row(child).next().is_some();
        if !was_mutated && !was_return {
          continue;
        }

        let child_deps = return_state.places.row(child).copied();
        let parent_deps = child_deps
          .filter_map(translate_child_to_parent)
          .collect::<Vec<_>>();

        debug!(
          "child {:?} / child_deps {:?} --> parent {:?} / parent_deps {:?}",
          child,
          return_state.places.row_set(child),
          parent,
          parent_deps
        );

        self.apply_mutation(parent, &parent_deps, location, was_return, true);
      }
    }

    true
  }
}

impl Visitor<'tcx> for TransferFunction<'a, 'b, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    self.apply_mutation(*place, &collector.places, location, true, false);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    let tcx = self.analysis.tcx;

    match &terminator.kind {
      TerminatorKind::Call {
        /*func,*/ // TODO: deal with func
        args,
        destination,
        ..
      } => {
        if is_extension_active(|mode| mode.context_mode == ContextMode::Recurse)
          && self.recurse_into_call(&terminator.kind, location)
        {
          return;
        }

        let arg_places = utils::arg_places(args);
        let arg_inputs = arg_places
          .iter()
          .map(|(_, place)| {
            utils::interior_pointers(*place, tcx, self.analysis.body, self.analysis.def_id)
              .into_values()
              .map(|places| {
                places
                  .into_iter()
                  .map(|(place, _)| tcx.mk_place_deref(place))
              })
              .flatten()
              .chain(iter::once(*place))
          })
          .flatten()
          .collect::<Vec<_>>();

        if let Some((dst_place, _)) = destination {
          let ret_is_unit = dst_place
            .ty(self.analysis.body.local_decls(), tcx)
            .ty
            .is_unit();
          let empty = vec![];
          let inputs = if ret_is_unit { &empty } else { &arg_inputs };

          self.apply_mutation(*dst_place, inputs, location, true, false);
        }

        for (_, mut_ptr) in
          utils::arg_mut_ptrs(&arg_places, tcx, self.analysis.body, self.analysis.def_id)
        {
          self.apply_mutation(mut_ptr, &arg_inputs, location, false, false);
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(src) = utils::operand_to_place(value) {
          self.apply_mutation(*place, &[src], location, true, false);
        }
      }

      _ => {}
    }
  }
}

pub struct FlowAnalysis<'a, 'tcx> {
  pub tcx: TyCtxt<'tcx>,
  pub def_id: DefId,
  pub body: &'a Body<'tcx>,
  pub control_dependencies: ControlDependencies,
  pub aliases: Aliases<'tcx>,
  pub location_domain: Rc<LocationDomain>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'a Body<'tcx>,
    aliases: Aliases<'tcx>,
    control_dependencies: ControlDependencies,
  ) -> Self {
    let location_domain = build_location_domain(body);

    FlowAnalysis {
      tcx,
      def_id,
      body,
      aliases,
      location_domain,
      control_dependencies,
    }
  }

  pub fn place_domain(&self) -> &Rc<PlaceDomain<'tcx>> {
    &self.aliases.place_domain
  }

  pub fn location_domain(&self) -> &Rc<LocationDomain> {
    &self.location_domain
  }
}

impl AnalysisDomain<'tcx> for FlowAnalysis<'a, 'tcx> {
  type Domain = FlowDomain<'tcx>;
  type Direction = Forward;
  const NAME: &'static str = "FlowAnalysis";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    FlowDomain::new(self.place_domain().clone(), self.location_domain().clone())
  }

  fn initialize_start_block(&self, _body: &Body<'tcx>, _state: &mut Self::Domain) {}
}

impl Analysis<'tcx> for FlowAnalysis<'a, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &Terminator<'tcx>,
    location: Location,
  ) {
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_terminator(terminator, location);
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _func: &Operand<'tcx>,
    _args: &[Operand<'tcx>],
    _return_place: Place<'tcx>,
  ) {
  }
}
