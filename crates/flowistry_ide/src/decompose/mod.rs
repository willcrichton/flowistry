use anyhow::Result;
use flowistry::{
  infoflow::{self},
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils::SpanExt},
  source_map,
};
use log::debug;
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
  chunks: Vec<Vec<Range>>,
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

    let communities_idxs = algo::naive_greedy_modularity_communities(&graph);
    let communities = communities_idxs
      .into_iter()
      .map(|c| {
        c.into_iter()
          .flat_map(|u| graph.node_weight(u).unwrap())
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    debug!("{communities:#?}");

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

    Ok(DecomposeOutput { chunks })
  }
}

pub fn decompose(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<DecomposeOutput> {
  DecomposeAnalysis { id }.run(compiler_args)
}
