use anyhow::Result;
use flowistry::{
  infoflow::{self, Direction},
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils::SpanExt},
  source_map::{self},
};
use rustc_hir::{BodyId, Expr, ExprKind, Node};
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::{FunctionIdentifier, Range},
};

#[derive(Debug, Clone, Encodable, Default)]
pub struct Slice {
  range: Range,
  slice: Vec<Range>,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct FocusOutput {
  slices: Vec<Slice>,
  body_range: Range,
  arg_range: Range,
}

impl FlowistryOutput for FocusOutput {
  fn merge(&mut self, other: Self) {
    self.slices.extend(other.slices);
    self.body_range = other.body_range;
    self.arg_range = other.arg_range;
  }
}

pub struct FocusAnalysis {
  id: FunctionIdentifier,
}

impl FlowistryAnalysis for FocusAnalysis {
  type Output = FocusOutput;

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
      .into_iter()
      .zip(relevant)
      .filter_map(|(mir_span, relevant)| {
        let spans = Span::merge_overlaps(relevant);
        let range = Range::from_span(mir_span.span, source_map).ok()?;
        let slice = spans
          .into_iter()
          .filter_map(|span| Range::from_span(span, source_map).ok())
          .collect::<Vec<_>>();
        Some(Slice { range, slice })
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
      slices,
      body_range,
      arg_range,
    })
  }
}

pub fn focus(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<FocusOutput> {
  FocusAnalysis { id }.run(compiler_args)
}
