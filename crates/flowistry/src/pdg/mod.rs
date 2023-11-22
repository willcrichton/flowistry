#![allow(missing_docs)]

use log::debug;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_utils::BodyExt;

use self::graph::DepGraph;
use crate::pdg::construct::GraphConstructor;

mod construct;
pub mod graph;

pub fn compute_pdg<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> DepGraph<'tcx> {
  let body = &body_with_facts.body;
  debug!("{}", body.to_string(tcx).unwrap());

  let constructor = GraphConstructor::new(tcx, body_id, body_with_facts);
  constructor.construct()
}
