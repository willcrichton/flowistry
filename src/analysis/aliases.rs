use super::borrow_ranges::BorrowRanges;
use log::{debug, warn};
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    borrows::{BorrowIndex, BorrowSet},
    regions::{Locations, OutlivesConstraint},
    visit::Visitor,
    *,
  },
  ty::RegionVid,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, Analysis, AnalysisDomain, Results, ResultsRefCursor,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

pub type AliasesDomain = IndexVec<Local, BitSet<BorrowIndex>>;

struct TransferFunction<'a, 'b, 'mir, 'tcx> {
  analysis: &'a Aliases<'b, 'mir, 'tcx>,
  state: &'a mut AliasesDomain,
}

impl<'a, 'b, 'mir, 'tcx> TransferFunction<'a, 'b, 'mir, 'tcx> {
  fn process(&mut self, place: Place<'tcx>, location: Location) {
    let borrow_ranges = self.analysis.borrow_ranges.borrow();
    let borrow_ranges = borrow_ranges.get();

    let constraints = self
      .analysis
      .outlives_constraints
      .iter()
      .filter(|constraint| {
        if let Locations::Single(constraint_location) = constraint.locations {
          constraint_location == location
        } else {
          false
        }
      });

    debug!("checking {:?}", place);
    for constraint in constraints {
      debug!("  against constraint {:?}", constraint);
      let borrow = borrow_ranges.iter().find(|borrow_idx| {
        let borrow = &self.analysis.borrow_set[*borrow_idx];
        borrow.region == constraint.sup
      });

      match borrow {
        Some(borrow_idx) => {
          debug!("    found borrow {:?}", borrow_idx);
          self.state[place.local].insert(borrow_idx);
        }
        None => {
          let local = &self.analysis.region_to_local.get(&constraint.sup);

          if let Some(local) = local {
            let borrows = self.state[**local].clone();
            debug!(
              "    found transitive borrows {:?} from local {:?}",
              borrows, local
            );
            self.state[place.local].union(&borrows);
          } else {
            warn!(
              "no region for local {:?} from constraint {:?} in context {:?} and {:?}",
              constraint.sup,
              constraint,
              self.analysis.region_to_local,
              self.analysis.outlives_constraints
            );
          }
        }
      }
    }
  }
}

impl<'tcx> Visitor<'tcx> for TransferFunction<'_, '_, '_, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, _rvalue: &Rvalue<'tcx>, location: Location) {
    self.process(*place, location);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    match &terminator.kind {
      TerminatorKind::Call { destination, .. } => {
        if let Some((place, _)) = destination {
          self.process(*place, location);
        }
      }
      _ => {}
    }
  }
}

pub struct Aliases<'a, 'mir, 'tcx> {
  borrow_set: &'a BorrowSet<'tcx>,
  borrow_ranges: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, BorrowRanges<'mir, 'tcx>>>,
  outlives_constraints: &'a Vec<OutlivesConstraint>,
  region_to_local: HashMap<RegionVid, Local>,
}

impl<'a, 'mir, 'tcx> Aliases<'a, 'mir, 'tcx> {
  pub fn new(
    body: &'mir Body<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    borrow_ranges: &'a Results<'tcx, BorrowRanges<'mir, 'tcx>>,
    outlives_constraints: &'a Vec<OutlivesConstraint>,
  ) -> Self {
    let borrow_ranges = RefCell::new(ResultsRefCursor::new(body, borrow_ranges));

    let region_to_local = outlives_constraints
      .iter()
      .filter_map(|constraint| {
        if let Locations::Single(location) = constraint.locations {
          let bb = &body.basic_blocks()[location.block];
          if location.statement_index == bb.statements.len() {
            match &bb.terminator.as_ref().unwrap().kind {
              TerminatorKind::Call { destination, .. } => {
                if let Some((place, _)) = destination {
                  Some((constraint.sub, place.local))
                } else {
                  None
                }
              }
              _ => None,
            }
          } else {
            let statement = &bb.statements[location.statement_index];
            if let StatementKind::Assign(assign) = &statement.kind {
              let place = assign.0;
              Some((constraint.sub, place.local))
            } else {
              unimplemented!("{:?}", statement)
            }
          }
        } else {
          // TODO
          None
        }
      })
      .collect();

    Aliases {
      borrow_set,
      borrow_ranges,
      outlives_constraints,
      region_to_local,
    }
  }
}

impl<'tcx> AnalysisDomain<'tcx> for Aliases<'_, '_, 'tcx> {
  type Domain = AliasesDomain;
  const NAME: &'static str = "Aliases";

  fn bottom_value(&self, body: &Body<'tcx>) -> Self::Domain {
    IndexVec::from_elem_n(
      BitSet::new_empty(self.borrow_set.len()),
      body.local_decls().len(),
    )
  }

  fn initialize_start_block(&self, _: &Body<'tcx>, _: &mut Self::Domain) {}
}

impl<'tcx> Analysis<'tcx> for Aliases<'_, '_, 'tcx> {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &Statement<'tcx>,
    location: Location,
  ) {
    self
      .borrow_ranges
      .borrow_mut()
      .seek_after_primary_effect(location);

    TransferFunction {
      state,
      analysis: self,
    }
    .visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &Terminator<'tcx>,
    location: Location,
  ) {
    self
      .borrow_ranges
      .borrow_mut()
      .seek_after_primary_effect(location);

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
    _func: &Operand<'tcx>,
    _args: &[Operand<'tcx>],
    _return_place: Place<'tcx>,
  ) {
  }
}

impl DebugWithContext<Aliases<'_, '_, '_>> for AliasesDomain {
  fn fmt_with(&self, _ctxt: &Aliases<'_, '_, '_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (local, borrows) in self.iter_enumerated() {
      if borrows.count() > 0 {
        write!(f, "{:?}: {:?}, ", local, borrows)?;
      }
    }
    Ok(())
  }
}
