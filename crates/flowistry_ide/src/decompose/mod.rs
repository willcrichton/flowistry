use std::path::Path;

use anyhow::Result;
use flowistry::{
  infoflow,
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{run_dot, SpanExt},
  },
  source_map::{self, EnclosingHirSpans},
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

// fn adjacent_chunk_analysis() {

//   let domain = results.analysis.location_domain();
//   let adj_mtx = construct::compute_adjacency_matrix(body, tcx, results);

//   let mut locations = domain
//     .as_vec()
//     .iter()
//     .filter_map(|loc| {
//       let spans = source_map::location_to_spans(
//         *loc,
//         tcx,
//         body,
//         &spanner,
//         EnclosingHirSpans::Full,
//       );
//       if spans.is_empty() {
//         return None;
//       }

//       let (mins, maxs): (Vec<_>, Vec<_>) = spans
//         .into_iter()
//         .map(|s| {
//           let lines = source_map.span_to_lines(s).unwrap();
//           let line_nums = lines
//             .lines
//             .into_iter()
//             .map(|l| l.line_index)
//             .collect::<Vec<_>>();
//           (
//             *line_nums.iter().min().unwrap(),
//             *line_nums.iter().max().unwrap(),
//           )
//         })
//         .unzip();
//       let min = *mins.iter().min().unwrap();
//       let max = *maxs.iter().max().unwrap();

//       Some((*loc, (min, max)))
//     })
//     .collect::<Vec<_>>();
//   locations.sort_by_key(|(_, (min, _))| *min);

//   let flows = |l1, l2| match adj_mtx.row_set(l2) {
//     Some(set) => set.contains(l1),
//     None => false,
//   };

//   let adjacent = |(min1, max1), (min2, max2)| max1 + 1 >= min2 && min1 <= max2;

//   let mut communities = vec![vec![locations.remove(0)]];
//   for (loc1, rng1) in locations {
//     let cur = communities.last_mut().unwrap();
//     if cur.iter().any(|(loc2, rng2)| {
//       let has_flow = flows(loc1, *loc2) || flows(*loc2, loc1);
//       let line_adjacent = adjacent(rng1, *rng2) || adjacent(*rng2, rng1);
//       debug!("{rng1:?} / {rng2:?} {line_adjacent}");
//       has_flow || line_adjacent
//     }) {
//       cur.push((loc1, rng1));
//     } else {
//       communities.push(vec![(loc1, rng1)]);
//     }
//     // let overlaps = spans1
//     //   .iter()
//     //   .any(|s1| spans2.iter().any(|s2| s1.overlaps_inclusive(*s2)));
//   }
//   let communities = communities
//     .into_iter()
//     .map(|c| c.into_iter().map(|(l, _)| l).collect::<Vec<_>>())
//     .collect::<Vec<_>>();
//   debug!("communities: {:#?}", communities);
// }

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
    let spanner = source_map::Spanner::new(tcx, body_id, body);

    let graph = construct::build(body, tcx, def_id.to_def_id(), results);

    let resolutions = [0.01, 0.1, 0.25, 0.5, 1.];
    let communities_idxs = resolutions
      .par_iter()
      .map(|r| (*r, algo::naive_greedy_modularity_communities(&graph, *r)))
      .collect::<Vec<_>>();

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
                  spanner.location_to_spans(*location, EnclosingHirSpans::OuterOnly)
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

    Ok(DecomposeOutput { chunks })
  }
}

pub fn decompose(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<DecomposeOutput> {
  DecomposeAnalysis { id }.run(compiler_args)
}
