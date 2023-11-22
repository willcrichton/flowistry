#![allow(missing_docs)]

use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;

use self::graph::DepGraph;
use crate::pdg::construct::GraphConstructor;

mod construct;
pub mod graph;

pub fn compute_pdg<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> DepGraph<'tcx> {
  let constructor = GraphConstructor::new(tcx, body_id, body_with_facts);
  constructor.construct()
}
