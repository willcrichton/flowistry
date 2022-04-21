#![allow(warnings)]

use either::Either;
use flowistry::{
  indexed::{impls::PlaceSet, IndexMatrix, IndexedDomain},
  infoflow::{mutation::ModularMutationVisitor, FlowResults},
  mir::utils::{BodyExt, PlaceCollector, PlaceExt},
};
use petgraph::{algo, graph::DiGraph};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_middle::{
  mir::{traversal, visit::Visitor, Body, Location, StatementKind, TerminatorKind},
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use super::algo::GraphExt;

pub fn compute_adjacency_matrix(
  results: &FlowResults,
) -> IndexMatrix<Location, Location> {
  let analysis = &results.analysis;
  let location_domain = analysis.location_domain();
  let mut adj_mtx = IndexMatrix::new(location_domain);

  ModularMutationVisitor::new(&results.analysis.aliases, |_, inputs, location, _| {
    let state = results.state_at(location);
    for (place, _) in inputs {
      adj_mtx.union_into_row(location, &state.row_set(*place));
    }
  })
  .visit_body(results.analysis.body);

  adj_mtx
}

type LocGraph = DiGraph<Vec<Location>, ()>;

fn compute_graph(
  adj_mtx: IndexMatrix<Location, Location>,
  results: &FlowResults,
) -> LocGraph {
  let mut g = DiGraph::<Location, ()>::default();
  let location_domain = results.analysis.location_domain();
  let loc_idx_to_pg_idx = location_domain
    .as_vec()
    .iter_enumerated()
    .map(|(loc_idx, loc)| (loc_idx, g.add_node(*loc)))
    .collect::<HashMap<_, _>>();

  for (src, dst) in adj_mtx.rows().flat_map(|(dst, srcs)| {
    srcs
      .indices()
      .map(move |src| (src, dst))
      .collect::<Vec<_>>()
  }) {
    g.add_edge(
      loc_idx_to_pg_idx[&src],
      loc_idx_to_pg_idx[&location_domain.index(&dst)],
      (),
    );
  }

  let to_remove = g
    .node_indices()
    .filter(|n| g.successors(*n).next().is_none() && g.predecessors(*n).next().is_none())
    .collect::<Vec<_>>();
  log::trace!(
    "Removing nodes: {:?}",
    to_remove
      .iter()
      .map(|n| g.node_weight(*n).unwrap())
      .collect::<Vec<_>>()
  );
  for i in to_remove.into_iter().rev() {
    g.remove_node(i);
  }

  let g = algo::condensation(g, true);
  // let g = super::algo::transitive_reduction(&g);

  g
}

pub fn build<'tcx>(
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  results: &FlowResults<'_, 'tcx>,
) -> LocGraph {
  let adj_mtx = compute_adjacency_matrix(results);
  compute_graph(adj_mtx, results)
}
