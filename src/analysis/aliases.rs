use super::borrow_ranges::BorrowRanges;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    borrows::{BorrowIndex, BorrowSet},
    regions::{OutlivesConstraint},
    visit::Visitor,
    *,
  },
  ty::{subst::GenericArgKind, RegionKind, RegionVid, TyCtxt},
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
  fn process(&mut self, place: Place<'tcx>) {
    let borrow_ranges = self.analysis.borrow_ranges.borrow();
    let borrow_ranges = borrow_ranges.get();

    let ty = place
      .ty(self.analysis.body.local_decls(), self.analysis.tcx)
      .ty;
    let ty_regions = ty
      .walk()
      .filter_map(|ty| {
        if let GenericArgKind::Lifetime(RegionKind::ReVar(region)) = ty.unpack() {
          Some(*region)
        } else {
          None
        }
      })
      .collect::<HashSet<_>>();

    for region in ty_regions {
      let ty_borrows = borrow_ranges
        .iter()
        .filter(|idx| {
          let borrow = &self.analysis.borrow_set[*idx];
          let borrow_scc = self.analysis.constraint_sccs.scc(borrow.region);
          self
            .analysis
            .region_ancestors
            .get(&region)
            .map(|ancestors| ancestors.contains(&borrow_scc))
            .unwrap_or(false)
        })
        .collect::<Vec<_>>();

      for idx in ty_borrows {
        self.state[place.local].insert(idx);
      }
    }
  }
}

impl<'tcx> Visitor<'tcx> for TransferFunction<'_, '_, '_, 'tcx> {
  fn visit_assign(&mut self, place: &Place<'tcx>, _rvalue: &Rvalue<'tcx>, _location: Location) {
    self.process(*place);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    match &terminator.kind {
      TerminatorKind::Call { destination, .. } => {
        if let Some((place, _)) = destination {
          self.process(*place);
        }
      }
      _ => {}
    }
  }
}

pub struct Aliases<'a, 'mir, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'mir Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  borrow_ranges: RefCell<ResultsRefCursor<'a, 'mir, 'tcx, BorrowRanges<'mir, 'tcx>>>,
  region_ancestors: HashMap<RegionVid, HashSet<ConstraintSccIndex>>,
  constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>, // region_to_local: HashMap<RegionVid, Local>,
}

impl<'a, 'mir, 'tcx> Aliases<'a, 'mir, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body: &'mir Body<'tcx>,
    borrow_set: &'a BorrowSet<'tcx>,
    borrow_ranges: &'a Results<'tcx, BorrowRanges<'mir, 'tcx>>,
    outlives_constraints: &'a Vec<OutlivesConstraint>,
    constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>,
  ) -> Self {
    let borrow_ranges = RefCell::new(ResultsRefCursor::new(body, borrow_ranges));

    let max_region = outlives_constraints
      .iter()
      .map(|constraint| constraint.sup.as_usize().max(constraint.sub.as_usize()))
      .max()
      .unwrap_or(0)
      + 1;

    let root_region = RegionVid::from_usize(0);
    let root_scc = constraint_sccs.scc(root_region);
    let region_ancestors = compute_region_ancestors(constraint_sccs, max_region, root_scc);

    Aliases {
      tcx,
      body,
      borrow_set,
      borrow_ranges,
      region_ancestors,
      constraint_sccs, // region_to_local,
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

use rustc_data_structures::graph::scc::Sccs;
use rustc_middle::mir::regions::ConstraintSccIndex;
// use rustc_index::vec::IndexVec;
use std::collections::{hash_map::Entry, HashSet};
use std::hash::Hash;

fn merge<K: Eq + Hash, V>(
  mut h1: HashMap<K, V>,
  h2: HashMap<K, V>,
  conflict: impl Fn(&V, &V) -> V,
) -> HashMap<K, V> {
  for (k, v) in h2.into_iter() {
    match h1.entry(k) {
      Entry::Vacant(entry) => {
        entry.insert(v);
      }
      Entry::Occupied(mut entry) => {
        let entry = entry.get_mut();
        *entry = conflict(&*entry, &v);
      }
    }
  }
  h1
}

fn compute_region_ancestors(
  sccs: &Sccs<RegionVid, ConstraintSccIndex>,
  max_region: usize,
  node: ConstraintSccIndex,
) -> HashMap<RegionVid, HashSet<ConstraintSccIndex>> {
  let mut initial_hash = HashSet::new();
  initial_hash.insert(node);
  sccs
    .successors(node)
    .iter()
    .map(|child| {
      let in_child = (0..max_region)
        .map(|i| RegionVid::from_usize(i))
        .filter(|r| sccs.scc(*r) == *child)
        .map(|r| (r, initial_hash.clone()))
        .collect::<HashMap<_, _>>();
      let grandchildren = compute_region_ancestors(sccs, max_region, *child)
        .into_iter()
        .map(|(region, mut parents)| {
          parents.insert(node);
          (region, parents)
        })
        .collect::<HashMap<_, _>>();
      merge(in_child, grandchildren, |h1, h2| h1 | h2)
    })
    .fold(HashMap::new(), |h1, h2| merge(h1, h2, |h1, h2| h1 | h2))
}
