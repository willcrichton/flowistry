use std::path::Path;

use either::Either;
use flowistry::{
  indexed::{impls::PlaceSet, IndexMatrix, IndexedDomain},
  infoflow::FlowResults,
  mir::utils::{run_dot, BodyExt, PlaceCollector, PlaceExt},
};
use petgraph::{
  algo,
  dot::{Config as DotConfig, Dot},
  graph::DiGraph,
};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_middle::{
  mir::{traversal, visit::Visitor, Body, Location, StatementKind},
  ty::TyCtxt,
};

fn compute_adjacency_matrix(
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  results: &FlowResults<'_, 'tcx>,
) -> IndexMatrix<Location, Location> {
  let location_domain = results.analysis.location_domain();
  let place_domain = results.analysis.place_domain();

  let inputs_at = |location: Location| -> PlaceSet {
    let mut inputs = PlaceSet::new(place_domain);

    let mut visitor = PlaceCollector {
      tcx,
      places: Vec::new(),
    };
    match body.stmt_at(location) {
      Either::Left(stmt) => match &stmt.kind {
        StatementKind::Assign(box (_, rvalue)) => {
          visitor.visit_rvalue(rvalue, location);
        }
        _ => {}
      },
      Either::Right(terminator) => {
        visitor.visit_terminator(terminator, location);
      }
    };

    for (input, _) in visitor.places {
      for place in input.place_and_refs_in_projection(tcx) {
        inputs.insert(place);
      }
    }

    inputs
  };

  let mut adj_mtx = IndexMatrix::new(location_domain, location_domain);
  for location in traversal::reverse_postorder(body)
    .map(|(block, _)| body.locations_in_block(block))
    .flatten()
  {
    let inputs = inputs_at(location);
    let infoflow = results.state_at(location);

    for input in inputs.iter() {
      if let Some(deps) = infoflow.row_set(*input) {
        adj_mtx.union_into_row(location, &deps);
      }
    }
  }

  adj_mtx
}

type LocGraph = DiGraph<Vec<Location>, ()>;

fn compute_graph(
  adj_mtx: IndexMatrix<Location, Location>,
  results: &FlowResults<'_, 'tcx>,
) -> LocGraph {
  let mut g = DiGraph::<Location, ()>::default();
  let loc_idx_to_pg_idx = results
    .analysis
    .location_domain()
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
    g.add_edge(loc_idx_to_pg_idx[&src], loc_idx_to_pg_idx[&dst], ());
  }

  let to_remove = g
    .node_indices()
    .filter(|n| g.neighbors(*n).next().is_none())
    .collect::<Vec<_>>();
  for i in to_remove.into_iter().rev() {
    g.remove_node(i);
  }

  let g = algo::condensation(g, true);
  let g = super::algo::transitive_reduction(&g);

  g
}

pub fn build(
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  results: &FlowResults<'_, 'tcx>,
) -> LocGraph {
  let adj_mtx = compute_adjacency_matrix(body, tcx, results);
  let g = compute_graph(adj_mtx, results);

  let dot = Dot::with_config(&g, &[DotConfig::EdgeNoLabel]);
  run_dot(Path::new("test.pdf"), format!("{dot:?}").into_bytes()).unwrap();

  g
}
