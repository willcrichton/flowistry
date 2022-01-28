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
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  results: &FlowResults<'_, 'tcx>,
) -> IndexMatrix<Location, Location> {
  let analysis = &results.analysis;
  let location_domain = analysis.location_domain();
  let place_domain = analysis.place_domain();

  let mut mutation_locs = IndexMatrix::new(place_domain, location_domain);

  for arg in place_domain
    .as_vec()
    .iter()
    .filter(|place| place.is_arg(body))
  {
    let index = place_domain.index(arg);
    let loc = location_domain.arg_to_location(index);
    mutation_locs.insert(index, loc);
  }

  ModularMutationVisitor::new(tcx, body, def_id, |mutated, _, location, _| {
    for p in analysis.aliases.conflicts(mutated).indices() {
      mutation_locs.insert(p, location);
    }
  })
  .visit_body(body);

  log::debug!("mutation_locs: {mutation_locs:#?}");

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
        inputs.union(&analysis.aliases.conflicts(place));
      }
    }

    inputs
  };

  let mut adj_mtx = IndexMatrix::new(location_domain, location_domain);
  for location in traversal::reverse_postorder(body)
    .flat_map(|(block, _)| body.locations_in_block(block))
    .filter(|location| match body.stmt_at(*location) {
      Either::Left(_) => true,
      Either::Right(terminator) => !matches!(
        terminator.kind,
        TerminatorKind::DropAndReplace { .. }
          | TerminatorKind::Drop { .. }
          | TerminatorKind::Goto { .. }
          | TerminatorKind::Return
          | TerminatorKind::Resume
      ),
    })
  {
    let inputs = inputs_at(location);
    let infoflow = results.state_at(location);
    for input in inputs.indices() {
      let mut deps = infoflow.row_set(input).to_owned();
      deps.intersect(&mutation_locs.row_set(input));
      adj_mtx.union_into_row(location, &deps);
    }

    if let Some(control_deps) = analysis.control_dependencies.dependent_on(location.block)
    {
      for block in control_deps.iter() {
        adj_mtx.insert(location, body.terminator_loc(block));
      }
    }
  }
  log::debug!("adj_mtx {adj_mtx:#?}");

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

pub fn build(
  body: &Body<'tcx>,
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  results: &FlowResults<'_, 'tcx>,
) -> LocGraph {
  let adj_mtx = compute_adjacency_matrix(body, tcx, def_id, results);
  compute_graph(adj_mtx, results)
}
