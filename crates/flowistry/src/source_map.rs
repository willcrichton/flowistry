//! Mapping source ranges to/from the HIR and MIR.

use std::rc::Rc;

use either::Either;
use log::trace;
use rustc_hir::{
  self as hir,
  intravisit::{self, Visitor as HirVisitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId, Expr, ExprKind, ForeignItem, ImplItem, Item, MatchSource, Node, Param, Stmt,
  TraitItem,
};
use rustc_middle::{
  hir::nested_filter::OnlyBodies,
  mir::{
    self,
    visit::{
      MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext,
      Visitor as MirVisitor,
    },
    Body, HasLocalDecls, TerminatorKind,
  },
  ty::TyCtxt,
};
use rustc_span::{Span, SpanData};

use crate::{
  block_timer,
  indexed::impls::LocationDomain,
  mir::utils::{self, SpanDataExt, SpanExt},
};

// Collect all the spans for children beneath the visited node.
// For example, when visiting "if true { 1 } else { 2 }" then we
// should collect: "true" "1" "2"
struct ChildExprSpans<'tcx> {
  spans: Vec<Span>,
  tcx: TyCtxt<'tcx>,
}
impl HirVisitor<'hir> for ChildExprSpans<'_> {
  fn visit_expr(&mut self, ex: &hir::Expr) {
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

  fn visit_stmt(&mut self, stmt: &hir::Stmt) {
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
  full: SpanData,
  outer: Vec<Span>,
  node: hir::Node<'hir>,
}

impl HirSpannedNode<'_> {
  fn get_spans(&self, span_type: EnclosingHirSpans) -> Vec<Span> {
    match span_type {
      EnclosingHirSpans::OuterOnly => self.outer.clone(),
      EnclosingHirSpans::Full => vec![self.full.span()],
    }
  }
}

struct HirSpanCollector<'a, 'b, 'hir, 'tcx>(&'a mut Spanner<'b, 'hir, 'tcx>);

macro_rules! try_span {
  ($self:expr, $span:expr) => {
    match $span.as_local($self.0.tcx) {
      Some(span) if !$self.0.invalid_span(span) => span,
      _ => {
        return;
      }
    }
  };
}

impl HirVisitor<'hir> for HirSpanCollector<'_, '_, 'hir, '_> {
  fn visit_expr(&mut self, expr: &'hir hir::Expr<'hir>) {
    intravisit::walk_expr(self, expr);

    let span = try_span!(self, expr.span);

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
      full: span.data(),
      outer: outer_spans,
      node: Node::Expr(expr),
    });
  }

  fn visit_stmt(&mut self, stmt: &'hir Stmt<'hir>) {
    intravisit::walk_stmt(self, stmt);

    let span = try_span!(self, stmt.span);

    let mut visitor = ChildExprSpans {
      spans: Vec::new(),
      tcx: self.0.tcx,
    };
    intravisit::walk_stmt(&mut visitor, stmt);
    let outer_spans = span.subtract(visitor.spans);

    self.0.hir_spans.push(HirSpannedNode {
      full: span.data(),
      outer: outer_spans,
      node: Node::Stmt(stmt),
    });
  }

  fn visit_param(&mut self, param: &'hir Param<'hir>) {
    intravisit::walk_param(self, param);

    let span = match param.span.as_local(self.0.tcx) {
      Some(span) if !self.0.invalid_span(span) => span,
      _ => {
        return;
      }
    };

    // TODO: more precise outer spans
    self.0.hir_spans.push(HirSpannedNode {
      full: span.data(),
      outer: vec![span],
      node: Node::Param(param),
    });
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirSpannedPlace<'tcx> {
  pub place: mir::Place<'tcx>,
  pub span: SpanData,
  pub location: mir::Location,
}

struct MirSpanCollector<'a, 'b, 'hir, 'tcx>(&'a mut Spanner<'b, 'hir, 'tcx>);

impl MirVisitor<'tcx> for MirSpanCollector<'_, '_, '_, 'tcx> {
  fn visit_place(
    &mut self,
    place: &mir::Place<'tcx>,
    context: PlaceContext,
    location: mir::Location,
  ) {
    trace!("place={place:?} context={context:?} location={location:?}");
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

    let span = try_span!(self, span);

    let spanned_place = MirSpannedPlace {
      place: *place,
      location,
      span: span.data(),
    };
    trace!("spanned place: {spanned_place:?}");

    self.0.mir_spans.push(spanned_place);
  }

  // The visit_statement and visit_terminator cases are a backup.
  // Eg in the static_method case, if you have x = Foo::bar(), then
  // then a slice on Foo::bar() would correspond to no places. The best we
  // can do is at least make the slice on x.
  fn visit_statement(
    &mut self,
    statement: &mir::Statement<'tcx>,
    location: mir::Location,
  ) {
    self.super_statement(statement, location);

    if let mir::StatementKind::Assign(box (lhs, _)) = &statement.kind {
      let span = try_span!(self, statement.source_info.span);
      let spanned_place = MirSpannedPlace {
        place: *lhs,
        location,
        span: span.data(),
      };
      trace!("spanned place (assign): {spanned_place:?}");
      self.0.mir_spans.push(spanned_place);
    }
  }

  fn visit_terminator(
    &mut self,
    terminator: &mir::Terminator<'tcx>,
    location: mir::Location,
  ) {
    self.super_terminator(terminator, location);

    let place = match &terminator.kind {
      mir::TerminatorKind::Call {
        destination: Some((place, _)),
        ..
      } => *place,
      mir::TerminatorKind::DropAndReplace { place, .. } => *place,
      _ => {
        return;
      }
    };

    let span = try_span!(self, terminator.source_info.span);
    let spanned_place = MirSpannedPlace {
      place,
      location,
      span: span.data(),
    };
    trace!("spanned place (terminator): {spanned_place:?}");
    self.0.mir_spans.push(spanned_place);
  }
}

pub struct Spanner<'a, 'hir, 'tcx> {
  pub hir_spans: Vec<HirSpannedNode<'hir>>,
  pub mir_spans: Vec<MirSpannedPlace<'tcx>>,
  pub body_span: Span,
  pub item_span: Span,
  tcx: TyCtxt<'tcx>,
  body: &'a mir::Body<'tcx>,
}

impl Spanner<'a, 'hir, 'tcx>
where
  'tcx: 'hir,
{
  pub fn new(tcx: TyCtxt<'tcx>, body_id: BodyId, body: &'a Body<'tcx>) -> Self {
    let hir = tcx.hir();
    let hir_body = hir.body(body_id);
    let owner = hir.body_owner(body_id);
    let item_span = hir.span_with_body(owner);

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
    predicate: impl Fn(SpanData) -> bool,
    spans: impl Iterator<Item = (SpanData, T)>,
  ) -> impl ExactSizeIterator<Item = T> {
    let mut matching = spans
      .filter(|(span, _)| predicate(*span))
      .collect::<Vec<_>>();
    matching.sort_by_key(|(span, _)| span.size());
    matching.into_iter().map(|(_, t)| t)
  }

  pub fn location_to_spans(
    &self,
    location: mir::Location,
    location_domain: &Rc<LocationDomain>,
    span_type: EnclosingHirSpans,
  ) -> Vec<Span> {
    let (target_span, stmt) = match location_domain.location_to_local(location) {
      Some(local) => (self.body.local_decls[local].source_info.span, None),
      None => (
        self.body.source_info(location).span,
        Some(self.body.stmt_at(location)),
      ),
    };

    let target_span = match target_span.as_local(self.tcx) {
      Some(span) if !self.invalid_span(span) => span,
      _ => {
        return vec![];
      }
    };

    let target_span_data = target_span.data();
    let mut enclosing_hir = Self::find_matching(
      |span| span.contains(target_span_data),
      self.hir_spans.iter().map(|span| (span.full, span)),
    )
    .collect::<Vec<_>>();
    trace!("enclosing_hir={enclosing_hir:#?}");

    // Get the spans of the immediately enclosing HIR node
    assert!(
      !enclosing_hir.is_empty(),
      "Location {location:?} (span {target_span:?}) had no enclosing HIR nodes"
    );
    let mut hir_spans = enclosing_hir.remove(0).get_spans(span_type);

    // Include the MIR span
    hir_spans.push(target_span);

    macro_rules! add_first_where {
      ($f:expr) => {
        if let Some(node) = enclosing_hir.iter().find($f) {
          hir_spans.extend(node.get_spans(span_type));
        }
      };
    }

    // Add the spans of the first enclosing statement
    add_first_where!(|node| matches!(node.node, Node::Stmt(_)));

    // Include `return` keyword if the location is an expression under a return.
    add_first_where!(|node| {
      matches!(
        node.node,
        Node::Expr(hir::Expr {
          kind: hir::ExprKind::Ret(_),
          ..
        })
      )
    });

    if let Some(Either::Right(mir::Terminator {
      kind: TerminatorKind::SwitchInt { .. },
      ..
    })) = stmt
    {
      // If the location is a switch, then include the closest enclosing if or loop
      add_first_where!(|node| {
        matches!(
          node.node,
          Node::Expr(hir::Expr {
            kind: ExprKind::If(..) | ExprKind::Loop(..),
            ..
          })
        )
      });
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
      format_spans(&[target_span]),
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
    let span_data = span.data();
    let mut contained =
      Self::find_matching(move |mir_span| span_data.contains(mir_span), spans.clone());
    let mut containing =
      Self::find_matching(move |mir_span| mir_span.contains(span_data), spans);

    let mut vec = if let Some(first) = contained.next() {
      contained
        .take_while(|other| other.span.size() == first.span.size())
        .chain([first])
        .collect()
    } else if let Some(first) = containing.next() {
      containing
        .take_while(|other| other.span.size() == first.span.size())
        .chain([first])
        .collect()
    } else {
      Vec::new()
    };
    vec.dedup();
    vec
  }
}

fn expr_to_string(ex: &Expr) -> String {
  rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s.print_expr(ex))
}

struct BodyFinder<'tcx> {
  tcx: TyCtxt<'tcx>,
  bodies: Vec<(Span, BodyId)>,
}

impl HirVisitor<'tcx> for BodyFinder<'tcx> {
  type NestedFilter = OnlyBodies;

  fn nested_visit_map(&mut self) -> Self::Map {
    self.tcx.hir()
  }

  fn visit_nested_body(&mut self, id: BodyId) {
    let hir = self.nested_visit_map();

    // const/static items are considered to have bodies, so we want to exclude
    // them from our search for functions
    if !hir.body_owner_kind(hir.body_owner(id)).is_fn_or_closure() {
      return;
    }

    let body = hir.body(id);
    self.visit_body(body);

    let hir = self.tcx.hir();
    let span = hir.span_with_body(hir.body_owner(id));
    trace!(
      "Searching body for {:?} with span {span:?} (local {:?})",
      self
        .tcx
        .def_path_debug_str(hir.body_owner_def_id(id).to_def_id()),
      span.as_local(self.tcx)
    );

    if !span.from_expansion() {
      self.bodies.push((span, id));
    }
  }
}

impl ItemLikeVisitor<'tcx> for BodyFinder<'tcx> {
  fn visit_item(&mut self, item: &'tcx Item<'tcx>) {
    intravisit::walk_item(self, item);
  }

  fn visit_impl_item(&mut self, impl_item: &'tcx ImplItem<'tcx>) {
    intravisit::walk_impl_item(self, impl_item);
  }

  fn visit_trait_item(&mut self, _trait_item: &'tcx TraitItem<'tcx>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'tcx ForeignItem<'tcx>) {}
}

pub fn find_bodies(tcx: TyCtxt<'tcx>) -> Vec<(Span, BodyId)> {
  block_timer!("find_bodies");
  let mut finder = BodyFinder {
    tcx,
    bodies: Vec::new(),
  };
  tcx.hir().visit_all_item_likes(&mut finder);
  finder.bodies
}

pub fn find_enclosing_bodies(
  tcx: TyCtxt<'tcx>,
  sp: Span,
) -> impl Iterator<Item = BodyId> {
  let mut bodies = find_bodies(tcx);
  bodies.retain(|(other, _)| other.contains(sp));
  bodies.sort_by_key(|(span, _)| span.size());
  bodies.into_iter().map(|(_, id)| id)
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
          .map(|spanned| source_map.span_to_snippet(spanned.span.span()).unwrap())
          .collect::<HashSet<_>>();

        println!("input_span={input_span:?}");
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
          |span| span.contains(input_span.data()),
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
      let location_domain = LocationDomain::new(&body_with_facts.body);
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
        let loc = mir::Location {
          block: BasicBlock::from_usize(*i),
          statement_index: *j,
        };
        let spans =
          spanner.location_to_spans(loc, &location_domain, EnclosingHirSpans::OuterOnly);
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
