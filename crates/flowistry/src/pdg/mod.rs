#![allow(missing_docs)]

use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;

use self::{graph::DepGraph, value::ArgValues};
use crate::pdg::construct::GraphConstructor;

// mod cfa;
mod construct;
pub mod graph;
mod value;

pub fn compute_pdg<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> DepGraph<'tcx> {
  let constructor =
    GraphConstructor::new(tcx, body_id, body_with_facts, ArgValues::default());
  constructor.construct()
}
