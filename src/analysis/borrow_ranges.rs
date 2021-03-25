use rustc_data_structures::fx::FxHashMap;
use rustc_index::bit_set::BitSet;
use rustc_middle::{
  mir::{
    borrows::{BorrowIndex, BorrowSet},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::{
  borrow_check::{borrow_conflicts_with_place, AccessDepth, PlaceConflictBias, PlaceExt},
  dataflow::{fmt::DebugWithContext, AnalysisDomain, GenKill, GenKillAnalysis},
};
use std::fmt;

pub struct BorrowRanges<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  borrows_out_of_scope_at_location: &'a FxHashMap<Location, Vec<BorrowIndex>>,
}

impl<'a, 'tcx> BorrowRanges<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    borrows_out_of_scope_at_location: &'a FxHashMap<Location, Vec<BorrowIndex>>,
  ) -> Self {
    BorrowRanges {
      tcx,
      body,
      borrow_set,
      borrows_out_of_scope_at_location,
    }
  }

  fn kill_loans_out_of_scope_at_location(
    &self,
    trans: &mut impl GenKill<BorrowIndex>,
    location: Location,
  ) {
    if let Some(indices) = self.borrows_out_of_scope_at_location.get(&location) {
      trans.kill_all(indices.iter().copied());
    }
  }

  /// Kill any borrows that conflict with `place`.
  fn kill_borrows_on_place(&self, trans: &mut impl GenKill<BorrowIndex>, place: Place<'tcx>) {
    let other_borrows_of_local = self
      .borrow_set
      .local_map
      .get(&place.local)
      .into_iter()
      .flat_map(|bs| bs.iter())
      .copied();

    // If the borrowed place is a local with no projections, all other borrows of this
    // local must conflict. This is purely an optimization so we don't have to call
    // `places_conflict` for every borrow.
    if place.projection.is_empty() {
      if !self.body.local_decls[place.local].is_ref_to_static() {
        trans.kill_all(other_borrows_of_local);
      }
      return;
    }

    // By passing `PlaceConflictBias::NoOverlap`, we conservatively assume that any given
    // pair of array indices are unequal, so that when `places_conflict` returns true, we
    // will be assured that two places being compared definitely denotes the same sets of
    // locations.
    let definitely_conflicting_borrows = other_borrows_of_local.filter(|&i| {
      borrow_conflicts_with_place(
        self.tcx,
        self.body,
        self.borrow_set[i].borrowed_place,
        BorrowKind::Mut {
          allow_two_phase_borrow: true,
        },
        place.as_ref(),
        AccessDepth::Deep,
        PlaceConflictBias::NoOverlap,
      )
    });

    trans.kill_all(definitely_conflicting_borrows);
  }
}

impl<'a, 'tcx> AnalysisDomain<'tcx> for BorrowRanges<'a, 'tcx> {
  type Domain = BitSet<BorrowIndex>;

  const NAME: &'static str = "BorrowRanges";

  fn bottom_value(&self, _: &Body<'tcx>) -> Self::Domain {
    // bottom = nothing is reserved or activated yet;
    BitSet::new_empty(self.borrow_set.len() * 2)
  }

  fn initialize_start_block(&self, _: &Body<'tcx>, _: &mut Self::Domain) {
    // no borrows of code region_scopes have been taken prior to
    // function execution, so this method has no effect.
  }
}

impl<'a, 'tcx> GenKillAnalysis<'tcx> for BorrowRanges<'a, 'tcx> {
  type Idx = BorrowIndex;

  fn before_statement_effect(
    &self,
    trans: &mut impl GenKill<Self::Idx>,
    _statement: &Statement<'tcx>,
    location: Location,
  ) {
    self.kill_loans_out_of_scope_at_location(trans, location);
  }

  fn statement_effect(
    &self,
    trans: &mut impl GenKill<Self::Idx>,
    stmt: &Statement<'tcx>,
    location: Location,
  ) {
    match &stmt.kind {
      StatementKind::Assign(assign) => {
        let (lhs, rhs) = &**assign;
        if let Rvalue::Ref(_, _, place) = *rhs {
          if place.ignore_borrow(self.tcx, self.body, &self.borrow_set.locals_state_at_exit) {
            return;
          }

          let index = self.borrow_set.get_index_of(&location).unwrap_or_else(|| {
            panic!("could not find BorrowIndex for location {:?}", location);
          });

          trans.gen(index);
        }

        // Make sure there are no remaining borrows for variables
        // that are assigned over.
        self.kill_borrows_on_place(trans, *lhs);
      }

      _ => {}
    }
  }

  fn before_terminator_effect(
    &self,
    trans: &mut impl GenKill<Self::Idx>,
    _terminator: &Terminator<'tcx>,
    location: Location,
  ) {
    self.kill_loans_out_of_scope_at_location(trans, location);
  }

  fn terminator_effect(
    &self,
    trans: &mut impl GenKill<Self::Idx>,
    teminator: &Terminator<'tcx>,
    _location: Location,
  ) {
    if let TerminatorKind::InlineAsm { operands, .. } = &teminator.kind {
      for op in operands {
        if let InlineAsmOperand::Out {
          place: Some(place), ..
        }
        | InlineAsmOperand::InOut {
          out_place: Some(place),
          ..
        } = *op
        {
          self.kill_borrows_on_place(trans, place);
        }
      }
    }
  }

  fn call_return_effect(
    &self,
    _trans: &mut impl GenKill<Self::Idx>,
    _block: BasicBlock,
    _func: &Operand<'tcx>,
    _args: &[Operand<'tcx>],
    _return_place: Place<'tcx>,
  ) {
  }
}

impl DebugWithContext<BorrowRanges<'_, '_>> for BorrowIndex {
  fn fmt_with(&self, ctxt: &BorrowRanges<'_, '_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", ctxt.borrow_set[*self].reserve_location)
  }
}
