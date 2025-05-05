#![allow(warnings)]

use std::path::Path;

use anyhow::Result;
use flowistry::infoflow;
use petgraph::dot::{Config as DotConfig, Dot};
use rayon::prelude::*;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;
use rustc_utils::{
  mir::{body::run_dot, borrowck_facts::get_body_with_borrowck_facts},
  source_map::{
    self, range::ByteRange, spanner::{EnclosingHirSpans, Spanner}
  },
  SpanExt,
};
use serde::Serialize;

mod algo;
mod construct;

#[derive(Debug, Clone, Serialize, Default)]
pub struct DecomposeOutput {
  chunks: Vec<(f64, Vec<Vec<ByteRange>>)>,
}

pub fn decompose(tcx: TyCtxt, body_id: BodyId) -> Result<DecomposeOutput> {
  let def_id = tcx.hir_body_owner_def_id(body_id);
  let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
  let body = &body_with_facts.body;
  let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

  let source_map = tcx.sess.source_map();
  let spanner = Spanner::new(tcx, body_id, body);

  let graph = construct::build(body, tcx, def_id.to_def_id(), results);

  let toplevel = algo::connected_components(&graph);
  let cut = toplevel
    .into_iter()
    .flat_map(|component| {
      let subgraph = algo::subgraph(&graph, &component);
      match algo::find_cut(&subgraph) {
        Some(more_components) => more_components
          .into_iter()
          .map(|v| {
            v.into_iter()
              .map(|n| *subgraph.node_weight(n).unwrap())
              .collect()
          })
          .collect(),
        None => vec![component],
      }
    })
    .collect::<Vec<_>>();
  let communities_idxs = vec![(0., cut)];

  // let resolutions = [0.1, 0.3, 0.6, 1., 2.];
  // let communities_idxs = resolutions
  //   .par_iter()
  //   .map(|r| (*r, algo::naive_greedy_modularity_communities(&graph, *r)))
  //   .collect::<Vec<_>>();

  let fn_path = tcx.def_path_str(def_id.to_def_id());
  let fn_name = fn_path.split("::").last().unwrap();
  let chunks = communities_idxs
    .into_iter()
    .map(|(r, mut communities_idxs)| {
      communities_idxs.sort_by_key(|c| -(c.len() as i32));

      let idx_map = communities_idxs
        .iter()
        .enumerate()
        .flat_map(|(i, ns)| ns.iter().map(move |n| (*n, i)))
        .collect::<HashMap<_, _>>();

      if log::log_enabled!(log::Level::Debug) {
        const PALETTE: &[&str] = &[
          "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2",
          "#7f7f7f", "#bcbd22", "#17becf",
        ];
        let get_node_attributes = |_, (n, _)| {
          format!(
            r#"fillcolor="{}" style="filled" fontcolor="white""#,
            PALETTE[idx_map[&n] % PALETTE.len()]
          )
        };
        let dot = Dot::with_attr_getters(
          &graph,
          &[DotConfig::EdgeNoLabel],
          &|_, _| "".into(),
          &get_node_attributes,
        );
        run_dot(
          Path::new(&format!("figures/{fn_name}_{r:.2}.pdf")),
          format!("{dot:?}").as_bytes(),
        )
        .unwrap();
      }

      let communities = communities_idxs.into_iter().map(|c| {
        c.into_iter()
          .flat_map(|u| graph.node_weight(u).unwrap())
          .collect::<Vec<_>>()
      });
      let chunks = communities
        .map(|c| {
          let spans = Span::merge_overlaps(
            c.into_iter()
              .flat_map(|location| {
                spanner.location_to_spans(
                  (*location).into(),
                  body,
                  EnclosingHirSpans::OuterOnly,
                )
              })
              .collect::<Vec<_>>(),
          );

          // TODO: byterange or charrange
          spans
            .into_iter()
            .filter_map(|span| ByteRange::from_span(span, source_map).ok())
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

      (r, chunks)
    })
    .collect::<Vec<_>>();

  Ok(DecomposeOutput { chunks })
}
