//! Compute program dependence graphs (PDG) for a function call graph.

use self::graph::DepGraph;
use crate::pdg::construct::GraphConstructor;
pub use crate::pdg::construct::PdgParams;

mod construct;
pub mod graph;
mod utils;

/// Computes a global program dependence graph (PDG) starting from the root function specified by `def_id`.
pub fn compute_pdg<'tcx>(params: PdgParams<'tcx>) -> DepGraph<'tcx> {
  let constructor = GraphConstructor::root(params);
  constructor.construct()
}
