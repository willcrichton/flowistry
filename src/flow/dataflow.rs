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

    // Union dependencies into all conflicting places of the mutated place
    for place in all_aliases.conflicts(mutated).indices() {
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

    let parent_arg_places = utils::arg_places(parent_args);

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
    let mut cursor = ResultsRefCursor::new(body, flow);

    let return_locs = body
      .basic_blocks()
      .iter_enumerated()
      .filter_map(|(bb, data)| match data.terminator().kind {
        TerminatorKind::Return => Some(body.terminator_loc(bb)),
        _ => None,
      });
    let combined_deps = return_locs
      .map(|loc| {
        cursor.seek_after_primary_effect(loc);
        cursor.get().clone()
      })
      .reduce(|mut a, b| {
        a.join(&b);
        a
      })
      .unwrap();

    let find_accessible_place = |place: Place<'tcx>, domain: &Rc<PlaceDomain<'tcx>>| {
      for i in (0..=place.projection.len()).rev() {
        let sub_place = utils::mk_place(place.local, &place.projection[..i], tcx);
        if domain.contains(&sub_place) {
          return sub_place;
        }
      }

      unreachable!("{:?}", place)
    };

    let get_parent_arg = |i| {
      parent_arg_places
        .iter()
        .find(|(j, _)| i == *j)
        .map(|(_, place)| place)
    };

    let parent_domain = self.analysis.place_domain();
    let child_domain = flow.analysis.place_domain();

    let relevant_to_place = |child_place| {
      let child_place_accessible_to_child = find_accessible_place(child_place, child_domain);
      trace!(
        "child_place {:?} accessible to child at {:?}",
        child_place,
        child_place_accessible_to_child
      );

      combined_deps
        .places
        .row(child_place_accessible_to_child)
        .filter_map(|child_place_dep| {
          if !utils::is_arg(*child_place_dep, body) {
            return None;
          }

          let parent_arg = get_parent_arg(child_place_dep.local.as_usize() - 1)?;
          let mut projection = parent_arg.projection.to_vec();
          projection.extend_from_slice(child_place_dep.projection);
          let parent_arg_projected = utils::mk_place(parent_arg.local, &projection, tcx);
          let accessible_to_parent = find_accessible_place(parent_arg_projected, parent_domain);
          trace!(
            "parent_arg_projected {:?} accessible to parent at {:?}",
            parent_arg_projected,
            accessible_to_parent
          );
          Some(accessible_to_parent)
        })
        .collect::<Vec<_>>()
    };

    for (arg_index, mut_ptr) in utils::arg_mut_ptrs(
      &parent_arg_places,
      tcx,
      self.analysis.body,
      self.analysis.def_id,
    ) {
      let projection = &mut_ptr.projection[get_parent_arg(arg_index).unwrap().projection.len()..];
      let arg_place = utils::mk_place(Local::from_usize(arg_index + 1), projection, tcx);
      let was_modified = combined_deps
        .locations
        .row(find_accessible_place(arg_place, child_domain))
        .next()
        .is_some();

      if was_modified {
        let relevant_args = relevant_to_place(arg_place);
        self.apply_mutation(mut_ptr, &relevant_args, location, false);
      }
    }

    if let Some((dst, _)) = destination {
      let inputs = relevant_to_place(utils::local_to_place(RETURN_PLACE, tcx));
      self.apply_mutation(*dst, &inputs, location, true);
    }

    true
  }
}

impl Visitor<'tcx> for TransferFunction<'a, 'b, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    self.apply_mutation(*place, &collector.places, location, true);
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

          self.apply_mutation(*dst_place, inputs, location, true);
        }

        for (_, mut_ptr) in
          utils::arg_mut_ptrs(&arg_places, tcx, self.analysis.body, self.analysis.def_id)
        {
          self.apply_mutation(mut_ptr, &arg_inputs, location, false);
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(src) = utils::operand_to_place(value) {
          self.apply_mutation(*place, &[src], location, true);
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
