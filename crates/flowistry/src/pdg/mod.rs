#![allow(missing_docs)]
#![allow(warnings)]

use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use self::{construct::CallingContext, graph::DepGraph, utils::FnResolution};
use crate::pdg::construct::GraphConstructor;

mod construct;
pub mod graph;
mod utils;
mod value;

pub fn compute_pdg<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> DepGraph<'tcx> {
  let constructor = GraphConstructor::new(
    tcx,
    FnResolution::Partial(def_id.to_def_id()),
    CallingContext::empty(),
  );
  constructor.construct()
}
