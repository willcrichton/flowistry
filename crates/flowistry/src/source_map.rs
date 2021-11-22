use crate::mir::utils;

use log::trace;
use rustc_data_structures::graph::{iterate::reverse_post_order, WithPredecessors};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  BodyId, Expr, Stmt,
};
use rustc_index::bit_set::HybridBitSet;

use rustc_middle::{hir::map::Map, mir::*, ty::TyCtxt};
use rustc_span::{source_map::SourceMap, Pos, Span};
use smallvec::{smallvec, SmallVec};

pub struct HirSpanner {
  expr_spans: Vec<Span>,
  stmt_spans: Vec<Span>,
  body_span: Span,
}

impl HirSpanner {
  pub fn new(tcx: TyCtxt, body_id: BodyId) -> Self {
    let body = tcx.hir().body(body_id);

    let mut spanner = HirSpanner {
      expr_spans: Vec::new(),
      stmt_spans: Vec::new(),
      body_span: body.value.span,
    };

    struct Collector<'a>(&'a mut HirSpanner);

    impl Visitor<'hir> for Collector<'_> {
      type Map = Map<'hir>;

      fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
      }

      // source_callsite gets the top-level source location if span is
      // from a macro expansion
      fn visit_expr(&mut self, expr: &Expr) {
        self.0.expr_spans.push(expr.span.source_callsite());
        intravisit::walk_expr(self, expr);
      }

      fn visit_stmt(&mut self, stmt: &Stmt) {
        self.0.stmt_spans.push(stmt.span.source_callsite());
        intravisit::walk_stmt(self, stmt);
      }
    }

    let mut collector = Collector(&mut spanner);
    intravisit::walk_body(&mut collector, body);

    spanner
  }

  pub fn find_enclosing_hir_span(&self, span: Span) -> SmallVec<[Span; 2]> {
    let find = |spans: &[Span]| {
      spans
        .iter()
        .filter(|hir_span| hir_span.contains(span))
        .min_by_key(|hir_span| hir_span.hi() - hir_span.lo())
        .cloned()
        .and_then(|span| (span != self.body_span).then(move || span))
    };

    find(&self.expr_spans)
      .into_iter()
      .chain(find(&self.stmt_spans))
      .collect()
  }
}

pub fn location_to_spans(
  location: Location,
  body: &Body,
  spanner: &HirSpanner,
  source_map: &SourceMap,
) -> SmallVec<[Span; 4]> {
  // special case for synthetic locations that represent arguments
  if location.block.as_usize() == body.basic_blocks().len() {
    return smallvec![];
  }

  let mut mir_spans: SmallVec<[Span; 2]> = smallvec![body.source_info(location).span];
  let block = &body.basic_blocks()[location.block];
  if location.statement_index == block.statements.len() {
    match block.terminator().kind {
      TerminatorKind::SwitchInt { .. } => {
        let mut reachable_set = HybridBitSet::new_empty(body.basic_blocks().len());
        for block in reverse_post_order(body, location.block) {
          reachable_set.insert(block);
        }

        for pred in WithPredecessors::predecessors(body, location.block)
          .filter(|pred| reachable_set.contains(*pred))
        {
          let loop_span = body.source_info(body.terminator_loc(pred)).span;
          mir_spans.push(loop_span);
        }
      }
      _ => {}
    }
  }

  // source_callsite gets the top-level source location if span is
  // from a macro expansion
  for span in mir_spans.iter_mut() {
    *span = span.source_callsite();
  }

  let format_spans = |spans: &[Span]| -> String {
    spans
      .iter()
      .map(|span| span_to_string(*span, source_map))
      .collect::<Vec<_>>()
      .join(" -- ")
  };

  let hir_spans = mir_spans
    .clone()
    .into_iter()
    .map(|mir_span| spanner.find_enclosing_hir_span(mir_span))
    .flatten()
    .collect::<SmallVec<[Span; 4]>>();

  trace!(
    "Location {:?} ({})\n  has MIR spans:\n  {}\n  and HIR spans:\n  {}",
    location,
    utils::location_to_string(location, body),
    format_spans(&mir_spans),
    format_spans(&hir_spans)
  );

  hir_spans
}

pub fn span_to_string(span: Span, source_map: &SourceMap) -> String {
  let lo = source_map.lookup_char_pos(span.lo());
  let hi = source_map.lookup_char_pos(span.hi());
  let snippet = source_map.span_to_snippet(span).unwrap();
  format!(
    "{} ({}:{}-{}:{})",
    snippet,
    lo.line,
    lo.col.to_usize() + 1,
    hi.line,
    hi.col.to_usize() + 1
  )
}
