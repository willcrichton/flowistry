use itertools::iproduct;
use log::debug;
use rust_slicer::config::{BorrowMode, Config, ContextMode, EvalMode, Range};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ParItemLikeVisitor,
  BodyId, Expr, ExprKind, ImplItemKind, ItemKind, Local,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::Span;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

struct EvalBodyVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  spans: Vec<Span>,
}

impl Visitor<'tcx> for EvalBodyVisitor<'tcx> {
  type Map = Map<'tcx>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::OnlyBodies(self.tcx.hir())
  }

  fn visit_local(&mut self, local: &'tcx Local<'tcx>) {
    self.spans.push(local.span);
  }

  fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
    match ex.kind {
      ExprKind::Assign(_, _, _) | ExprKind::AssignOp(_, _, _) => {
        self.spans.push(ex.span);
      }
      _ => {
        intravisit::walk_expr(self, ex);
      }
    }
  }
}

pub struct EvalCrateVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  pub eval_results: Mutex<Vec<EvalResult>>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  borrow_mode: BorrowMode,
  context_mode: ContextMode,
  slice: Range,
  function_range: Range,
  function_path: String,
  output: Vec<Range>,
  duration: f64,
}

impl EvalCrateVisitor<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>) -> Self {
    EvalCrateVisitor {
      tcx,
      eval_results: Mutex::new(Vec::new()),
    }
  }

  fn analyze(&self, body_span: Span, body_id: &BodyId) {
    let body = self.tcx.hir().body(*body_id);

    let mut body_visitor = EvalBodyVisitor {
      tcx: self.tcx,
      spans: Vec::new(),
    };
    body_visitor.visit_expr(&body.value);

    let def_id = self.tcx.hir().body_owner_def_id(*body_id).to_def_id();
    let function_path = &self.tcx.def_path_debug_str(def_id);
    debug!("Visiting {}", function_path);

    let eval_results = body_visitor
      .spans
      .into_iter()
      .map(|span| {
        let source_map = self.tcx.sess.source_map();
        let tcx = self.tcx;

        iproduct!(
          vec![BorrowMode::DistinguishMut, BorrowMode::IgnoreMut].into_iter(),
          vec![ContextMode::Recurse, ContextMode::SigOnly].into_iter()
        )
        .map(move |(borrow_mode, context_mode)| {
          let config = Config {
            range: Range::from_span(span, source_map),
            debug: false,
            eval_mode: EvalMode {
              borrow_mode,
              context_mode,
            },
          };

          let start = Instant::now();
          let (output, _) = rust_slicer::analysis::intraprocedural::analyze_function(
            &config,
            tcx,
            *body_id,
            Some(span),
            Vec::new()
          )
          .unwrap();

          EvalResult {
            context_mode,
            borrow_mode,
            slice: config.range,
            function_range: Range::from_span(body_span, source_map),
            function_path: function_path.clone(),
            output: output.ranges().to_vec(),
            duration: (start.elapsed().as_nanos() as f64) / 10e9,
          }
        })
      })
      .flatten()
      .collect::<Vec<_>>();

    self
      .eval_results
      .lock()
      .unwrap()
      .extend(eval_results.into_iter());
  }
}

impl ParItemLikeVisitor<'tcx> for EvalCrateVisitor<'tcx> {
  fn visit_item(&self, item: &'tcx rustc_hir::Item<'tcx>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        self.analyze(item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_impl_item(&self, impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
    match &impl_item.kind {
      ImplItemKind::Fn(_, body_id) => {
        self.analyze(impl_item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {}

  fn visit_foreign_item(&self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {}
}
