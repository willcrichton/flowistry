use super::aliases::{interior_pointers, AliasVisitor};
use super::relevance::TransferFunction;
use super::intraprocedural::BODY_STACK;
use log::{ info, };
use rustc_data_structures::graph::scc::Sccs;
use rustc_middle::{
  mir::{
    regions::{ConstraintSccIndex, Locations, OutlivesConstraint},
    visit::Visitor,
    *,
  },
  ty::{RegionVid, TyCtxt, TyKind, TyS},
};
use rustc_mir::borrow_check::constraints::OutlivesConstraintSet;
use std::collections::{HashSet};


impl AliasVisitor<'_, '_, 'tcx> {
  pub(super) fn handle_synthetic_aliases(&mut self, region: RegionVid, sub_place: Place<'tcx>) {
    for (input_region, input_place) in &self.input_regions {
      let input_scc = self.constraint_sccs.scc(*input_region);
      let is_alias = self
        .region_ancestors
        .get(&region)
        .map(|ancestors| ancestors.contains(&input_scc))
        .unwrap_or(false);
      if is_alias {
        let sub_place_idx = self.place_indices.insert(&sub_place);
        let alias_set = self
          .aliases
          .synthetic_aliases
          .entry(sub_place_idx)
          .or_insert_with(HashSet::new);
        let input_place_deref = self.place_indices.insert(input_place);
        alias_set.insert(input_place_deref);
      }
    }
  }
}

struct FindConstraints<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  constraints: Vec<OutlivesConstraint>,
}

impl FindConstraints<'_, 'tcx> {
  pub fn add_alias(&mut self, place1: Place<'tcx>, place2: Place<'tcx>, location: Location) {
    let place1_pointers = interior_pointers(place1, self.tcx, self.body);
    let place2_pointers = interior_pointers(place2, self.tcx, self.body);

    let mk_constraint = |r1, r2| OutlivesConstraint {
      sup: r1,
      sub: r2,
      locations: Locations::Single(location),
      category: ConstraintCategory::Internal,
    };

    let tcx = self.tcx;
    let body = self.body;

    let deref_ty = move |sub_place| {
      let sub_place_deref = tcx.mk_place_deref(sub_place);
      sub_place_deref.ty(body.local_decls(), tcx).ty
    };

    let constraints = place1_pointers
      .iter()
      .map(|(region1, sub_place1)| {
        place2_pointers
          .iter()
          .filter_map(move |(region2, sub_place2)| {
            if TyS::same_type(deref_ty(*sub_place1), deref_ty(*sub_place2)) {
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

impl TransferFunction<'_, '_, '_, 'tcx> {
  pub(super) fn slice_into_procedure(
    &mut self,
    call: &TerminatorKind<'tcx>,
    input_places: &[Place<'tcx>],
  ) -> bool {
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

    let def_id = if let TyKind::FnDef(def_id, _) = func.literal.ty.kind() {
      def_id
    } else {
      return false;
    };

    let node = if let Some(node) = self.analysis.tcx.hir().get_if_local(*def_id) {
      node
    } else {
      return false;
    };

    let body_id = if let Some(body_id) = node.body_id() {
      body_id
    } else {
      return false;
    };

    let recursive = BODY_STACK.with(|body_stack| {
      body_stack.borrow().iter().any(|visited_id| *visited_id == body_id)
    });
    if recursive {
      return false;
    }

    let relevant_inputs = input_places.iter().enumerate().filter_map(|(i, arg)| {
      if self.relevant_places(*arg).count() > 0 {
        Some(Local::from_usize(1 + i))
      } else {
        None
      }
    });

    let relevant_return = if let Some((dst, _)) = destination {
      if self.relevant_places(*dst).count() > 0 {
        vec![RETURN_PLACE]
      } else {
        vec![]
      }
    } else {
      vec![]
    };

    let relevant_locals = relevant_inputs
      .chain(relevant_return.into_iter())
      .collect::<HashSet<_>>();

    info!(
      "Recursing into {}",
      self.analysis.tcx.def_path_debug_str(*def_id)
    );
    let (_, mutated_locals) = super::intraprocedural::analyze_function(
      self.analysis.config,
      self.analysis.tcx,
      body_id,
      None,
      relevant_locals.iter().cloned().collect::<Vec<_>>(),
    )
    .unwrap();

    let relevant_and_mutated_locals = &relevant_locals & &mutated_locals;

    if relevant_and_mutated_locals.len() > 0 {
      let relevant_inputs = mutated_locals
        .iter()
        .filter_map(|local| {
          let i = local.as_usize();
          if 1 <= i && i <= input_places.len() {
            Some(input_places[i - 1])
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      self.add_relevant(&self.analysis.place_indices.vec_to_set(&relevant_inputs));
    }

    true
  }
}
