use super::{
  aliases::Aliases,
  borrow_ranges::BorrowRanges,
  place_index::PlaceSet,
  place_index::{PlaceIndex, PlaceIndices},
  points_to::{NonlocalDecls, PlacePrim, PointsToAnalysis, PointsToDomain},
};
use log::debug;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{
    self,
    borrows::BorrowSet,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::{Const, TyCtxt},
};
use rustc_mir::{
  borrow_check::{borrow_conflicts_with_place, AccessDepth, PlaceConflictBias},
  dataflow::{
    fmt::{DebugWithAdapter, DebugWithContext},
    impls::Borrows,
    Analysis, AnalysisDomain, Backward, JoinSemiLattice, Results, ResultsRefCursor,
  },
};
use rustc_span::DUMMY_SP;
use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
  fmt,
};

pub type SliceSet = HashSet<Location>;

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
pub struct RelevanceDomain {
  pub places: PlaceSet,
  pub statement_relevant: bool,
  pub path_relevant: Relevant,
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let places_changed = self.places.join(&other.places);
    let path_relevant_changed = self.path_relevant.join(&other.path_relevant);
    places_changed || path_relevant_changed
  }
}

impl DebugWithContext<RelevanceAnalysis<'_, '_, '_>> for RelevanceDomain {
  fn fmt_with(
    &self,
    ctxt: &RelevanceAnalysis<'_, '_, '_>,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    self.places.fmt_with(ctxt.place_indices, f)?;
    write!(
      f,
      " {:?}, {:?}",
      self.statement_relevant, self.path_relevant
    )
  }
}

struct CollectPlaceIndices<'a, 'tcx> {
  places: PlaceSet,
  place_indices: &'a PlaceIndices<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for CollectPlaceIndices<'a, 'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.places.insert(self.place_indices.index(place));
  }
}

struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  analysis: &'a RelevanceAnalysis<'b, 'mir, 'tcx>,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'b, 'mir, 'tcx> TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn add_relevant(&mut self, place: Place<'tcx>) {
    self
      .state
      .places
      .insert(self.analysis.place_indices.index(&place));
    self.state.statement_relevant = true;
    self.state.path_relevant = Relevant::Yes;
  }

  fn add_relevant_many(&mut self, places: &PlaceSet) {
    self.state.places.union(places);
    self.state.statement_relevant = true;
    self.state.path_relevant = Relevant::Yes;
  }
}

impl<'a, 'b, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
    self.state.statement_relevant = false;
    match &statement.kind {
      StatementKind::Assign(_) => {
        self.super_statement(statement, location);
      }
      _ => {}
    }
  }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    macro_rules! fmt_places {
      ($places:expr) => {
        DebugWithAdapter {
          this: &$places,
          ctxt: self.analysis.place_indices,
        }
      };
    }

    debug!("checking {:?} = {:?}", place, rvalue);
    let (possibly_mutated, pointers_to_mutated) = self.analysis.places_and_pointers(*place);
    debug!(
      "  relevant {:?}, possibly_mutated {:?}, pointers_to_mutated {:?}",
      fmt_places!(self.state.places),
      fmt_places!(possibly_mutated),
      fmt_places!(pointers_to_mutated)
    );

    let any_relevant_mutated = possibly_mutated.iter().any(|mutated_place| {
      self.state.places.iter().any(|relevant_place| {
        self
          .analysis
          .place_index_is_part(mutated_place, relevant_place)
          || self
            .analysis
            .place_index_is_part(relevant_place, mutated_place)
      })
    });

    if any_relevant_mutated {
      // strong update
      if possibly_mutated.count() == 1 {
        debug!("  deleting {:?}", fmt_places!(possibly_mutated));
        let definitely_mutated = possibly_mutated.iter().next().unwrap();
        let to_delete = self
          .state
          .places
          .iter()
          .filter(|relevant_place| {
            self
              .analysis
              .place_index_is_part(*relevant_place, definitely_mutated)
          })
          .collect::<Vec<_>>();
        for i in to_delete {
          self.state.places.remove(i);
        }
        debug!("  after deletion: {:?}", fmt_places!(self.state.places));
      }

      let mut collector = CollectPlaceIndices {
        places: self.analysis.place_indices.empty_set(),
        place_indices: self.analysis.place_indices,
      };
      collector.visit_rvalue(rvalue, location);

      debug!(
        "  adding relevant places {:?} and pointers to possibly mutated {:?}",
        fmt_places!(collector.places),
        fmt_places!(pointers_to_mutated)
      );
      self.add_relevant_many(&collector.places);
      self.add_relevant_many(&pointers_to_mutated);
    }

    // let pointer_analysis = self.analysis.pointer_analysis.borrow();
    // let pointer_analysis = pointer_analysis.get();

    // let (possibly_mutated, pointers_to_possibly_mutated) =
    //   pointer_analysis.possible_prims_and_pointers(*place);

    // debug!(
    //   "checking assign {:?} = {:?} in context {:?} (possibly mutated {:?})",
    //   place, rvalue, self.state.places, possibly_mutated
    // );
    // if self.any_relevant_mutated(&possibly_mutated) {
    //   debug!("  relevant assignment to {:?}", place);

    //   // TODO: better checking for strong udpates
    //   if possibly_mutated.len() == 1 {
    //     let definitely_mutated = possibly_mutated.iter().next().unwrap();

    //     // if mutating x.0 and x is relevant, this should return false
    //     // since x.0 is not in the relevant set, but it is a relevant mutation
    //     self.state.places.remove(definitely_mutated);

    //     debug!(
    //       "  deleting {:?}, remaining {:?}",
    //       definitely_mutated, self.state.places
    //     );
    //   }

    //   let mut collector = CollectPlaces {
    //     places: HashSet::new(),
    //     pointer_analysis,
    //   };
    //   collector.visit_rvalue(rvalue, location);

    //   debug!(
    //     "  collected from rvalue {:?}, got places {:?} and pointers {:?}",
    //     rvalue, collector.places, pointers_to_possibly_mutated
    //   );

    //   self.add_relevant(&collector.places);
    //   self.add_relevant(&pointers_to_possibly_mutated);

    //   debug!("  new places: {:?}", self.state.places);
    // }
  }

  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&location) {
      self.add_relevant(*place);
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    // let pointer_analysis = self.analysis.pointer_analysis.borrow();
    // let pointer_analysis = pointer_analysis.get();
    // self.state.statement_relevant = false;

    // debug!(
    //   "checking terminator {:?} in context {:?}",
    //   terminator.kind, self.state.places
    // );

    // match &terminator.kind {
    //   TerminatorKind::SwitchInt { discr, .. } => {
    //     if self.state.path_relevant == Relevant::Yes {
    //       match discr {
    //         Operand::Move(place) | Operand::Copy(place) => {
    //           let relevant_to_control = pointer_analysis.possible_prims(*place);
    //           self.add_relevant(&relevant_to_control);
    //         }
    //         Operand::Constant(_) => {}
    //       };
    //     }
    //   }

    //   TerminatorKind::Call {
    //     args, destination, ..
    //   } => {
    //     let any_relevant_mutable_inputs = args.iter().any(|arg| match arg {
    //       Operand::Move(place) | Operand::Copy(place) => {
    //         let input_prims = pointer_analysis.possible_prims(*place);
    //         input_prims.iter().any(|input_prim| {
    //           self.state.places.iter().any(|relevant| {
    //             debug!(
    //               "  comparing relevant {:?} and mutated {:?}",
    //               relevant, input_prim
    //             );
    //             self.analysis.points_to_prim(input_prim, relevant)
    //               || self.analysis.points_to_prim(relevant, input_prim)
    //           })
    //         })
    //       }
    //       Operand::Constant(_) => false,
    //     });

    //     if any_relevant_mutable_inputs {
    //       let relevant_inputs = args
    //         .iter()
    //         .filter_map(|arg| match arg {
    //           Operand::Move(place) | Operand::Copy(place) => {
    //             let prim = PlacePrim::local(place.as_local().expect(&format!("{:?}", place)));
    //             Some(prim)
    //           }
    //           Operand::Constant(_) => None,
    //         })
    //         .collect::<HashSet<_>>();
    //       self.add_relevant(&relevant_inputs);
    //     }

    //     if let Some((dst_place, _)) = destination {
    //       let possibly_mutated = pointer_analysis.possible_prims(*dst_place);
    //       if self.any_relevant_mutated(&possibly_mutated) {
    //         let input_places: HashSet<_> = args
    //           .iter()
    //           .map(|arg| match arg {
    //             Operand::Move(place) | Operand::Copy(place) => {
    //               pointer_analysis.possible_prims(*place).clone()
    //             }
    //             Operand::Constant(_) => HashSet::new(),
    //           })
    //           .flatten()
    //           .collect();

    //         self.add_relevant(&input_places);
    //       }
    //     }
    //   }
    //   _ => {}
    // };

    // self.state.path_relevant = if self.state.statement_relevant {
    //   Relevant::Yes
    // } else {
    //   Relevant::No
    // };
  }
}

pub struct RelevanceAnalysis<'a, 'mir, 'tcx> {
  slice_set: SliceSet,
  pointer_analysis: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, PointsToAnalysis<'mir, 'tcx>>>,
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  module: DefId,
  nonlocal_decls: NonlocalDecls<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  borrow_ranges: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, BorrowRanges<'mir, 'tcx>>>,
  place_indices: &'a PlaceIndices<'tcx>,
  aliases: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, Aliases<'a, 'mir, 'tcx>>>,
}

impl<'a, 'mir, 'tcx> RelevanceAnalysis<'a, 'mir, 'tcx> {
  pub fn new(
    slice_set: SliceSet,
    tcx: TyCtxt<'tcx>,
    module: DefId,
    body: &'mir Body<'tcx>,
    results: &'a Results<'tcx, PointsToAnalysis<'mir, 'tcx>>,
    nonlocal_decls: NonlocalDecls<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    borrow_ranges: &'a Results<'tcx, BorrowRanges<'mir, 'tcx>>,
    place_indices: &'a PlaceIndices<'tcx>,
    aliases: &'a Results<'tcx, Aliases<'a, 'mir, 'tcx>>,
  ) -> Self {
    let pointer_analysis = RefCell::new(ResultsRefCursor::new(body, &results));
    let borrow_ranges = RefCell::new(ResultsRefCursor::new(body, &borrow_ranges));
    let aliases = RefCell::new(ResultsRefCursor::new(body, aliases));
    RelevanceAnalysis {
      slice_set,
      pointer_analysis,
      tcx,
      body,
      module,
      nonlocal_decls,
      borrow_set,
      borrow_ranges,
      place_indices,
      aliases,
    }
  }

  fn place_index_is_part(&self, part_place: PlaceIndex, whole_place: PlaceIndex) -> bool {
    self.place_is_part(
      self.place_indices.lookup(part_place),
      self.place_indices.lookup(whole_place),
    )
  }

  fn place_is_part(&self, part_place: Place<'tcx>, whole_place: Place<'tcx>) -> bool {
    borrow_conflicts_with_place(
      self.tcx,
      self.body,
      whole_place,
      BorrowKind::Mut {
        allow_two_phase_borrow: true,
      },
      part_place.as_ref(),
      AccessDepth::Deep,
      PlaceConflictBias::Overlap,
    )
  }

  fn places_and_pointers(&self, place: Place<'tcx>) -> (PlaceSet, PlaceSet) {
    let borrow_ranges = self.borrow_ranges.borrow();
    let borrow_ranges = borrow_ranges.get();

    let aliases = self.aliases.borrow();
    let aliases = aliases.get();

    let mut places = self.place_indices.empty_set();
    let mut pointers = self.place_indices.empty_set();
    places.insert(self.place_indices.index(&place));

    for i in borrow_ranges.iter() {
      let borrow = &self.borrow_set[i];

      let mut borrow_aliases = aliases.iter_enumerated().filter_map(|(local, borrows)| {
        if borrows.contains(i) {
          Some(local)
        } else {
          None
        }
      });

      let part_of_alias = borrow_aliases.any(|alias| {
        self.place_is_part(place, Place {
          local: alias, 
          projection: self.tcx.intern_place_elems(&[])
        })
      });

      if self.place_is_part(place, borrow.assigned_place) || part_of_alias {
        // println!(
        //   "  {:?} conflicts with {:?}, adding {:?}",
        //   place, borrow.assigned_place, borrow.borrowed_place
        // );
        places.insert(self.place_indices.index(&borrow.borrowed_place));
        pointers.insert(self.place_indices.index(&borrow.assigned_place));

        let (sub_places, sub_pointers) = self.places_and_pointers(borrow.borrowed_place);
        places.union(&sub_places);
        pointers.union(&sub_pointers);
      }
    }

    (places, pointers)
  }
}

impl<'a, 'mir, 'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis<'a, 'mir, 'tcx> {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    RelevanceDomain {
      places: self.place_indices.empty_set(),
      statement_relevant: false,
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
    self
      .pointer_analysis
      .borrow_mut()
      .seek_before_primary_effect(location);
    self
      .borrow_ranges
      .borrow_mut()
      .seek_before_primary_effect(location);
    self
      .aliases
      .borrow_mut()
      .seek_before_primary_effect(location);

    TransferFunction {
      state,
      analysis: self,
    }
    .visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &mir::Terminator<'tcx>,
    location: Location,
  ) {
    self
      .pointer_analysis
      .borrow_mut()
      .seek_before_primary_effect(location);
    self
      .borrow_ranges
      .borrow_mut()
      .seek_before_primary_effect(location);
    self
      .aliases
      .borrow_mut()
      .seek_before_primary_effect(location);

    TransferFunction {
      state,
      analysis: self,
    }
    .visit_terminator(terminator, location);
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
