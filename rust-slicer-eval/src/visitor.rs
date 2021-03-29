use log::debug;
use rust_slicer::config::{Config, EvalMode, Range};
use rustc_hir::{
  intravisit::{NestedVisitorMap, Visitor},
  itemlikevisit::ParItemLikeVisitor,
  BodyId, ImplItemKind, ItemKind, Local,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::Span;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

struct EvalBodyVisitor {
  spans: Vec<Span>,
}

impl Visitor<'v> for EvalBodyVisitor {
  type Map = Map<'v>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::None
  }

  fn visit_local(&mut self, local: &'v Local<'v>) {
    self.spans.push(local.span);
  }
}

pub struct EvalCrateVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  pub eval_results: Mutex<Vec<EvalResult>>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  mode: EvalMode,
  slice: Range,
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

  fn analyze(&self, _body_span: Span, body_id: &BodyId) {
    let body = self.tcx.hir().body(*body_id);

    let mut body_visitor = EvalBodyVisitor { spans: Vec::new() };
    body_visitor.visit_expr(&body.value);

    let eval_results = body_visitor
      .spans
      .into_iter()
      .map(|span| {
        let source_map = self.tcx.sess.source_map();
        let tcx = self.tcx;
        [EvalMode::Standard, EvalMode::LikeC]
          .iter()
          .cloned()
          .map(move |eval_mode| {
            let config = Config {
              range: Range::from_span(span, source_map),
              debug: false,
              eval_mode,
            };

            let start = Instant::now();
            let output =
              rust_slicer::analysis::intraprocedural::analyze_function(&config, tcx, body_id, span)
                .unwrap();
            EvalResult {
              mode: eval_mode,
              slice: config.range,
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

impl<'hir, 'tcx> ParItemLikeVisitor<'hir> for EvalCrateVisitor<'tcx> {
  fn visit_item(&self, item: &'hir rustc_hir::Item<'hir>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        debug!("Visiting function {}", item.ident);
        self.analyze(item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_impl_item(&self, impl_item: &'hir rustc_hir::ImplItem<'hir>) {
    match &impl_item.kind {
      ImplItemKind::Fn(_, body_id) => {
        debug!("Visiting impl function {}", impl_item.ident);
        self.analyze(impl_item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&self, _trait_item: &'hir rustc_hir::TraitItem<'hir>) {}

  fn visit_foreign_item(&self, _foreign_item: &'hir rustc_hir::ForeignItem<'hir>) {}
}
