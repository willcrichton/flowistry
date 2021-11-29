use crate::mir::utils;
use log::{debug, trace};
use rustc_data_structures::{fx::FxHashSet as HashSet, graph::iterate::reverse_post_order};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor as HirVisitor},
  BodyId, Expr, Stmt,
};
use rustc_index::bit_set::HybridBitSet;
use rustc_middle::{
  hir::map::Map,
  mir::{
    visit::{
      MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext, Visitor as MirVisitor,
    },
    *,
  },
  ty::TyCtxt,
};
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

    impl HirVisitor<'hir> for Collector<'_> {
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

pub fn span_to_place(body: &Body<'tcx>, span: Span) -> Option<(Place<'tcx>, Location, Span)> {
  struct FindSpannedPlaces<'a, 'tcx> {
    body: &'a Body<'tcx>,
    span: Span,
    places: HashSet<(Place<'tcx>, Location, Span)>,
  }

  impl MirVisitor<'tcx> for FindSpannedPlaces<'_, 'tcx> {
    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
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

      if span.from_expansion() {
        return;
      }

      if self.span.contains(span) {
        self.places.insert((*place, location, span));
      }
    }
  }

  let mut visitor = FindSpannedPlaces {
    body,
    span,
    places: HashSet::default(),
  };
  visitor.visit_body(body);

  debug!("Spanned places: {:#?}", visitor.places);
  visitor
    .places
    .into_iter()
    .max_by_key(|(_, _, span)| span.hi() - span.lo())
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::test_utils;

  fn harness(src: &str, f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &Body, Vec<Span>) + Send) {
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
    }"#;
    harness(src, |tcx, _, body, spans| {
      let source_map = tcx.sess.source_map();
      let expected = ["z", "x", "x + y", "x", "x", "y"];
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let (_, _, output_span) = span_to_place(body, input_span).unwrap();
        let snippet = source_map.span_to_snippet(output_span).unwrap();
        assert_eq!(snippet, desired);
      }
    });
  }

  #[test]
  fn test_hir_spanner() {
    let src = r#"fn foo(){
      let x = `(1)`;
      let `(y)` = x + 1;      
    }"#;
    harness(src, |tcx, body_id, _, spans| {
      let spanner = HirSpanner::new(tcx, body_id);
      let source_map = tcx.sess.source_map();
      let expected: &[&[&str]] = &[&["1", "let x = 1;"], &["let y = x + 1;"]];
      for (input_span, desired) in spans.into_iter().zip(expected) {
        let output_spans = spanner.find_enclosing_hir_span(input_span);
        let mut desired_set = desired.into_iter().copied().collect::<HashSet<_>>();
        for output_span in &output_spans {
          let snippet = source_map.span_to_snippet(*output_span).unwrap();
          assert!(
            desired_set.remove(snippet.as_str()),
            "desired {:?} / actual {:?}",
            desired,
            output_spans
          );
        }
        assert!(
          desired_set.is_empty(),
          "desired {:?} / actual {:?}",
          desired,
          output_spans
        );
      }
    });
  }

  #[test]
  fn test_location_to_places() {
    let src = r#"fn foo(){
      let x = 1;
      let y = x + 1;      
    }"#;
    let (input, _ranges) = test_utils::parse_ranges(src, [("`(", ")`")]).unwrap();
    test_utils::compile_body(input, move |tcx, body_id, body_with_facts| {
      let source_map = tcx.sess.source_map();
      let _snippet = |sp| source_map.span_to_snippet(sp).unwrap();

      let spanner = HirSpanner::new(tcx, body_id);
      let location = Location::START;
      let _spans = location_to_spans(location, &body_with_facts.body, &spanner, source_map);
      // TODO: finish these tests
    });
  }
}
