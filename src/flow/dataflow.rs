use log::{debug, trace};
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::{subst::GenericArgKind, ClosureKind, TyCtxt, TyKind},
};
use rustc_mir_dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Forward, JoinSemiLattice, ResultsRefCursor,
};
use std::{fmt, rc::Rc};

use super::BODY_STACK;
use crate::core::{
  aliases::Aliases,
  control_dependencies::ControlDependencies,
  extensions::{is_extension_active, ContextMode},
  indexed::{IndexMatrix, IndexedDomain},
  indexed_impls::{
    build_location_domain, LocationDomain, LocationSet, PlaceDomain, PlaceIndex, PlaceSet,
  },
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
    is_borrow: bool,
  ) {
    debug!(
      "Applying mutation to {:?} with inputs {:?}",
      mutated, inputs
    );
    let tcx = self.analysis.tcx;
    let place_domain = self.analysis.place_domain();
    let location_domain = self.analysis.location_domain();

    let mut input_location_deps = LocationSet::new(location_domain.clone());
    input_location_deps.insert(location);

    let mut input_place_deps = PlaceSet::new(place_domain.clone());

    let add_deps =
      |place, input_location_deps: &mut LocationSet, input_place_deps: &mut PlaceSet<'tcx>| {
        if let Some(loc_deps) = self.state.locations.row_set(place) {
          input_location_deps.union(&loc_deps);
        }

        input_place_deps.insert(place);
        if let Some(place_deps) = self.state.places.row_set(place) {
          trace!(
            "  Adding {:?} with deps {:?}",
            place_domain.value(place),
            place_deps
          );
          input_place_deps.union(&place_deps);
        }
      };

    let opt_ref = move |place: Place<'tcx>| -> Option<PlaceIndex> {
      utils::split_deref(place, tcx).map(|(ptr, _)| place_domain.index(&ptr))
    };

    let all_input_places = inputs
      .iter()
      .map(|place| {
        let aliases = self.analysis.aliases.aliases(*place);
        aliases
          .iter()
          .map(|alias| {
            vec![place_domain.index(alias)]
              .into_iter()
              .chain(opt_ref(*alias).into_iter())
          })
          .flatten()
          .collect::<Vec<_>>()
          .into_iter()
      })
      .flatten();

    for place in all_input_places {
      add_deps(place, &mut input_location_deps, &mut input_place_deps);
    }

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
          add_deps(
            place_domain.index(&discr_place),
            &mut input_location_deps,
            &mut input_place_deps,
          );
        }
      }
    }

    if let Some(ptr) = opt_ref(mutated) {
      // Only need to include provenance for references defined within the function,
      //   not from arguments.
      // TODO: this was primarily for recurse_project_dst,
      //   is this a special case of a more general optimization?
      let local = place_domain.value(ptr).local.as_usize();
      let is_arg = local > 0 && local - 1 < self.analysis.body.arg_count;
      if !is_arg {
        add_deps(ptr, &mut input_location_deps, &mut input_place_deps);
      }
    }

    let conflicts = self.analysis.aliases.conflicts(mutated);

    if definitely_mutated && conflicts.single_pointee {
      for sub in conflicts.subs.indices() {
        self.state.places.clear_row(sub);
        self.state.locations.clear_row(sub);
      }
    }

    for place in conflicts.iter() {
      self
        .state
        .locations
        .union_into_row(place, &input_location_deps);
      self.state.places.union_into_row(place, &input_place_deps);
    }

    // see pointer_reborrow_nested for why this matters
    if is_borrow {
      let deref_place = tcx.mk_place_deref(mutated);
      self
        .state
        .locations
        .union_into_row(deref_place, &input_location_deps);
      self
        .state
        .places
        .union_into_row(deref_place, &input_place_deps);
    }
  }

  fn recurse_into_call(&mut self, call: &TerminatorKind<'tcx>, location: Location) -> bool {
    let tcx = self.analysis.tcx;
    let (func, args, _destination) = match call {
      TerminatorKind::Call {
        func,
        args,
        destination,
        ..
      } => (func, args, destination),
      _ => unreachable!(),
    };

    let arg_places = utils::arg_places(args);

    let func = match func.constant() {
      Some(func) => func,
      None => {
        return false;
      }
    };

    let def_id = match func.literal.ty().kind() {
      TyKind::FnDef(def_id, _) => def_id,
      _ => {
        return false;
      }
    };

    let node = match tcx.hir().get_if_local(*def_id) {
      Some(node) => node,
      None => {
        // REACHED_LIBRARY.get(|reached_library| {
        //   if let Some(reached_library) = reached_library {
        //     *reached_library.borrow_mut() = true;
        //   }
        // });
        return false;
      }
    };

    let body_id = match node.body_id() {
      Some(body_id) => body_id,
      None => {
        return false;
      }
    };

    let any_closure_inputs = arg_places.iter().any(|place| {
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
      return false;
    }

    let recursive = BODY_STACK.with(|body_stack| {
      let body_stack = body_stack.borrow();
      body_stack.iter().any(|visited_id| *visited_id == body_id)
    });
    if recursive {
      return false;
    }

    let body_with_facts = utils::get_body_with_borrowck_facts(tcx, body_id);
    let flow = super::compute_flow(tcx, body_id, &body_with_facts);
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

    for (arg_index, mut_ptr) in
      utils::arg_mut_ptrs(&arg_places, tcx, self.analysis.body, self.analysis.def_id)
    {
      let projection = &mut_ptr.projection[arg_places[arg_index].projection.len()..];
      let arg_place = Place {
        local: Local::from_usize(arg_index + 1),
        projection: tcx.intern_place_elems(projection),
      };

      let relevant_args = combined_deps
        .places
        .row(arg_place)
        .filter_map(|place| {
          let idx = place.local.as_usize();
          (idx > 0 && idx - 1 < args.len()).then(|| {
            let arg = arg_places[idx - 1];
            let mut projection = arg.projection.to_vec();
            projection.extend(place.projection.iter());
            Place {
              local: arg.local,
              projection: tcx.intern_place_elems(&projection),
            }
          })
        })
        .collect::<Vec<_>>();

      if !relevant_args.is_empty() {
        self.apply_mutation(mut_ptr, &relevant_args, location, true, false);
      }
    }

    true
  }
}

impl Visitor<'tcx> for TransferFunction<'a, 'b, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);

    let is_borrow = matches!(rvalue, Rvalue::Ref(..));
    self.apply_mutation(*place, &collector.places, location, true, is_borrow);
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

        if let Some((dst_place, _)) = destination {
          self.apply_mutation(*dst_place, &arg_places, location, true, false);
        }

        for (_, mut_ptr) in
          utils::arg_mut_ptrs(&arg_places, tcx, self.analysis.body, self.analysis.def_id)
        {
          self.apply_mutation(mut_ptr, &arg_places, location, false, false);
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
  pub aliases: Aliases<'a, 'tcx>,
  pub location_domain: Rc<LocationDomain>,
}

impl FlowAnalysis<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    body: &'a Body<'tcx>,
    aliases: Aliases<'a, 'tcx>,
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
