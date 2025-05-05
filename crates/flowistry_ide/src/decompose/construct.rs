#![allow(warnings)]

use either::Either;
use flowistry::infoflow::{mutation::ModularMutationVisitor, FlowResults};
use indexical::{bitset::rustc::RustcBitSet, pointer::RcFamily};
use petgraph::{algo, graph::DiGraph};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_middle::{
  mir::{traversal, visit::Visitor, Body, Location, StatementKind, TerminatorKind},
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;
use rustc_utils::{mir::location_or_arg::LocationOrArg, BodyExt, PlaceExt};

use super::algo::GraphExt;

pub type IndexMatrix<'tcx, R, C> =
  indexical::IndexMatrix<'tcx, R, C, RustcBitSet, RcFamily>;

pub fn compute_adjacency_matrix<'a, 'tcx>(
  results: &'a FlowResults<'a, 'tcx>,
) -> IndexMatrix<'tcx, LocationOrArg, LocationOrArg> {
  let analysis = &results.analysis;
  let location_domain = analysis.location_domain();
  let mut adj_mtx: indexical::IndexMatrix<'tcx, _, _, _, _> =
    IndexMatrix::new(location_domain);

  ModularMutationVisitor::new(&results.analysis.place_info, |location, inputs| {
    let state = results.state_at(location);
    for mutation in inputs {
      adj_mtx.union_into_row(
        LocationOrArg::Location(location),
        &state.row_set(&mutation.mutated),
      );
    }
  })
  .visit_body(results.analysis.body);

  adj_mtx
}

type LocGraph = DiGraph<Vec<LocationOrArg>, ()>;

fn compute_graph<'tcx>(
  adj_mtx: IndexMatrix<'tcx, LocationOrArg, LocationOrArg>,
  results: &FlowResults,
) -> LocGraph {
  let mut g = DiGraph::<LocationOrArg, ()>::default();
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

pub fn build<'a, 'tcx>(
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  results: &'a FlowResults<'a, 'tcx>,
) -> LocGraph {
  let adj_mtx = compute_adjacency_matrix(results);
  compute_graph(adj_mtx, results)
}
