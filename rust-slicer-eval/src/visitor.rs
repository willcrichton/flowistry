use log::debug;
use rust_slicer::config::{Config, EvalMode, Range, CONFIG};
use rustc_hir::{
  intravisit::{NestedVisitorMap, Visitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId, ImplItemKind, ItemKind, Local,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::Span;
use serde::Serialize;

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
  pub eval_results: Vec<EvalResult>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  mode: EvalMode,
  slice: Range,
  output: Vec<Range>,
}

impl EvalCrateVisitor<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>) -> Self {
    EvalCrateVisitor {
      tcx,
      eval_results: Vec::new(),
    }
  }

  fn analyze(&mut self, _body_span: Span, body_id: &BodyId) {
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

            CONFIG.set(config.clone(), || {
              let output =
                rust_slicer::analysis::intraprocedural::analyze_function(tcx, body_id, span)
                  .unwrap();
              EvalResult {
                mode: eval_mode,
                slice: config.range,
                output: output.ranges().to_vec(),
              }
            })
          })
      })
      .flatten()
      .collect::<Vec<_>>();

    self.eval_results.extend(eval_results.into_iter());
  }
}

impl<'hir, 'tcx> ItemLikeVisitor<'hir> for EvalCrateVisitor<'tcx> {
  fn visit_item(&mut self, item: &'hir rustc_hir::Item<'hir>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        debug!("Visiting function {}", item.ident);
        self.analyze(item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_impl_item(&mut self, impl_item: &'hir rustc_hir::ImplItem<'hir>) {
    match &impl_item.kind {
      ImplItemKind::Fn(_, body_id) => {
        debug!("Visiting impl function {}", impl_item.ident);
        self.analyze(impl_item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'hir rustc_hir::TraitItem<'hir>) {}

  fn visit_foreign_item(&mut self, _foreign_item: &'hir rustc_hir::ForeignItem<'hir>) {}
}
