use super::{relevance::TransferFunction, SliceOutput, BODY_STACK};
use crate::{
  core::{
    indexed::{IndexSetIteratorExt, IndexedDomain},
    indexed_impls::PlaceSet,
    utils,
  },
  fmt_places,
};
use fluid_let::fluid_let;
use log::{debug, info};
use rustc_data_structures::{fx::FxHashSet as HashSet, graph::scc::Sccs};
use rustc_middle::{
  mir::{
    regions::{ConstraintSccIndex, Locations, OutlivesConstraint},
    visit::Visitor,
    *,
  },
  ty::{subst::GenericArgKind, ClosureKind, RegionVid, TyCtxt, TyKind, TyS, VarianceDiagInfo},
};
use rustc_mir::borrow_check::constraints::OutlivesConstraintSet;
use std::cell::RefCell;

fluid_let!(pub static REACHED_LIBRARY: RefCell<bool>);

struct FindConstraints<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  constraints: Vec<OutlivesConstraint<'tcx>>,
}

impl FindConstraints<'_, 'tcx> {
  pub fn add_alias(&mut self, place1: Place<'tcx>, place2: Place<'tcx>, location: Location) {
    let place1_pointers = utils::interior_pointers(place1, self.tcx, self.body);
    let place2_pointers = utils::interior_pointers(place2, self.tcx, self.body);

    let mk_constraint = |r1, r2| OutlivesConstraint {
      sup: r1,
      sub: r2,
      locations: Locations::Single(location),
      category: ConstraintCategory::Internal,
      variance_info: VarianceDiagInfo::default(),
    };

    let tcx = self.tcx;
    let body = self.body;

    let deref_ty = move |sub_place| {
      let sub_place_deref = tcx.mk_place_deref(sub_place);
      sub_place_deref.ty(body.local_decls(), tcx).ty
    };

    let constraints = place1_pointers
      .iter()
      .map(|(region1, (sub_place1, _))| {
        place2_pointers
          .iter()
          .filter_map(move |(region2, (sub_place2, _))| {
            if TyS::same_type(deref_ty(*sub_place1), deref_ty(*sub_place2)) {
              debug!(
                "Adding alias {:?} = {:?} ({:?} = {:?})",
                sub_place1, sub_place2, region1, region2
              );
              Some((*region1, *region2))
            } else {
              None
            }
          })
      })
      .flatten()
      .map(|(r1, r2)| vec![mk_constraint(r1, r2), mk_constraint(r2, r1)].into_iter())
      .flatten();

    self.constraints.extend(constraints);
  }
}

impl Visitor<'tcx> for FindConstraints<'_, 'tcx> {
  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    match &terminator.kind {
      TerminatorKind::Call {
        args, destination, ..
      } => {
        let input_places = args
          .iter()
          .filter_map(|arg| match arg {
            Operand::Move(place) | Operand::Copy(place) => Some(*place),
            Operand::Constant(_) => None,
          })
          .collect::<Vec<_>>();

        for (i, input_place_i) in input_places.iter().enumerate() {
          for input_place_j in &input_places[(i + 1)..] {
            self.add_alias(*input_place_i, *input_place_j, location);
          }
        }

        if let Some((dst, _)) = destination {
          for input_place in input_places.iter() {
            self.add_alias(*dst, *input_place, location);
          }
        }
      }
      _ => {}
    }
  }
}

pub fn generate_conservative_constraints<'tcx>(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  outlives_constraints: &Vec<OutlivesConstraint>,
) -> Sccs<RegionVid, ConstraintSccIndex> {
  let max_region = outlives_constraints
    .iter()
    .map(|constraint| constraint.sup.as_usize().max(constraint.sub.as_usize()))
    .max()
    .unwrap_or(0)
    + 1;

  let mut finder = FindConstraints {
    tcx,
    body,
    constraints: Vec::new(),
  };
  finder.visit_body(body);

  let inputs = (0..body.arg_count)
    .map(|i| Place {
      local: Local::from_usize(i + 1),
      projection: tcx.intern_place_elems(&[]),
    })
    .collect::<Vec<_>>();
  for (i, input_place1) in inputs.iter().enumerate() {
    for input_place2 in &inputs[(i + 1)..] {
      finder.add_alias(*input_place1, *input_place2, Location::START);
    }
  }

  let mut constraint_set = OutlivesConstraintSet::default();
  for constraint in outlives_constraints.iter().chain(finder.constraints.iter()) {
    constraint_set.push(constraint.clone());
  }

  let constraint_graph = constraint_set.graph(max_region);
  constraint_set.compute_sccs(&constraint_graph, RegionVid::from_usize(0))
}

const MAX_DEPTH: usize = 3;

impl TransferFunction<'_, '_, '_, 'tcx> {
  #![allow(warnings)]
  pub(super) fn slice_into_procedure(
    &mut self,
    call: &TerminatorKind<'tcx>,
    input_places: &[(usize, Place<'tcx>)],
    input_mut_ptrs: &[(usize, PlaceSet<'tcx>)],
    location: Location,
  ) -> bool {
    let tcx = self.analysis.tcx;
    let place_domain = &self.analysis.alias_analysis.place_domain;

    let (func, destination) = if let TerminatorKind::Call {
      func, destination, ..
    } = call
    {
      (func, destination)
    } else {
      return false;
    };

    let func = if let Some(func) = func.constant() {
      func
    } else {
      return false;
    };

    let def_id = if let TyKind::FnDef(def_id, _) = func.literal.ty().kind() {
      def_id
    } else {
      return false;
    };

    let node = if let Some(node) = tcx.hir().get_if_local(*def_id) {
      node
    } else {
      REACHED_LIBRARY.get(|reached_library| {
        if let Some(reached_library) = reached_library {
          *reached_library.borrow_mut() = true;
        }
      });
      return false;
    };

    let body_id = if let Some(body_id) = node.body_id() {
      body_id
    } else {
      return false;
    };

    // Issue: if recursing into a function w/ closure that mutates environment,
    // then pointers in closure become opaque once recursing. No eays way to track
    // calls to the function as mutations to the environment.
    //
    // For now, ignore the issue by not analyzing functions with mutable closure inputs.
    let any_closure_inputs = input_places.iter().any(|(_, place)| {
      let ty = place.ty(self.analysis.body.local_decls(), tcx).ty;
      ty.walk().any(|arg| match arg.unpack() {
        GenericArgKind::Type(ty) => match ty.kind() {
          TyKind::Closure(_, substs) => match substs.as_closure().kind() {
            ClosureKind::FnOnce | ClosureKind::FnMut => true,
            _ => false,
          },
          _ => false,
        },
        _ => false,
      })
    });
    if any_closure_inputs {
      return false;
    }

    let (recursive, depth) = BODY_STACK.with(|body_stack| {
      let body_stack = body_stack.borrow();
      (
        body_stack.iter().any(|visited_id| *visited_id == body_id),
        body_stack.len(),
      )
    });
    if recursive || depth > MAX_DEPTH {
      return false;
    }

    let relevant_inputs = input_mut_ptrs
      .iter()
      .map(|(i, places)| {
        let (_, orig_input) = input_places.iter().find(|(j, _)| i == j).unwrap();
        places.iter().map(move |place| {
          let projection = &place.projection[orig_input.projection.len()..];
          Place {
            local: Local::from_usize(*i + 1),
            projection: tcx.intern_place_elems(projection),
          }
        })
      })
      .flatten();

    let relevant_return = if let Some((dst, _)) = destination {
      if !dst.ty(self.analysis.body.local_decls(), tcx).ty.is_unit()
        && self.is_relevant(place_domain.index(dst))
      {
        Some(Place {
          local: RETURN_PLACE,
          projection: tcx.intern_place_elems(&[]),
        })
      } else {
        None
      }
    } else {
      None
    };

    let relevant_places = relevant_inputs
      .chain(relevant_return.clone().into_iter())
      .collect::<HashSet<_>>();

    let def_path = tcx.def_path_debug_str(*def_id);
    info!(
      "Recursing into {} on places {:?}",
      def_path, relevant_places
    );
    let recursive_inputs = relevant_places.iter().cloned().collect::<Vec<_>>();
    let results: SliceOutput = todo!();
    // let results = super::slicing::analyze_function(
    //   self.analysis.config,
    //   tcx,
    //   body_id,
    //   &SliceLocation::PlacesOnExit(recursive_inputs.clone()),
    // )
    // .unwrap();

    debug!(
      "Done recursing into {}, mutated inputs: {:?}, relevant inputs: {:?}",
      def_path, results.mutated_inputs, results.relevant_inputs
    );

    let mutated_inputs = results
      .mutated_inputs
      .iter()
      .filter_map(|index| {
        let callee_place = recursive_inputs[*index];
        (callee_place.local != RETURN_PLACE).then(|| callee_place)
      })
      .map(|callee_place| {
        let i = callee_place.local.as_usize();
        let (_, caller_place) = input_places.iter().find(|(j, _)| *j == i - 1).unwrap();
        let mut projection = caller_place.projection.to_vec();
        projection.extend(callee_place.projection.iter());
        place_domain.index(&Place {
          local: caller_place.local,
          projection: tcx.intern_place_elems(&projection),
        })
      })
      .collect_indices::<Place<'tcx>>(place_domain.clone());

    let relevant_inputs = results
      .relevant_inputs
      .iter()
      .filter_map(|local| {
        input_places
          .iter()
          .find(|(j, _)| *j == local - 1)
          .map(|(_, caller_place)| place_domain.index(caller_place))
      })
      .collect_indices(place_domain.clone());

    if mutated_inputs.len() > 0 {
      debug!(
        "Adding mutated inputs {:?} from relevant inputs {:?}",
        fmt_places!(mutated_inputs, self.analysis),
        fmt_places!(relevant_inputs, self.analysis)
      );

      for place in mutated_inputs.indices() {
        self.check_mutation(place, &relevant_inputs, false, location);
      }
    }

    if relevant_return.is_some() {
      let (dst, _) = destination.unwrap();
      debug!("Adding relevant return: {:?}", dst);
      self.check_mutation(place_domain.index(&dst), &relevant_inputs, true, location);
    }

    true
  }
}
