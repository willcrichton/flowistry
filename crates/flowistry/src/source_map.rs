use either::Either;
use log::{debug, trace};
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor as HirVisitor},
  BodyId, Expr, ExprKind, Stmt,
};
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

use crate::mir::utils;

type SpanVec = SmallVec<[Span; 4]>;

type HirNode<'hir> = Either<&'hir Expr<'hir>, &'hir Stmt<'hir>>;

// Given a span for an AST node and a visitor function to visit that AST node,
// compute the set of spans that is span with all children removed
fn compute_outer_spans(span: Span, node: &HirNode) -> SpanVec {
  let mut child_spans = {
    let mut visitor = ChildExprSpans::default();
    match node {
      Either::Left(expr) => intravisit::walk_expr(&mut visitor, expr),
      Either::Right(stmt) => intravisit::walk_stmt(&mut visitor, stmt),
    };
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

// Collect all the spans for children beneath the visited node.
// For example, when visiting "if true { 1 } else { 2 }" then we
// should collect: "true" "1" "2"
#[derive(Default)]
struct ChildExprSpans(SpanVec);
impl HirVisitor<'hir> for ChildExprSpans {
  type Map = Map<'hir>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::None
  }

  fn visit_expr(&mut self, ex: &Expr) {
    match ex.kind {
      // Don't take the span for the whole block, since we want to leave
      // curly braces to be associated with the outer statement
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

pub struct HirSpanner<'hir> {
  spans: Vec<(Span, SpanVec, HirNode<'hir>)>,
  body_span: Span,
}

impl HirSpanner<'hir> {
  pub fn new(tcx: TyCtxt<'tcx>, body_id: BodyId) -> Self
  where
    'tcx: 'hir,
  {
    let body = tcx.hir().body(body_id);

    let mut spanner = HirSpanner {
      spans: Vec::new(),
      body_span: body.value.span,
    };

    struct Collector<'a, 'hir>(&'a mut HirSpanner<'hir>);

    impl HirVisitor<'hir> for Collector<'_, 'hir> {
      type Map = Map<'hir>;

      fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
      }

      fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
        intravisit::walk_expr(self, expr);

        let span = expr.span.source_callsite();
        if span == self.0.body_span {
          return;
        }

        let expr = Either::Left(expr);
        let outer_spans = compute_outer_spans(span, &expr);

        self.0.spans.push((span, outer_spans, expr));
      }

      fn visit_stmt(&mut self, stmt: &'hir Stmt<'hir>) {
        intravisit::walk_stmt(self, stmt);

        let span = stmt.span.source_callsite();

        let stmt = Either::Right(stmt);
        let outer_spans = compute_outer_spans(span, &stmt);

        self.0.spans.push((span, outer_spans, stmt));
      }
    }

    let mut collector = Collector(&mut spanner);
    intravisit::walk_body(&mut collector, body);

    spanner
  }

  pub fn find_enclosing_hir(&self, span: Span) -> Vec<(SpanVec, HirNode<'hir>)> {
    let mut enclosing = self
      .spans
      .iter()
      .filter(|(node_span, _, _)| node_span.contains(span))
      .collect::<Vec<_>>();
    enclosing.sort_by_key(|(node_span, _, _)| node_span.hi() - node_span.lo());

    enclosing
      .into_iter()
      .map(|(_, spans, node)| (spans.clone(), node.clone()))
      .collect()
  }
}

pub fn location_to_spans(
  location: Location,
  body: &Body,
  spanner: &HirSpanner,
  source_map: &SourceMap,
) -> SpanVec {
  // special case for synthetic locations that represent arguments
  if location.block.as_usize() == body.basic_blocks().len() {
    return smallvec![];
  }

  let loc_span = body.source_info(location).span.source_callsite();
  let mut enclosing_hir = spanner.find_enclosing_hir(loc_span);

  // Get the spans of the immediately enclosing HIR node
  let (mut hir_spans, _) = enclosing_hir.remove(0);

  // Include the MIR span
  hir_spans.push(loc_span);

  // Add the spans of the first enclosing statement
  if let Some((stmt_spans, _)) = enclosing_hir.iter().find(|(_, node)| node.is_right()) {
    hir_spans.extend(stmt_spans.clone());
  }

  let block = &body.basic_blocks()[location.block];
  if location.statement_index == block.statements.len() {
    match block.terminator().kind {
      TerminatorKind::SwitchInt { .. } => {
        // If the location is a switch, then include the closest enclosing if or loop
        if let Some(spans) = enclosing_hir
          .iter()
          .filter_map(|(spans, node)| {
            matches!(
              node.left()?.kind,
              ExprKind::If(..) | ExprKind::Loop(..) | ExprKind::Break(..)
            )
            .then(|| spans)
          })
          .next()
        {
          hir_spans.extend(spans.clone());
        }
      }
      _ => {}
    }
  }

  let format_spans = |spans: &[Span]| -> String {
    spans
      .iter()
      .map(|span| span_to_string(*span, source_map))
      .collect::<Vec<_>>()
      .join(" -- ")
  };

  trace!(
    "Location {:?} ({})\n  has loc span:\n  {}\n  and HIR spans:\n  {}",
    location,
    utils::location_to_string(location, body),
    format_spans(&[loc_span]),
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

pub fn span_overlaps(s1: Span, s2: Span) -> bool {
  let s1 = s1.data();
  let s2 = s2.data();
  s1.lo <= s2.hi && s2.lo <= s1.hi
}

pub fn simplify_spans(mut spans: Vec<Span>) -> Vec<Span> {
  spans.sort_by_key(|s| s.lo());
  let mut output = Vec::new();
  for span in spans {
    match output.iter_mut().find(|other| span_overlaps(span, **other)) {
      Some(other) => {
        *other = span.to(*other);
      }
      None => {
        output.push(span);
      }
    }
  }
  output
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
      let expected: &[&[&str]] = &[&["1"], &["let y = ", ";"], &["true"], &[" + "]];
      debug!("{}", body.to_string(tcx).unwrap());
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let mut enclosing = spanner.find_enclosing_hir(input_span);
        let desired_set = desired.into_iter().copied().collect::<HashSet<_>>();
        let output_snippets = enclosing
          .remove(0)
          .0
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
        (mk_loc(1, 3), &[
          "if ",
          " {\n         ",
          " \n      }",
          "true",
        ]),
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
