use either::Either;
use log::trace;
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
use rustc_span::Span;

use crate::mir::utils::{self, SpanExt};

type HirNode<'hir> = Either<&'hir Expr<'hir>, &'hir Stmt<'hir>>;

// Collect all the spans for children beneath the visited node.
// For example, when visiting "if true { 1 } else { 2 }" then we
// should collect: "true" "1" "2"
struct ChildExprSpans<'tcx> {
  spans: Vec<Span>,
  tcx: TyCtxt<'tcx>,
}
impl HirVisitor<'hir> for ChildExprSpans<'_> {
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
        if let Some(span) = ex.span.as_local(self.tcx) {
          self.spans.push(span);
        }
      }
    }
  }

  fn visit_stmt(&mut self, stmt: &Stmt) {
    if let Some(span) = stmt.span.as_local(self.tcx) {
      self.spans.push(span);
    }
  }
}

#[derive(Clone)]
pub enum EnclosingHirSpans {
  OuterOnly,
  Full,
}

pub struct HirSpanner<'hir> {
  spans: Vec<(Span, Vec<Span>, HirNode<'hir>)>,
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

    struct Collector<'a, 'hir, 'tcx> {
      spanner: &'a mut HirSpanner<'hir>,
      tcx: TyCtxt<'tcx>,
    }

    impl HirVisitor<'hir> for Collector<'_, 'hir, '_> {
      type Map = Map<'hir>;

      fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
        NestedVisitorMap::None
      }

      fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
        intravisit::walk_expr(self, expr);

        let span = match expr.span.as_local(self.tcx) {
          Some(span) if span != self.spanner.body_span => span,
          _ => {
            return;
          }
        };

        let mut visitor = ChildExprSpans {
          spans: Vec::new(),
          tcx: self.tcx,
        };
        intravisit::walk_expr(&mut visitor, expr);
        let outer_spans = span.subtract(visitor.spans);

        self
          .spanner
          .spans
          .push((span, outer_spans, Either::Left(expr)));
      }

      fn visit_stmt(&mut self, stmt: &'hir Stmt<'hir>) {
        intravisit::walk_stmt(self, stmt);

        let span = match stmt.span.as_local(self.tcx) {
          Some(span) if span != self.spanner.body_span => span,
          _ => {
            return;
          }
        };

        let mut visitor = ChildExprSpans {
          spans: Vec::new(),
          tcx: self.tcx,
        };
        intravisit::walk_stmt(&mut visitor, stmt);
        let outer_spans = span.subtract(visitor.spans);

        self
          .spanner
          .spans
          .push((span, outer_spans, Either::Right(stmt)));
      }
    }

    let mut collector = Collector {
      spanner: &mut spanner,
      tcx,
    };
    intravisit::walk_body(&mut collector, body);

    spanner
  }

  pub fn find_enclosing_hir(
    &self,
    span: Span,
    span_type: EnclosingHirSpans,
  ) -> Vec<(Vec<Span>, HirNode<'hir>)> {
    let mut enclosing = self
      .spans
      .iter()
      .filter(|(node_span, _, _)| node_span.contains(span))
      .collect::<Vec<_>>();
    enclosing.sort_by_key(|(node_span, _, _)| node_span.hi() - node_span.lo());

    enclosing
      .into_iter()
      .map(|(node_span, spans, node)| {
        (
          match span_type {
            EnclosingHirSpans::OuterOnly => spans.clone(),
            EnclosingHirSpans::Full => vec![*node_span],
          },
          *node,
        )
      })
      .collect()
  }
}

pub fn location_to_spans(
  location: Location,
  tcx: TyCtxt<'_>,
  body: &Body,
  spanner: &HirSpanner,
  span_type: EnclosingHirSpans,
) -> Vec<Span> {
  // special case for synthetic locations that represent arguments
  if location.block.as_usize() == body.basic_blocks().len() {
    return vec![];
  }

  let loc_span = match body.source_info(location).span.as_local(tcx) {
    Some(span) => span,
    None => {
      return vec![];
    }
  };

  let mut enclosing_hir = spanner.find_enclosing_hir(loc_span, span_type);

  // Get the spans of the immediately enclosing HIR node
  debug_assert!(
    !enclosing_hir.is_empty(),
    "Location {location:?} (span {loc_span:?}) had no enclosing HIR nodes"
  );
  let (mut hir_spans, _) = enclosing_hir.remove(0);

  // Include the MIR span
  hir_spans.push(loc_span);

  // Add the spans of the first enclosing statement
  if let Some((stmt_spans, _)) = enclosing_hir.iter().find(|(_, node)| node.is_right()) {
    hir_spans.extend(stmt_spans.clone());
  }

  if let Either::Right(terminator) = body.stmt_at(location) {
    match terminator.kind {
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
      .map(|span| span.to_string(tcx))
      .collect::<Vec<_>>()
      .join(" -- ")
  };

  trace!(
    "Location {location:?} ({})\n  has loc span:\n  {}\n  and HIR spans:\n  {}",
    utils::location_to_string(location, body),
    format_spans(&[loc_span]),
    format_spans(&hir_spans)
  );

  hir_spans
}

pub fn span_to_places(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  body_span: Span,
  span: Span,
) -> Vec<(Place<'tcx>, Location, Span)> {
  struct FindSpannedPlaces<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
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

      let span = match span.as_local(self.tcx) {
        Some(span) if !span.source_equal(&self.body_span) => span,
        _ => {
          return;
        }
      };

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
    tcx,
    body,
    body_span,
    span,
    contained: HashSet::default(),
    containing: HashSet::default(),
  };
  visitor.visit_body(body);

  let mut contained = Vec::from_iter(visitor.contained);
  let mut containing = Vec::from_iter(visitor.containing);

  let metric = |sp: &Span| (sp.hi() - sp.lo()).0 as i32;
  contained.sort_by_key(|(_, _, span)| metric(span));

  if !contained.is_empty() {
    let (_, _, sp) = &contained[0];
    let min = metric(sp);
    contained
      .into_iter()
      .take_while(|(_, _, sp)| metric(sp) == min)
      .collect()
  } else if !containing.is_empty() {
    containing.sort_by_key(|(_, _, span)| -metric(span));
    let (_, _, sp) = &containing[0];
    let max = metric(sp);
    containing
      .into_iter()
      .take_while(|(_, _, sp)| metric(sp) == max)
      .collect()
  } else {
    vec![]
  }
}

#[cfg(test)]
mod test {
  use log::debug;
  use test_log::test;

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
      let expected: &[&[_]] = &[
        &["z"],
        &["x"],
        &["x", "y"],
        &["x"],
        &["x"],
        &["y"],
        &["w.0"],
        &["w.0"],
        &["w.0"],
      ];
      let body_span = tcx.hir().body(body_id).value.span;
      for (input_span, desired) in spans.into_iter().zip(expected.into_iter()) {
        let outputs = span_to_places(tcx, body, body_span, input_span);
        let snippets = outputs
          .into_iter()
          .map(|(_, _, span)| source_map.span_to_snippet(span).unwrap())
          .collect::<HashSet<_>>();

        compare_sets(&desired.iter().collect::<HashSet<_>>(), &snippets);
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
        panic!("Missing {key}: {el}. Actual = {actual:?}. Desired = {desired:?}",);
      }
    };

    check("desired", missing_desired);
    check("actual", missing_actual);
  }

  fn hir_spanner_harness(
    src: &str,
    expected: &[&[&str]],
    enclosing_spans: EnclosingHirSpans,
  ) {
    harness(src, |tcx, body_id, body, spans| {
      let spanner = HirSpanner::new(tcx, body_id);
      let source_map = tcx.sess.source_map();
      debug!("{}", body.to_string(tcx).unwrap());
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let mut enclosing =
          spanner.find_enclosing_hir(input_span, enclosing_spans.clone());
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
  fn test_hir_spanner_outer() {
    let src = r#"fn foo() {
      let x = `(1)`;
      let `(y)` = x + 1;
      if `(true)``( )`{ let z = 1; }
      x `(+)` y;
    }"#;
    let expected: &[&[&str]] = &[
      &["1"],
      &["let y = ", ";"],
      &["true"],
      &["if ", " { ", " }"],
      &[" + "],
    ];
    hir_spanner_harness(src, expected, EnclosingHirSpans::OuterOnly)
  }

  #[test]
  fn test_hir_spanner_full() {
    let src = r#"fn foo() {
      `(let mut x: Vec<()> = Vec::new();)`
      `(x = Vec::new();)`
    }"#;
    let expected: &[&[&str]] =
      &[&["let mut x: Vec<()> = Vec::new();"], &["x = Vec::new();"]];
    hir_spanner_harness(src, expected, EnclosingHirSpans::Full)
  }

  #[test]
  fn test_location_to_spans() {
    let src = r#"fn foo() {
  let mut x = 1;
  let y = x + 2;
  let w = if true {
    let z = 0;
    z
  } else {
    3
  };
  let z = &mut x; 
  *z = 4;
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

      // These locations are just selected by inspecting the actual body, so this test might break
      // if the compiler is updated. Run with RUST_LOG=debug to see the body.
      let pairs: &[(_, &[&str])] = &[
        (mk_loc(0, 0), &["let mut x = ", "1", ";"]),
        (mk_loc(0, 2), &["let y = ", "x", ";"]),
        (mk_loc(0, 3), &["let y = ", " + ", "x + 2", ";"]),
        (mk_loc(1, 2), &["let w = ", "true", ";"]),
        (mk_loc(1, 3), &[
          "let w = ",
          "if ",
          "true",
          " {\n    ",
          "\n    ",
          "\n  } else {\n    ",
          "\n  }",
          ";",
        ]),
        (mk_loc(4, 1), &["let z = ", "&mut ", "&mut x", ";"]),
        (mk_loc(4, 3), &[" = ", ";", "*z = 4"]),
      ];

      for (loc, outp) in pairs {
        let spans = location_to_spans(
          *loc,
          tcx,
          &body_with_facts.body,
          &spanner,
          EnclosingHirSpans::OuterOnly,
        );
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
