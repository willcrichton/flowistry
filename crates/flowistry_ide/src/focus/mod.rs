use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::borrowck_facts::get_body_with_borrowck_facts,
  range::Range,
  source_map,
};
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

  let targets = spanner
    .mir_spans
    .iter()
    .map(|mir_span| (mir_span.place, mir_span.location))
    .collect::<Vec<_>>();
  let relevant = infoflow::compute_dependency_spans(
    results,
    targets.clone(),
    Direction::Both,
    &spanner,
  );

  let slices = spanner
    .mir_spans
    .iter()
    .zip(relevant)
    .filter_map(|(mir_span, relevant)| {
      log::debug!("Slice for {mir_span:?} is {relevant:#?}");
      let range = Range::from_span(mir_span.span, source_map).ok()?;

      let slice = relevant
        .into_iter()
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>();

      let mutations = find_mutations::find_mutations(
        tcx,
        body,
        def_id.to_def_id(),
        mir_span.place,
        &results.analysis.aliases,
      );
      let mutations = mutations
        .into_iter()
        .flat_map(|location| {
          spanner.location_to_spans(location, source_map::EnclosingHirSpans::OuterOnly)
        })
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>();

      Some(PlaceInfo {
        range,
        slice,
        mutations,
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
