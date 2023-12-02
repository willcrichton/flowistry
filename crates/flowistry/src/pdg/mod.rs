#![allow(missing_docs)]

use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use self::{construct::CallingContext, graph::DepGraph};
use crate::pdg::construct::GraphConstructor;

// mod cfa;
mod construct;
pub mod graph;
mod value;

pub fn compute_pdg<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> DepGraph<'tcx> {
  let constructor = GraphConstructor::new(tcx, def_id, CallingContext::empty());
  constructor.construct()
}
