//! Compute program dependence graphs (PDG) for a function call graph.

use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use self::graph::DepGraph;
use crate::pdg::construct::GraphConstructor;

mod construct;
pub mod graph;
mod utils;

/// Computes a global program dependence graph (PDG) starting from the root function specified by `def_id`.
pub fn compute_pdg<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> DepGraph<'tcx> {
  let constructor = GraphConstructor::root(tcx, def_id);
  constructor.construct()
}
