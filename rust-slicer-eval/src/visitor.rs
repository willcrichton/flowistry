use itertools::iproduct;
use log::debug;
use rust_slicer::config::{Config, ContextMode, EvalMode, MutabilityMode, PointerMode, Range};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ParItemLikeVisitor,
  BodyId, Expr, ExprKind, ImplItemKind, ItemKind, Local,
};
use rustc_middle::{
  hir::map::Map,
  ty::{subst::GenericArgKind, TyCtxt, TypeckResults},
};
use rustc_span::Span;
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

struct EvalBodyVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  typeck_results: &'tcx TypeckResults<'tcx>,
  spans: Vec<Span>,
  contains_call_with_ref: bool,
}

impl Visitor<'tcx> for EvalBodyVisitor<'tcx> {
  type Map = Map<'tcx>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::OnlyBodies(self.tcx.hir())
  }

  fn visit_local(&mut self, local: &'tcx Local<'tcx>) {
    intravisit::walk_local(self, local);
    self.spans.push(local.span);
  }

  fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
    intravisit::walk_expr(self, ex);

    match ex.kind {
      ExprKind::Assign(_, _, _) | ExprKind::AssignOp(_, _, _) => {
        self.spans.push(ex.span);
      }
      ExprKind::Call(_, args) | ExprKind::MethodCall(_, _, args, _) => {
        let arg_contains_ref = args.iter().any(|expr| {
          let ty = self.typeck_results.expr_ty(expr);
          ty.walk().any(|ty_piece| match ty_piece.unpack() {
            GenericArgKind::Lifetime(_) => true,
            _ => false,
          })
        });

        if arg_contains_ref {
          self.contains_call_with_ref = true;
        }
      }
      _ => {}
    }
  }
}

pub struct EvalCrateVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  pub eval_results: Mutex<Vec<EvalResult>>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  mutability_mode: MutabilityMode,
  context_mode: ContextMode,
  pointer_mode: PointerMode,
  slice: Range,
  function_range: Range,
  function_path: String,
  output: Vec<Range>,
  duration: f64,
  contains_call_with_ref: bool,
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
      typeck_results: self.tcx.typeck_body(*body_id),
      contains_call_with_ref: false,
    };
    body_visitor.visit_expr(&body.value);
    let contains_call_with_ref = body_visitor.contains_call_with_ref;

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
          vec![MutabilityMode::DistinguishMut, MutabilityMode::IgnoreMut].into_iter(),
          vec![ContextMode::Recurse, ContextMode::SigOnly].into_iter(),
          vec![PointerMode::Precise, PointerMode::Conservative].into_iter()
        )
        .filter_map(move |(mutability_mode, context_mode, pointer_mode)| {
          let config = Config {
            range: Range::from_span(span, source_map).ok()?,
            debug: false,
            eval_mode: EvalMode {
              mutability_mode,
              context_mode,
              pointer_mode,
            },
          };

          let start = Instant::now();
          let (output, _) = rust_slicer::analysis::intraprocedural::analyze_function(
            &config,
            tcx,
            *body_id,
            Some(span),
            Vec::new(),
          )
          .unwrap();

          Some(EvalResult {
            context_mode,
            mutability_mode,
            pointer_mode,
            contains_call_with_ref,
            slice: config.range,
            function_range: Range::from_span(body_span, source_map).ok()?,
            function_path: function_path.clone(),
            output: output.ranges().to_vec(),
            duration: (start.elapsed().as_nanos() as f64) / 10e9,
          })
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
