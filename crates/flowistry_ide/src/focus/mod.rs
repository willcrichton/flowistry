use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::borrowck_facts::get_body_with_borrowck_facts,
  range::Range,
  source_map,
};
use itertools::Itertools;
use rustc_hir::{BodyId, Expr, ExprKind, Node};
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;

mod find_mutations;

#[derive(Debug, Encodable)]
pub struct PlaceInfo {
  range: Range,
  slice: Vec<Range>,
  mutations: Vec<Range>,
}

#[derive(Debug, Encodable)]
pub struct FocusOutput {
  place_info: Vec<PlaceInfo>,
  body_range: Range,
  arg_range: Range,
}

pub fn focus(tcx: TyCtxt<'tcx>, body_id: BodyId) -> Result<FocusOutput> {
  let def_id = tcx.hir().body_owner_def_id(body_id);
  let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
  let body = &body_with_facts.body;
  let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

  let source_map = tcx.sess.source_map();
  let spanner = source_map::Spanner::new(tcx, body_id, body);

  use std::collections::HashMap;

  use rustc_span::SpanData;
  let mut groups: HashMap<SpanData, usize> = HashMap::default();
  for mir_span in &spanner.mir_spans {
    *groups.entry(mir_span.span).or_insert(0) += 1;
  }
  log::info!("groups: {:?}", groups.values().collect::<Vec<_>>());
  log::info!(
    "avg: {:?} / {:?}",
    groups.values().filter(|x| **x > 1).sum::<usize>(),
    spanner.mir_spans.len()
  );

  let grouped_spans = spanner
    .mir_spans
    .iter()
    .map(|mir_span| (mir_span.span, (mir_span.place, mir_span.location)))
    .into_group_map()
    .into_iter()
    .collect::<Vec<_>>();
  let targets = grouped_spans
    .iter()
    .map(|(_, target)| target.clone())
    .collect();

  let relevant =
    infoflow::compute_dependency_spans(results, targets, Direction::Both, &spanner);

  let slices = grouped_spans
    .iter()
    .zip(relevant)
    .filter_map(|((mir_span, _targets), relevant)| {
      log::debug!("Slice for {mir_span:?} is {relevant:#?}");
      let range = Range::from_span(mir_span.span(), source_map).ok()?;

      let slice = relevant
        .into_iter()
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>();

      // TODO: restore this code once find_mutations is integrated
      // let mutations = find_mutations::find_mutations(
      //   tcx,
      //   body,
      //   def_id.to_def_id(),
      //   mir_span.place,
      //   &results.analysis.aliases,
      // );
      // let mutations = mutations
      //   .into_iter()
      //   .flat_map(|location| {
      //     spanner.location_to_spans(location, source_map::EnclosingHirSpans::OuterOnly)
      //   })
      //   .filter_map(|span| Range::from_span(span, source_map).ok())
      //   .collect::<Vec<_>>();

      Some(PlaceInfo {
        range,
        slice,
        mutations: Vec::new(),
      })
    })
    .collect::<Vec<_>>();

  let owner_id = tcx.hir().body_owner(body_id);
  let owner_node = tcx.hir().get(owner_id);
  let arg_span = match (owner_node.fn_sig(), owner_node) {
    (Some(sig), _) => sig.span,
    (
      None,
      Node::Expr(Expr {
        kind: ExprKind::Closure(_, _, _, arg_span, _),
        ..
      }),
    ) => *arg_span,
    _ => panic!("Unknown arg span for owner_node: {owner_node:#?}"),
  };
  let arg_range = Range::from_span(arg_span, source_map)?;
  let body_range = Range::from_span(spanner.body_span, source_map)?;

  Ok(FocusOutput {
    place_info: slices,
    body_range,
    arg_range,
  })
}
