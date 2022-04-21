use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils::SpanExt},
  source_map::{self, Range},
};
use itertools::Itertools;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;
use serde::Serialize;

mod direct_influence;

#[derive(Debug, Serialize)]
pub struct PlaceInfo {
  pub range: Range,
  pub ranges: Vec<Range>,
  pub slice: Vec<Range>,
  pub direct_influence: Vec<Range>,
}

#[derive(Debug, Serialize)]
pub struct FocusOutput {
  pub place_info: Vec<PlaceInfo>,
  pub containers: Vec<Range>,
}

pub fn focus(tcx: TyCtxt, body_id: BodyId) -> Result<FocusOutput> {
  let def_id = tcx.hir().body_owner_def_id(body_id);
  let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
  let body = &body_with_facts.body;
  let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);
  let location_domain = results.analysis.location_domain();

  let source_map = tcx.sess.source_map();
  let spanner = source_map::Spanner::new(tcx, body_id, body);

  let grouped_spans = spanner
    .mir_span_tree
    .iter()
    .map(|mir_span| {
      (
        mir_span.span,
        mir_span
          .locations
          .iter()
          .map(|location| (mir_span.place, *location))
          .collect::<Vec<_>>(),
      )
    })
    .into_group_map()
    .into_iter()
    .map(|(k, vs)| (k, vs.concat()))
    .collect::<Vec<_>>();

  let targets = grouped_spans
    .iter()
    .map(|(_, target)| target.clone())
    .collect();

  let relevant =
    infoflow::compute_dependency_spans(results, targets, Direction::Both, &spanner);

  let direct = direct_influence::DirectInfluence::build(body, &results.analysis.aliases);

  let slices = grouped_spans
    .iter()
    .zip(relevant)
    .filter_map(|((mir_span, targets), relevant)| {
      log::debug!("Slice for {mir_span:?} is {relevant:#?}");

      let direct_influence = targets
        .iter()
        .flat_map(|(target, _)| direct.lookup(*target))
        .flat_map(|location| {
          spanner.location_to_spans(
            location,
            location_domain,
            source_map::EnclosingHirSpans::None,
          )
        })
        .filter(|span| relevant.iter().any(|slice_span| slice_span.contains(*span)))
        .collect::<Vec<_>>();

      let slice = relevant;

      let to_ranges = |v: Vec<Span>| {
        v.into_iter()
          .filter_map(|span| span.trim_leading_whitespace(source_map))
          .flatten()
          .filter_map(|span| Range::from_span(span, source_map).ok())
          .collect::<Vec<_>>()
      };

      Some(PlaceInfo {
        range: Range::from_span(mir_span.span(), source_map).ok()?,
        ranges: to_ranges(vec![mir_span.span()]),
        slice: to_ranges(slice),
        direct_influence: to_ranges(direct_influence),
      })
    })
    .collect::<Vec<_>>();

  let body_range = Range::from_span(spanner.body_span, source_map)?;
  let ret_range = Range::from_span(spanner.ret_span, source_map)?;
  let mut containers = vec![body_range, ret_range];

  let hir_body = tcx.hir().body(body_id);
  let arg_span = hir_body
    .params
    .iter()
    .map(|param| param.span)
    .reduce(|s1, s2| s1.to(s2));
  if let Some(sp) = arg_span {
    containers.push(Range::from_span(sp, source_map)?);
  }

  Ok(FocusOutput {
    place_info: slices,
    containers,
  })
}
