use either::Either;
use log::trace;
use rustc_hir::{
  intravisit::{self, Visitor as HirVisitor},
  BodyId, Expr, ExprKind, MatchSource, Stmt,
};
use rustc_middle::{
  mir::{
    visit::{
      MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext,
      Visitor as MirVisitor,
    },
    Body, HasLocalDecls, Location, Place, Terminator, TerminatorKind,
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
  fn visit_expr(&mut self, ex: &Expr) {
    match ex.kind {
      // Don't take the span for the whole block, since we want to leave
      // curly braces to be associated with the outer statement
      ExprKind::Block(..) => {
        intravisit::walk_expr(self, ex);
      }
      ExprKind::Match(_, arms, MatchSource::ForLoopDesugar) => {
        for arm in arms {
          intravisit::walk_arm(self, arm);
        }
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

#[derive(Clone, Copy)]
pub enum EnclosingHirSpans {
  OuterOnly,
  Full,
}

#[derive(Clone, Debug)]
pub struct HirSpannedNode<'hir> {
  full: Span,
  outer: Vec<Span>,
  node: HirNode<'hir>,
}

impl HirSpannedNode<'_> {
  fn get_spans(&self, span_type: EnclosingHirSpans) -> Vec<Span> {
    match span_type {
      EnclosingHirSpans::OuterOnly => self.outer.clone(),
      EnclosingHirSpans::Full => vec![self.full],
    }
  }
}

struct HirSpanCollector<'a, 'b, 'hir, 'tcx>(&'a mut Spanner<'b, 'hir, 'tcx>);

impl HirVisitor<'hir> for HirSpanCollector<'_, '_, 'hir, '_> {
  fn visit_expr(&mut self, expr: &'hir Expr<'hir>) {
    intravisit::walk_expr(self, expr);

    let span = match expr.span.as_local(self.0.tcx) {
      Some(span) if !self.0.invalid_span(span) => span,
      _ => {
        return;
      }
    };

    let inner_spans = match expr.kind {
      ExprKind::Loop(_, _, _, header) => {
        vec![expr.span.trim_start(header).unwrap_or(expr.span)]
      }
      ExprKind::Break(..) => {
        return;
      }
      _ => {
        let mut visitor = ChildExprSpans {
          spans: Vec::new(),
          tcx: self.0.tcx,
        };
        intravisit::walk_expr(&mut visitor, expr);
        visitor.spans
      }
    };

    let outer_spans = span.subtract(inner_spans.clone());

    trace!(
      "Expr:\n{}\nhas span: {:?}\nand inner spans: {:?}\nand outer spans: {:?}",
      expr_to_string(expr),
      span,
      inner_spans,
      outer_spans
    );

    if outer_spans.is_empty() {
      return;
    }

    self.0.hir_spans.push(HirSpannedNode {
      full: span,
      outer: outer_spans,
      node: Either::Left(expr),
    });
  }

  fn visit_stmt(&mut self, stmt: &'hir Stmt<'hir>) {
    intravisit::walk_stmt(self, stmt);

    let span = match stmt.span.as_local(self.0.tcx) {
      Some(span) if !self.0.invalid_span(span) => span,
      _ => {
        return;
      }
    };

    let mut visitor = ChildExprSpans {
      spans: Vec::new(),
      tcx: self.0.tcx,
    };
    intravisit::walk_stmt(&mut visitor, stmt);
    let outer_spans = span.subtract(visitor.spans);

    self.0.hir_spans.push(HirSpannedNode {
      full: span,
      outer: outer_spans,
      node: Either::Right(stmt),
    });
  }
}

#[derive(Clone, Debug)]
pub struct MirSpannedPlace<'tcx> {
  pub place: Place<'tcx>,
  pub span: Span,
  pub location: Location,
}

struct MirSpanCollector<'a, 'b, 'hir, 'tcx>(&'a mut Spanner<'b, 'hir, 'tcx>);

impl MirVisitor<'tcx> for MirSpanCollector<'_, '_, '_, 'tcx> {
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
        let source_info = self.0.body.source_info(location);
        source_info.span
      }
      PlaceContext::NonUse(NonUseContext::VarDebugInfo)
        if self.0.body.args_iter().any(|local| local == place.local) =>
      {
        let source_info = self.0.body.local_decls()[place.local].source_info;
        source_info.span
      }
      _ => {
        return;
      }
    };

    let span = match span.as_local(self.0.tcx) {
      Some(span) if !self.0.invalid_span(span) => span,
      _ => {
        return;
      }
    };

    self.0.mir_spans.push(MirSpannedPlace {
      place: *place,
      location,
      span,
    });
  }
}

pub struct Spanner<'a, 'hir, 'tcx> {
  pub hir_spans: Vec<HirSpannedNode<'hir>>,
  pub mir_spans: Vec<MirSpannedPlace<'tcx>>,
  pub body_span: Span,
  pub item_span: Span,
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
}

impl Spanner<'a, 'hir, 'tcx>
where
  'tcx: 'hir,
{
  pub fn new(tcx: TyCtxt<'tcx>, body_id: BodyId, body: &'a Body<'tcx>) -> Self {
    let hir = tcx.hir();
    let hir_body = hir.body(body_id);
    let item_span = hir.span_with_body(hir.body_owner(body_id));
    let mut spanner = Spanner {
      hir_spans: Vec::new(),
      mir_spans: Vec::new(),
      body_span: hir_body.value.span,
      item_span,
      tcx,
      body,
    };
    trace!(
      "Body span: {:?}, item span: {:?}",
      spanner.body_span,
      spanner.item_span
    );

    let mut hir_collector = HirSpanCollector(&mut spanner);
    hir_collector.visit_body(hir_body);

    let mut mir_collector = MirSpanCollector(&mut spanner);
    mir_collector.visit_body(body);

    spanner
  }

  pub fn invalid_span(&self, span: Span) -> bool {
    span.is_dummy()
      || span.source_equal(self.body_span)
      || span.source_equal(self.item_span)
  }

  pub fn find_matching<T>(
    predicate: impl Fn(Span) -> bool,
    spans: impl Iterator<Item = (Span, T)>,
  ) -> impl ExactSizeIterator<Item = T> {
    let mut matching = spans
      .filter(|(span, _)| predicate(*span))
      .collect::<Vec<_>>();
    matching.sort_by_key(|(span, _)| span.size());
    matching.into_iter().map(|(_, t)| t)
  }

  pub fn location_to_spans(
    &self,
    location: Location,
    span_type: EnclosingHirSpans,
  ) -> Vec<Span> {
    // special case for synthetic locations that represent arguments
    if location.block.as_usize() == self.body.basic_blocks().len() {
      return vec![];
    }

    let loc_span = match self.body.source_info(location).span.as_local(self.tcx) {
      Some(span) => span,
      None => {
        return vec![];
      }
    };
    if self.invalid_span(loc_span) {
      return vec![];
    }

    let is_return = matches!(
      self.body.stmt_at(location),
      Either::Right(Terminator {
        kind: TerminatorKind::Return
          | TerminatorKind::Resume
          | TerminatorKind::Drop { .. },
        ..
      })
    );
    if is_return {
      return vec![];
    }

    let mut enclosing_hir = Self::find_matching(
      |span| span.contains(loc_span),
      self.hir_spans.iter().map(|span| (span.full, span)),
    )
    .collect::<Vec<_>>();

    // Get the spans of the immediately enclosing HIR node
    assert!(
      !enclosing_hir.is_empty(),
      "Location {location:?} (span {loc_span:?}) had no enclosing HIR nodes"
    );
    let mut hir_spans = enclosing_hir.remove(0).get_spans(span_type);
    // trace!(
    //   "Initial hir node:\n{}\nhas spans: {:?}",
    //   node_to_string(node),
    //   hir_spans
    // );

    // Include the MIR span
    hir_spans.push(loc_span);

    // Add the spans of the first enclosing statement
    if let Some(hir_span) = enclosing_hir.iter().find(|span| span.node.is_right()) {
      // trace!(
      //   "Spans for stmt:\n{}\nare {:?}",
      //   stmt_to_string(hir_span.node.right().unwrap()),
      // );
      hir_spans.extend(hir_span.get_spans(span_type));
    }

    if let Either::Right(terminator) = self.body.stmt_at(location) {
      match terminator.kind {
        TerminatorKind::SwitchInt { .. } => {
          // If the location is a switch, then include the closest enclosing if or loop
          if let Some(spans) = enclosing_hir
            .iter()
            .filter_map(|span| {
              matches!(
                span.node.left()?.kind,
                ExprKind::If(..) | ExprKind::Loop(..)
              )
              .then(move || span.get_spans(span_type))
            })
            .next()
          {
            // trace!("Switch spans: {:?}", spans);
            hir_spans.extend(spans);
          }
        }
        _ => {}
      }
    }

    let format_spans = |spans: &[Span]| -> String {
      spans
        .iter()
        .map(|span| span.to_string(self.tcx))
        .collect::<Vec<_>>()
        .join(" -- ")
    };

    trace!(
      "Location {location:?} ({})\n  has loc span:\n  {}\n  and HIR spans:\n  {}",
      utils::location_to_string(location, self.body),
      format_spans(&[loc_span]),
      format_spans(&hir_spans)
    );

    hir_spans
  }

  pub fn span_to_places<'this>(
    &'this self,
    span: Span,
  ) -> Vec<&'this MirSpannedPlace<'tcx>> {
    // Note that MIR does not have granular source maps around projections.
    // So in the expression `let x = z.0`, the MIR Body only contains the place
    // z.0 with a span for the string "z.0". If the user selects only "z", there
    // is no way to determine map that selection back to a subset of the projection.
    //
    // At least, we can conservatively include the containing span "z.0" and slice on that.

    let spans = self
      .mir_spans
      .iter()
      .map(|mir_span| (mir_span.span, mir_span));
    let mut contained =
      Self::find_matching(|mir_span| span.contains(mir_span), spans.clone());
    let mut containing = Self::find_matching(|mir_span| mir_span.contains(span), spans);

    if contained.len() > 0 {
      let first = contained.next().unwrap();
      contained
        .take_while(|other| other.span.size() == first.span.size())
        .chain([first])
        .collect()
    } else if containing.len() > 0 {
      let first = containing.next().unwrap();
      containing
        .take_while(|other| other.span.size() == first.span.size())
        .chain([first])
        .collect()
    } else {
      Vec::new()
    }
  }
}

fn stmt_to_string(st: &Stmt) -> String {
  rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s.print_stmt(st))
}

fn expr_to_string(ex: &Expr) -> String {
  rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s.print_expr(ex))
}

#[allow(dead_code)]
fn node_to_string(node: HirNode) -> String {
  match node {
    Either::Left(ex) => expr_to_string(ex),
    Either::Right(st) => stmt_to_string(st),
  }
}

#[cfg(test)]
mod test {

  use rustc_data_structures::fx::FxHashSet as HashSet;
  use rustc_middle::mir::BasicBlock;
  use test_log::test;

  use super::*;
  use crate::test_utils;

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
      let spanner = Spanner::new(tcx, body_id, body);
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
      for (input_span, desired) in spans.into_iter().zip(expected.into_iter()) {
        let outputs = spanner.span_to_places(input_span);
        let snippets = outputs
          .into_iter()
          .map(|span| source_map.span_to_snippet(span.span).unwrap())
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

  fn hir_spanner_harness(src: &str, expected: &[&[&str]], span_type: EnclosingHirSpans) {
    harness(src, |tcx, body_id, body, spans| {
      let spanner = Spanner::new(tcx, body_id, body);
      let source_map = tcx.sess.source_map();
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let mut enclosing = Spanner::find_matching(
          |span| span.contains(input_span),
          spanner.hir_spans.iter().map(|span| (span.full, span)),
        )
        .collect::<Vec<_>>();
        let desired_set = desired.into_iter().copied().collect::<HashSet<_>>();
        let output_snippets = enclosing
          .remove(0)
          .get_spans(span_type)
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
  let mut x: i32 = 1;
  let y = x + 2;
  let w = if true {
    let z = 0;
    z
  } else {
    3
  };
  let z = &mut x; 
  *z = 4;
  let q = x
    .leading_ones()
    .trailing_zeros();
}"#;
    let (input, _ranges) = test_utils::parse_ranges(src, [("`(", ")`")]).unwrap();
    test_utils::compile_body(input, move |tcx, body_id, body_with_facts| {
      let source_map = tcx.sess.source_map();

      let spanner = Spanner::new(tcx, body_id, &body_with_facts.body);

      // These locations are just selected by inspecting the actual body, so this test might break
      // if the compiler is updated. Run with RUST_LOG=debug to see the body.
      let pairs: &[(_, &[&str])] = &[
        // Variable assignment
        ((0, 0), &["let mut x: i32 = ", "1", ";"]),
        // Expression RHS
        ((0, 3), &["let y = ", "x", ";"]),
        ((0, 4), &["let y = ", " + ", "x + 2", ";"]),
        // If expression
        ((1, 2), &["let w = ", "true", ";"]),
        ((1, 3), &[
          "let w = ",
          "if ",
          "true",
          " {\n    ",
          "\n    ",
          "\n  } else {\n    ",
          "\n  }",
          ";",
        ]),
        // Reference
        ((4, 1), &["let z = ", "&mut ", "&mut x", ";"]),
        // Reference assignment
        ((4, 3), &[" = ", ";", "*z = 4"]),
        // Method chain
        ((4, 4), &["let q = ", "x", ";"]),
        ((4, 5), &[
          "let q = ",
          "x\n    .leading_ones()",
          "\n    .leading_ones()",
          ";",
        ]),
        ((5, 0), &[
          "let q = ",
          "x\n    .leading_ones()\n    .trailing_zeros()",
          "\n    .trailing_zeros()",
          ";",
        ]),
      ];

      for ((i, j), outp) in pairs {
        let loc = Location {
          block: BasicBlock::from_usize(*i),
          statement_index: *j,
        };
        let spans = spanner.location_to_spans(loc, EnclosingHirSpans::OuterOnly);
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
