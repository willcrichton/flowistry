use log::{debug, trace};
use rustc_data_structures::{
  fx::FxHashSet as HashSet, graph::iterate::reverse_post_order,
};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor as HirVisitor},
  BodyId, Expr, ExprKind, Stmt,
};
use rustc_index::bit_set::HybridBitSet;
use rustc_middle::{
  hir::map::Map,
  mir::{
    visit::{
      MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext,
      Visitor as MirVisitor,
    },
    *,
  },
  ty::TyCtxt,
};
use rustc_span::{source_map::SourceMap, Pos, Span};
use smallvec::{smallvec, SmallVec};

use crate::mir::utils::{self};

type SpanVec = SmallVec<[Span; 4]>;

pub struct HirSpanner {
  expr_spans: Vec<(Span, SpanVec)>,
  stmt_spans: Vec<(Span, SpanVec)>,
  body_span: Span,
}

fn compute_outer_spans(span: Span, f: impl FnOnce(&mut ChildExprSpans) -> ()) -> SpanVec {
  let mut child_spans = {
    let mut visitor = ChildExprSpans::default();
    f(&mut visitor);
    visitor.0
  };

  let mut outer_spans = smallvec![];
  if !child_spans.is_empty() {
    child_spans.sort_by_key(|s| s.lo());

    let start = span.until(*child_spans.first().unwrap());
    if (start.hi() - start.lo()).0 > 0 {
      outer_spans.push(start);
    }

    for children in child_spans.windows(2) {
      outer_spans.push(children[0].between(children[1]));
    }

    if let Some(end) = span.trim_start(*child_spans.last().unwrap()) {
      outer_spans.push(end);
    }
  } else {
    outer_spans.push(span);
  };

  debug!(
    "outer span for {:?} with inner spans {:?} is {:?}",
    span, child_spans, outer_spans
  );

  outer_spans
}

#[derive(Default)]
struct ChildExprSpans(SpanVec);

impl HirVisitor<'hir> for ChildExprSpans {
  type Map = Map<'hir>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::None
  }

  fn visit_expr(&mut self, ex: &Expr) {
    match ex.kind {
      ExprKind::Block(..) => {
        intravisit::walk_expr(self, ex);
      }
      _ => {
        self.0.push(ex.span.source_callsite());
      }
    }
  }

  fn visit_stmt(&mut self, stmt: &Stmt) {
    self.0.push(stmt.span.source_callsite());
  }
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

    impl HirVisitor<'hir> for Collector<'_> {
      type Map = Map<'hir>;

      fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
      }

      // source_callsite gets the top-level source location if span is
      // from a macro expansion
      fn visit_expr(&mut self, expr: &Expr) {
        intravisit::walk_expr(self, expr);

        let span = expr.span.source_callsite();
        if span == self.0.body_span {
          return;
        }

        let outer_spans =
          compute_outer_spans(span, move |visitor| intravisit::walk_expr(visitor, expr));

        self.0.expr_spans.push((span, outer_spans));
      }

      fn visit_stmt(&mut self, stmt: &Stmt) {
        intravisit::walk_stmt(self, stmt);

        let span = stmt.span.source_callsite();
        let outer_spans =
          compute_outer_spans(span, move |visitor| intravisit::walk_stmt(visitor, stmt));
        self.0.stmt_spans.push((span, outer_spans));
      }
    }

    let mut collector = Collector(&mut spanner);
    intravisit::walk_body(&mut collector, body);

    spanner
  }

  pub fn find_enclosing_hir_span(&self, span: Span) -> SpanVec {
    let find = |spans: &[(Span, SpanVec)]| {
      spans
        .iter()
        .filter(|(hir_span, _)| hir_span.contains(span))
        .min_by_key(|(hir_span, _)| hir_span.hi() - hir_span.lo())
        .map(|(_, v)| v.iter().copied())
        .into_iter()
        .flatten()
        .collect::<SpanVec>()
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

        for pred in body.predecessors()[location.block]
          .iter()
          .filter(|pred| reachable_set.contains(**pred))
        {
          let loop_span = body.source_info(body.terminator_loc(*pred)).span;
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

  hir_spans.into_iter().chain(mir_spans).collect()
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

pub fn span_to_place(
  body: &Body<'tcx>,
  body_span: Span,
  span: Span,
) -> Option<(Place<'tcx>, Location, Span)> {
  struct FindSpannedPlaces<'a, 'tcx> {
    body: &'a Body<'tcx>,
    body_span: Span,
    span: Span,
    contained: HashSet<(Place<'tcx>, Location, Span)>,
    containing: HashSet<(Place<'tcx>, Location, Span)>,
  }

  impl MirVisitor<'tcx> for FindSpannedPlaces<'_, 'tcx> {
    fn visit_place(
      &mut self,
      place: &Place<'tcx>,
      context: PlaceContext,
      location: Location,
    ) {
      // Three cases, shown by example:
      //   fn foo(x: i32) {
      //     let y = x + 1;
      //   }
      // If the user selects...
      // * "x: i32" -- this span is contained in the LocalDecls for _1,
      //   which is represented by NonUseContext::VarDebugInfo
      // * "x + 1" -- MIR will generate a temporary to assign x into, whose
      //   span is given to "x". That corresponds to MutatingUseContext::Store
      // * "y" -- this corresponds to NonMutatingUseContext::Inspect
      let span = match context {
        PlaceContext::MutatingUse(MutatingUseContext::Store)
        | PlaceContext::NonMutatingUse(NonMutatingUseContext::Inspect) => {
          let source_info = self.body.source_info(location);
          source_info.span
        }
        PlaceContext::NonUse(NonUseContext::VarDebugInfo)
          if self.body.args_iter().any(|local| local == place.local) =>
        {
          let source_info = self.body.local_decls()[place.local].source_info;
          source_info.span
        }
        _ => {
          return;
        }
      };

      if span.from_expansion() || span.source_equal(&self.body_span) {
        return;
      }

      // Note that MIR does not have granular source maps around projections.
      // So in the expression `let x = z.0`, the MIR Body only contains the place
      // z.0 with a span for the string "z.0". If the user selects only "z", there
      // is no way to determine map that selection back to a subset of the projection.
      //
      // At least, we can conservatively include the containing span "z.0" and slice on that.
      if self.span.contains(span) {
        self.contained.insert((*place, location, span));
      } else if span.contains(self.span) {
        self.containing.insert((*place, location, span));
      }
    }
  }

  let mut visitor = FindSpannedPlaces {
    body,
    body_span,
    span,
    contained: HashSet::default(),
    containing: HashSet::default(),
  };
  visitor.visit_body(body);

  visitor
    .contained
    .into_iter()
    .max_by_key(|(_, _, span)| span.hi() - span.lo())
    .or_else(|| {
      visitor
        .containing
        .into_iter()
        .min_by_key(|(_, _, span)| span.hi() - span.lo())
    })
}

#[cfg(test)]
mod test {
  use test_env_log::test;

  use super::*;
  use crate::{mir::utils::BodyExt, test_utils};

  fn harness(
    src: &str,
    f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &Body<'tcx>, Vec<Span>) + Send,
  ) {
    let (input, mut ranges) = test_utils::parse_ranges(src, [("`(", ")`")]).unwrap();
    test_utils::compile_body(input, move |tcx, body_id, body_with_facts| {
      let spans = ranges
        .remove("`(")
        .unwrap()
        .into_iter()
        .map(test_utils::make_span)
        .collect::<Vec<_>>();
      f(tcx, body_id, &body_with_facts.body, spans);
    });
  }

  #[test]
  fn test_span_to_places() {
    let src = r#"fn foo(`(z)`: i32){
      let `(x)` = 1;
      let y = 1;
      `(x + y)`;
      `(x)` + y;
      `(x + )`y;
      print!("{} {}", x, `(y)`);
      let w = (0, 0);
      `(w)`.0;
      `(w.0)`;
      `(w.)`0;
    }"#;
    harness(src, |tcx, body_id, body, spans| {
      let source_map = tcx.sess.source_map();
      let expected = ["z", "x", "x + y", "x", "x", "y", "w.0", "w.0", "w.0"];
      let body_span = tcx.hir().body(body_id).value.span;
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let (_, _, output_span) = span_to_place(body, body_span, input_span)
          .unwrap_or_else(|| {
            panic!("No place for span: {:?} / {:?}", input_span, desired)
          });
        let snippet = source_map.span_to_snippet(output_span).unwrap();
        assert_eq!(snippet, desired, "{:?}", input_span);
      }
    });
  }

  fn compare_sets(desired: &HashSet<impl AsRef<str>>, actual: &HashSet<impl AsRef<str>>) {
    let desired = desired.iter().map(|s| s.as_ref()).collect::<HashSet<_>>();
    let actual = actual.iter().map(|s| s.as_ref()).collect::<HashSet<_>>();
    let missing_desired = &desired - &actual;
    let missing_actual = &actual - &desired;

    let check = |key: &str, set: HashSet<&str>| {
      if let Some(el) = set.iter().next() {
        panic!(
          "Missing {}: {}. Actual = {:?}. Desired = {:?}",
          key, el, actual, desired
        );
      }
    };

    check("desired", missing_desired);
    check("actual", missing_actual);
  }

  #[test]
  fn test_hir_spanner() {
    let src = r#"fn foo() {
      let x = `(1)`;
      let `(y)` = x + 1;
      if `(true)` { let z = 1; }
      x `(+)` y;
    }"#;
    harness(src, |tcx, body_id, body, spans| {
      let spanner = HirSpanner::new(tcx, body_id);
      let source_map = tcx.sess.source_map();
      let expected: &[&[&str]] =
        &[&["1", "let x = ", ";"], &["let y = ", ";"], &["true"], &[
          " + ",
        ]];
      debug!("{}", body.to_string(tcx).unwrap());
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let output_spans = spanner.find_enclosing_hir_span(input_span);
        let desired_set = desired.into_iter().copied().collect::<HashSet<_>>();
        let output_snippets = output_spans
          .into_iter()
          .map(|s| source_map.span_to_snippet(s).unwrap())
          .collect::<HashSet<_>>();
        compare_sets(&desired_set, &output_snippets);
      }
    });
  }

  #[test]
  fn test_location_to_spans() {
    let src = r#"fn foo() {
      let mut x = 1;
      let y = x + 2;
      if true {
         let z = 0; 
      }
      let z = &mut x; 
      *z = 2;
    }"#;
    let (input, _ranges) = test_utils::parse_ranges(src, [("`(", ")`")]).unwrap();
    test_utils::compile_body(input, move |tcx, body_id, body_with_facts| {
      debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

      let source_map = tcx.sess.source_map();

      let spanner = HirSpanner::new(tcx, body_id);

      let mk_loc = |i, j| Location {
        block: BasicBlock::from_usize(i),
        statement_index: j,
      };

      let pairs: &[(_, &[&str])] = &[
        (mk_loc(0, 0), &["let mut x = ", "1", ";"]),
        (mk_loc(0, 2), &["let y = ", "x", ";"]),
        (mk_loc(0, 3), &["let y = ", " + ", "x + 2", ";"]),
        (mk_loc(1, 2), &["true"]),
        (mk_loc(1, 3), &["true"]),
        (mk_loc(4, 0), &["let z = ", "&mut ", "&mut x", ";"]),
        (mk_loc(4, 2), &[" = ", ";", "*z = 2"]),
      ];

      for (loc, outp) in pairs {
        let spans = location_to_spans(*loc, &body_with_facts.body, &spanner, source_map);
        let desired = outp.iter().collect::<HashSet<_>>();
        let actual = spans
          .into_iter()
          .map(|s| source_map.span_to_snippet(s).unwrap())
          .collect::<HashSet<_>>();
        compare_sets(&desired, &actual);
      }
    });
  }
}
