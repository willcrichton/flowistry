use super::aliases::{
  interior_pointers, place_relation, place_set_join, Aliases, PlaceRelation, PlaceSet,
};
use super::control_dependencies::ControlDependencies;
use crate::config::{Config, ContextMode, MutabilityMode};
use indexmap::map::Entry;
use log::debug;
use rustc_data_structures::fx::{
  FxHashMap as HashMap, FxHashSet as HashSet, FxIndexMap as IndexMap,
};
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
use std::{cell::RefCell, fmt};

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
pub struct RelevanceTrace<'tcx> {
  pub mutated: PlaceSet<'tcx>,
}

impl JoinSemiLattice for RelevanceTrace<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    place_set_join(&mut self.mutated, &other.mutated)
  }
}

type RelevantStatements<'tcx> = IndexMap<Location, RelevanceTrace<'tcx>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelevanceDomain<'tcx> {
  pub relevant_places: PlaceSet<'tcx>,
  pub relevant_statements: RelevantStatements<'tcx>,
}

fn relevant_statements_join(
  this: &mut RelevantStatements<'tcx>,
  other: &RelevantStatements<'tcx>,
) -> bool {
  let changes = other
    .iter()
    .map(|(loc, trace)| match this.entry(*loc) {
      Entry::Vacant(entry) => {
        entry.insert(trace.clone());
        true
      }
      Entry::Occupied(mut entry) => entry.get_mut().join(trace),
    })
    .collect::<Vec<_>>();
  changes.into_iter().any(|x| x)
}

impl JoinSemiLattice for RelevanceDomain<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    let places_changed = place_set_join(&mut self.relevant_places, &other.relevant_places);
    let statements_changed =
      relevant_statements_join(&mut self.relevant_statements, &other.relevant_statements);
    places_changed || statements_changed
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
      "{{{}}}",
      format_places(&self.relevant_places),
      // self.relevant_statements.keys().collect::<HashSet<_>>()
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
  fn add_relevant(
    &mut self,
    mutated: &Vec<(Place<'tcx>, MutationKind)>,
    used: &PlaceSet<'tcx>,
    location: Location,
  ) {
    let to_delete = mutated
      .iter()
      .filter_map(|(place, mutation)| match mutation {
        MutationKind::Strong => Some(*place),
        _ => None,
      })
      .collect::<HashSet<_>>();

    self.state.relevant_places = &self.state.relevant_places - &to_delete;
    self.state.relevant_places.extend(used.iter().cloned());

    let mutated_places = mutated
      .iter()
      .map(|(place, _)| *place)
      .collect::<HashSet<_>>();
    let mut new_statements = IndexMap::default();
    new_statements.insert(
      location,
      RelevanceTrace {
        mutated: mutated_places,
      },
    );
    relevant_statements_join(&mut self.state.relevant_statements, &new_statements);
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
      .relevant_places
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

        // TODO: is there a more precise check for strong updated than |mutated_places| == 1?
        // eg if *x mutated (*_2) and (_1) then that's a strong update on both, but only b/c
        // they're at different level of indirection.
        if relations
          .iter()
          .any(|relation| *relation == PlaceRelation::Sub)
        {
          let mutation_kind = if mutated_places.len() == 1 && definitely_mutated {
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

  pub(super) fn check_mutation(
    &mut self,
    place: Place<'tcx>,
    input_places: &PlaceSet<'tcx>,
    definitely_mutated: bool,
    location: Location,
  ) -> bool {
    debug!(
      "checking {:?} with relevant = {:?}",
      place, self.state.relevant_places,
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

      self.add_relevant(&vec![], &pointers, location);
      self.add_relevant(&relevant_mutated, input_places, location);
      debug!("  updated relevant: {:?}", self.state.relevant_places);

      true
    } else {
      false
    }
  }

  fn check_slice_set(&mut self, location: Location) {
    self.analysis.slice_set.get(&location).map(|places| {
      self.add_relevant(&vec![], places, location);
    });
  }
}

fn operand_to_place(operand: &Operand<'tcx>) -> Option<Place<'tcx>> {
  match operand {
    Operand::Copy(place) | Operand::Move(place) => Some(*place),
    Operand::Constant(_) => None,
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let mut collector = CollectPlaces {
      places: HashSet::default(),
    };
    collector.visit_rvalue(rvalue, location);

    self.check_mutation(*place, &collector.places, true, location);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    self.super_terminator(terminator, location);

    debug!(
      "checking terminator {:?} in context {:?}",
      terminator.kind, self.state.relevant_places
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
          .filter_map(|(i, arg)| operand_to_place(arg).map(|place| (i, place)))
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

        let dst_relevant = destination.and_then(|(dst, _)| {
          // Special case: if a function returns unit (common with mutation-only functions),
          // then we're guaranteed that the function body has no effect on the return value.
          // This case mainly shows up in the evaluation when we auto-generate slices on all locals
          // that includes unit return values of functions.
          let not_unit = !dst.ty(self.analysis.body.local_decls(), tcx).ty.is_unit();
          (not_unit && self.is_relevant(dst)).then(|| dst)
        });

        // For performance (especially w/ Recurse), don't check function if both inputs and outputs
        // aren't relevant
        if input_mut_ptrs.iter().any(|(_, v)| v.len() > 0) || dst_relevant.is_some() {
          let eval_mode = self.analysis.config.eval_mode;
          let could_recurse = if eval_mode.context_mode == ContextMode::Recurse {
            self.slice_into_procedure(&terminator.kind, &input_places, &input_mut_ptrs, location)
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
                if self.check_mutation(ptr, &input_places, false, location) {
                  break;
                }
              }
            }

            if let Some(dst) = dst_relevant {
              self.check_mutation(dst, &input_places, true, location);
            }
          }
        }
      }

      TerminatorKind::SwitchInt { discr, .. } => {
        let is_relevant = self.state.relevant_statements.keys().any(|relevant| {
          self
            .analysis
            .control_dependencies
            .is_dependent(relevant.block, location.block)
        });

        if is_relevant {
          let mut input = HashSet::default();
          if let Some(place) = operand_to_place(discr) {
            input.insert(place);
          }
          self.add_relevant(&vec![], &input, location);
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(input_place) = operand_to_place(value) {
          let mut input = HashSet::default();
          input.insert(input_place);
          self.check_mutation(*place, &input, true, location);
        }
      }

      _ => {}
    }
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
  control_dependencies: ControlDependencies,
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
    control_dependencies: ControlDependencies,
  ) -> Self {
    let current_block = RefCell::new(body.basic_blocks().indices().next().unwrap());

    RelevanceAnalysis {
      config,
      slice_set,
      tcx,
      body,
      alias_analysis,
      control_dependencies,
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
      relevant_places: PlaceSet::default(),
      relevant_statements: IndexMap::default(),
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
