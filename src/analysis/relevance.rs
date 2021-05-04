use super::aliases::{interior_pointers, place_relation, Aliases, PlaceRelation, PlaceSet};
use crate::config::{Config, ContextMode, MutabilityMode};
use log::debug;
use rustc_data_structures::graph::dominators::Dominators;
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Backward, JoinSemiLattice,
};
use rustc_span::Span;
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
  fmt,
};

pub type SliceSet<'tcx> = HashMap<Location, PlaceSet<'tcx>>;

// Previous strategy of representing path relevance as a bool didn't seem to work out
// with out dataflow framework handles start/exit states and join? Adding a third unknown
// state as bottom rather than defaulting to false seemed to work
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Relevant {
  Yes,
  No,
  Unknown,
}

impl JoinSemiLattice for Relevant {
  fn join(&mut self, other: &Self) -> bool {
    let state = match (*self, *other) {
      (Relevant::Yes, _) | (_, Relevant::Yes) => Relevant::Yes,
      (Relevant::No, _) | (_, Relevant::No) => Relevant::No,
      _ => Relevant::Unknown,
    };
    if state != *self {
      *self = state;
      true
    } else {
      false
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelevanceTrace<'tcx> {
  NotRelevant,
  Relevant { mutated: PlaceSet<'tcx> },
}

impl RelevanceTrace<'tcx> {
  pub fn reset(&mut self) {
    *self = RelevanceTrace::NotRelevant;
  }

  pub fn merge(&mut self, mutated: PlaceSet<'tcx>) {
    match self {
      RelevanceTrace::NotRelevant => {
        *self = RelevanceTrace::Relevant { mutated };
      }
      RelevanceTrace::Relevant {
        mutated: orig_mutated,
      } => {
        orig_mutated.extend(mutated.into_iter());
      }
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelevanceDomain<'tcx> {
  pub places: PlaceSet<'tcx>,
  pub path_relevant: Relevant,
  pub statement_relevant: RelevanceTrace<'tcx>,
}

impl JoinSemiLattice for RelevanceDomain<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    let places_changed = {
      if other.places.is_subset(&self.places) {
        false
      } else {
        self.places = &self.places | &other.places;
        true
      }
    };
    let path_relevant_changed = self.path_relevant.join(&other.path_relevant);
    places_changed || path_relevant_changed
  }
}

impl<C> DebugWithContext<C> for RelevanceDomain<'tcx> {
  fn fmt_with(&self, _ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let format_place = |place: Place| {
      let mut s = format!("{:?}", place.local);
      for elem in place.projection.iter() {
        s = match elem {
          ProjectionElem::Deref => format!("(*{})", s),
          ProjectionElem::Field(field, _) => format!("{}.{:?}", s, field.as_usize()),
          ProjectionElem::Index(_) => format!("{}[]", s),
          _ => format!("TODO({})", s),
        };
      }
      s
    };

    let format_places = |places: &PlaceSet| {
      places
        .iter()
        .map(|place| format_place(*place))
        .collect::<Vec<_>>()
        .join(", ")
    };

    write!(
      f,
      "{}, {}, {:?}",
      format_places(&self.places),
      match self.statement_relevant {
        RelevanceTrace::NotRelevant => "NotRelevant".to_string(),
        RelevanceTrace::Relevant { .. } => "Relevant".to_string(),
      },
      self.path_relevant
    )
  }
}

struct CollectPlaces<'tcx> {
  places: PlaceSet<'tcx>,
}

impl Visitor<'tcx> for CollectPlaces<'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.places.insert(*place);
  }
}

pub(super) struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  pub(super) analysis: &'a RelevanceAnalysis<'b, 'mir, 'tcx>,
  pub(super) state: &'a mut RelevanceDomain<'tcx>,
}

#[derive(Debug)]
pub enum MutationKind {
  Strong,
  Weak,
}

impl<'a, 'b, 'mir, 'tcx> TransferFunction<'a, 'b, 'mir, 'tcx> {
  pub(super) fn add_relevant(
    &mut self,
    mutated: &Vec<(Place<'tcx>, MutationKind)>,
    used: &PlaceSet<'tcx>,
  ) {
    let to_delete = mutated
      .iter()
      .filter_map(|(place, mutation)| match mutation {
        MutationKind::Strong => Some(*place),
        _ => None,
      })
      .collect::<HashSet<_>>();

    self.state.places = &self.state.places - &to_delete;

    self.state.places.extend(used.iter().cloned());
    self
      .state
      .statement_relevant
      .merge(mutated.iter().map(|(place, _)| *place).collect());

    let current_block = self.analysis.current_block.borrow();
    let preds = &self.analysis.body.predecessors()[*current_block];
    let dominates_all_preds = preds.iter().all(|pred_bb| {
      self.analysis.post_dominators.is_reachable(*pred_bb)
        && self
          .analysis
          .post_dominators
          .is_dominated_by(*pred_bb, *current_block)
    });
    if !dominates_all_preds {
      self.state.path_relevant = Relevant::Yes;
    }
  }

  pub(super) fn relevant_places(
    &self,
    mutated_place: Place<'tcx>,
    definitely_mutated: bool,
  ) -> Vec<(Place<'tcx>, MutationKind)> {
    let mutated_places = self.analysis.alias_analysis.loans(mutated_place);
    debug!("  mutated {:?} / {:?}", mutated_place, mutated_places);

    self
      .state
      .places
      .iter()
      .filter_map(|relevant_place| {
        let relations = mutated_places
          .iter()
          .filter_map(
            |mutated_place| match place_relation(*relevant_place, *mutated_place) {
              PlaceRelation::Disjoint => None,
              relation => Some(relation),
            },
          )
          .collect::<Vec<_>>();

        if relations
          .iter()
          .any(|relation| *relation == PlaceRelation::Sub)
        {
          let mutation_kind = if definitely_mutated {
            MutationKind::Strong
          } else {
            MutationKind::Weak
          };
          Some((*relevant_place, mutation_kind))
        } else if relations
          .iter()
          .any(|relation| *relation == PlaceRelation::Super)
        {
          Some((*relevant_place, MutationKind::Weak))
        } else {
          None
        }
      })
      .collect::<Vec<_>>()
  }

  pub fn is_relevant(&mut self, place: Place<'tcx>) -> bool {
    self.relevant_places(place, false).len() > 0
  }

  fn check_mutation(
    &mut self,
    place: Place<'tcx>,
    input_places: &PlaceSet<'tcx>,
    definitely_mutated: bool,
  ) -> bool {
    // is `place` in the relevant set?
    debug!(
      "checking {:?} with relevant = {:?}",
      place, self.state.places,
    );
    let relevant_mutated = self.relevant_places(place, definitely_mutated);
    debug!("  relevant mutated = {:?}", relevant_mutated);

    if relevant_mutated.len() > 0 {
      let pointers = place
        .iter_projections()
        .filter_map(|(place_ref, projection_elem)| {
          if let ProjectionElem::Deref = projection_elem {
            let place = Place {
              local: place_ref.local,
              projection: self.analysis.tcx.intern_place_elems(place_ref.projection),
            };
            Some(place)
          } else {
            None
          }
        })
        .collect::<HashSet<_>>();

      self.add_relevant(&vec![], &pointers);
      self.add_relevant(&relevant_mutated, input_places);
      debug!("  updated relevant: {:?}", self.state.places);

      true
    } else {
      false
    }
  }

  fn check_slice_set(&mut self, location: Location) {
    self.analysis.slice_set.get(&location).map(|places| {
      self.add_relevant(&vec![], places);
    });
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    self.state.statement_relevant.reset();
    match &statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {}
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let mut collector = CollectPlaces {
      places: HashSet::new(),
    };
    collector.visit_rvalue(rvalue, location);

    self.check_mutation(*place, &collector.places, true);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    self.super_terminator(terminator, location);

    // Ignore FalseEdge nodes since they can trip up the soundness of path_relevance wrt the post-dominator tree.
    // eg a FalseEdge node always post-dominates a while-loop condition so it would set path_relevant to false,
    // but while-body state gets propagated through the FalseEdge node which cause the while-condition to be incorrectly
    // marked as irrelevant when it is relevant to the while-body
    match &terminator.kind {
      TerminatorKind::FalseEdge { .. } => {
        return;
      }
      _ => {}
    }

    self.state.statement_relevant.reset();

    debug!(
      "checking terminator {:?} in context {:?}",
      terminator.kind, self.state.places
    );

    let tcx = self.analysis.tcx;
    let eval_mode = self.analysis.config.eval_mode;

    match &terminator.kind {
      TerminatorKind::Call {
        args, destination, ..
      } => {
        let input_places = args
          .iter()
          .enumerate()
          .filter_map(|(i, arg)| match arg {
            Operand::Move(place) | Operand::Copy(place) => Some((i, *place)),
            Operand::Constant(_) => None,
          })
          .collect::<Vec<_>>();

        let input_mut_ptrs = input_places
          .iter()
          .map(|(i, place)| {
            let ptr_places = interior_pointers(*place, tcx, self.analysis.body)
              .into_iter()
              .filter_map(|(_, (place, mutability))| match mutability {
                Mutability::Mut => Some(place),
                Mutability::Not => {
                  (eval_mode.mutability_mode == MutabilityMode::IgnoreMut).then(|| place)
                }
              })
              .map(|ptr_place| tcx.mk_place_deref(ptr_place))
              .filter(|deref_place| self.is_relevant(*deref_place))
              .collect::<HashSet<_>>();

            (*i, ptr_places)
          })
          .collect::<Vec<_>>();

        let eval_mode = self.analysis.config.eval_mode;
        let could_recurse = if eval_mode.context_mode == ContextMode::Recurse {
          self.slice_into_procedure(&terminator.kind, &input_places, &input_mut_ptrs)
        } else {
          false
        };

        if !could_recurse {
          let input_places = input_places
            .into_iter()
            .map(|(_, place)| place)
            .collect::<HashSet<_>>();

          for (_, ptrs) in input_mut_ptrs {
            for ptr in ptrs {
              if self.check_mutation(ptr, &input_places, false) {
                break;
              }
            }
          }

          if let Some((dst, _)) = destination {
            // Special case: if a function returns unit (common with mutation-only functions),
            // then we're guaranteed that the function body has no effect on the return value.
            // This case mainly shows up in the evaluation when we auto-generate slices on all locals
            // that includes unit return values of functions.
            if !dst.ty(self.analysis.body.local_decls(), tcx).ty.is_unit() {
              self.check_mutation(*dst, &input_places, true);
            }
          }
        }
      }

      TerminatorKind::SwitchInt { discr, .. } => {
        if self.state.path_relevant == Relevant::Yes {
          match discr {
            Operand::Move(place) | Operand::Copy(place) => {
              let mut relevant = HashSet::new();
              relevant.insert(*place);
              self.add_relevant(&vec![], &relevant);
            }
            Operand::Constant(_) => {}
          }
        }
      }

      _ => {}
    }

    self.state.path_relevant =
      if let RelevanceTrace::Relevant { .. } = self.state.statement_relevant {
        Relevant::Yes
      } else {
        Relevant::No
      };
  }
}

struct FindSpans {
  spans: Vec<Span>,
  relevant_locals: HashSet<Local>,
}

impl Visitor<'tcx> for FindSpans {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, _location: Location) {
    match statement.kind {
      StatementKind::Assign(box (place, _)) => {
        if self.relevant_locals.contains(&place.local) {
          self.spans.push(statement.source_info.span);
        }
      }
      _ => {}
    }
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub(super) config: &'a Config,
  slice_set: SliceSet<'tcx>,
  pub(super) tcx: TyCtxt<'tcx>,
  pub(super) body: &'mir Body<'tcx>,
  post_dominators: Dominators<BasicBlock>,
  current_block: RefCell<BasicBlock>,
  alias_analysis: &'a Aliases<'tcx>,
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub fn new(
    config: &'a Config,
    slice_set: SliceSet<'tcx>,
    tcx: TyCtxt<'tcx>,
    body: &'mir Body<'tcx>,
    alias_analysis: &'a Aliases<'tcx>,
    post_dominators: Dominators<BasicBlock>,
  ) -> Self {
    let current_block = RefCell::new(body.basic_blocks().indices().next().unwrap());

    RelevanceAnalysis {
      config,
      slice_set,
      tcx,
      body,
      alias_analysis,
      post_dominators,
      current_block,
    }
  }
}

impl<'a, 'mir, 'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  type Domain = RelevanceDomain<'tcx>;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    RelevanceDomain {
      places: PlaceSet::new(),
      statement_relevant: RelevanceTrace::NotRelevant,
      path_relevant: Relevant::Unknown,
    }
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {}
}

impl<'a, 'mir, 'tcx> Analysis<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &mir::Statement<'tcx>,
    location: Location,
  ) {
    *self.current_block.borrow_mut() = location.block;
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_statement(statement, location);
    tf.check_slice_set(location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &mir::Terminator<'tcx>,
    location: Location,
  ) {
    *self.current_block.borrow_mut() = location.block;
    let mut tf = TransferFunction {
      state,
      analysis: self,
    };
    tf.visit_terminator(terminator, location);
    tf.check_slice_set(location);
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _func: &mir::Operand<'tcx>,
    _args: &[mir::Operand<'tcx>],
    _return_place: mir::Place<'tcx>,
  ) {
  }
}
