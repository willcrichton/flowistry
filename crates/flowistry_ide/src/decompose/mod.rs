use std::path::Path;

use anyhow::Result;
use flowistry::{
  infoflow,
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{run_dot, SpanExt},
  },
  source_map,
};
use petgraph::dot::{Config as DotConfig, Dot};
use rayon::prelude::*;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{FunctionIdentifier, Range},
};

mod algo;
mod construct;

#[derive(Debug, Clone, Encodable, Default)]
pub struct PlaceDescriptor {
  place: String,
  local: usize,
  name: String,
  projection: Vec<String>,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct DecomposeOutput {
  chunks: Vec<(f64, Vec<Vec<Range>>)>,
}

impl FlowistryOutput for DecomposeOutput {
  fn merge(&mut self, other: Self) {
    self.chunks.extend(other.chunks);
  }
}

pub struct DecomposeAnalysis {
  id: FunctionIdentifier,
}

impl FlowistryAnalysis for DecomposeAnalysis {
  type Output = DecomposeOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.id.to_span(tcx)?])
  }

  fn analyze_function(
    &mut self,
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
  ) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;
    let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

    let source_map = tcx.sess.source_map();
    let spanner = source_map::HirSpanner::new(tcx, body_id);

    let graph = construct::build(body, tcx, results);

    let resolutions = [0.1, 0.25, 0.5, 1., 2.];
    let communities_idxs = resolutions
      .par_iter()
      .map(|r| (*r, algo::naive_greedy_modularity_communities(&graph, *r)))
      .collect::<Vec<_>>();

    let chunks = communities_idxs
      .into_iter()
      .map(|(r, communities_idxs)| {
        let idx_map = communities_idxs
          .iter()
          .enumerate()
          .flat_map(|(i, ns)| ns.iter().map(move |n| (*n, i)))
          .collect::<HashMap<_, _>>();
        let communities = communities_idxs
          .into_iter()
          .map(|c| {
            c.into_iter()
              .flat_map(|u| graph.node_weight(u).unwrap())
              .collect::<Vec<_>>()
          })
          .collect::<Vec<_>>();

        if log::log_enabled!(log::Level::Debug) {
          const PALETTE: &[&str] = &[
            "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2",
            "#7f7f7f", "#bcbd22", "#17becf",
          ];
          let get_color =
            |_, (n, _)| format!("color=\"{}\"", PALETTE[idx_map[&n] % PALETTE.len()]);
          let dot = Dot::with_attr_getters(
            &graph,
            &[DotConfig::EdgeNoLabel],
            &|_, _| "".into(),
            &get_color,
          );
          run_dot(
            Path::new(&format!("test_{r:2}.pdf")),
            format!("{dot:?}").into_bytes(),
          )
          .unwrap();
        }

        let chunks = communities
          .into_iter()
          .map(|c| {
            let spans = Span::merge_overlaps(
              c.into_iter()
                .flat_map(|location| {
                  source_map::location_to_spans(*location, tcx, body, &spanner)
                })
                .collect::<Vec<_>>(),
            );

            spans
              .into_iter()
              .filter_map(|span| Range::from_span(span, source_map).ok())
              .collect::<Vec<_>>()
          })
          .collect::<Vec<_>>();

        (r, chunks)
      })
      .collect::<Vec<_>>();

    // debug!(
    //   "{:#?}",
    //   communities
    //     .iter()
    //     .map(|c| {
    //       c.iter()
    //         .map(|l| {
    //           (
    //             l,
    //             source_map::location_to_spans(**l, tcx, body, &spanner)
    //               .into_iter()
    //               .map(|s| source_map.span_to_snippet(s).unwrap())
    //               .collect::<Vec<_>>(),
    //           )
    //         })
    //         .collect::<HashMap<_, _>>()
    //     })
    //     .collect::<Vec<_>>()
    // );

    Ok(DecomposeOutput { chunks })
  }
}

pub fn decompose(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<DecomposeOutput> {
  DecomposeAnalysis { id }.run(compiler_args)
}
