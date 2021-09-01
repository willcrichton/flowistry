#![allow(dead_code)]

use anyhow::{anyhow, Context, Result};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ItemLikeVisitor,
  Body, BodyId, Expr, Stmt,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::{FileName, RealFileName, SourceFile, Span};
use smallvec::SmallVec;
use std::{path::Path, rc::Rc};

pub fn qpath_to_span(tcx: TyCtxt, qpath: String) -> Result<Span> {
  struct Finder<'tcx> {
    tcx: TyCtxt<'tcx>,
    qpath: String,
    span: Option<Span>,
  }

  impl Visitor<'tcx> for Finder<'tcx> {
    type Map = Map<'tcx>;

    fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
      NestedVisitorMap::OnlyBodies(self.tcx.hir())
    }

    fn visit_nested_body(&mut self, id: BodyId) {
      intravisit::walk_body(self, self.tcx.hir().body(id));

      let local_def_id = self.tcx.hir().body_owner_def_id(id);
      let function_path = self
        .tcx
        .def_path(local_def_id.to_def_id())
        .to_string_no_crate_verbose();
      if &function_path[2..] == self.qpath {
        self.span = Some(self.tcx.hir().span(id.hir_id));
      }
    }
  }

  impl ItemLikeVisitor<'hir> for Finder<'tcx>
  where
    'hir: 'tcx,
  {
    fn visit_item(&mut self, item: &'hir rustc_hir::Item<'hir>) {
      <Self as Visitor<'tcx>>::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, impl_item: &'hir rustc_hir::ImplItem<'hir>) {
      <Self as Visitor<'tcx>>::visit_impl_item(self, impl_item);
    }

    fn visit_trait_item(&mut self, _trait_item: &'hir rustc_hir::TraitItem<'hir>) {}
    fn visit_foreign_item(&mut self, _foreign_item: &'hir rustc_hir::ForeignItem<'hir>) {}
  }

  let mut finder = Finder {
    tcx,
    qpath,
    span: None,
  };
  tcx.hir().krate().visit_all_item_likes(&mut finder);
  return finder
    .span
    .with_context(|| format!("No function with qpath {}", finder.qpath));
}

pub fn path_to_source_file<'tcx>(
  path: impl AsRef<str>,
  tcx: TyCtxt<'tcx>,
) -> Result<Rc<SourceFile>> {
  let source_map = tcx.sess.source_map();
  let files = source_map.files();
  let path = path.as_ref();
  let target_file = Path::new(&path).canonicalize().unwrap();
  files
    .iter()
    .find(|file| {
      if let FileName::Real(RealFileName::LocalPath(other_path)) = &file.name {
        target_file == other_path.canonicalize().unwrap()
      } else {
        false
      }
    })
    .map(|file| file.clone())
    .ok_or_else(|| anyhow!("Could not find file {} out of files {:#?}", path, **files))
}

pub struct HirSpanner {
  expr_spans: Vec<Span>,
  stmt_spans: Vec<Span>,
}

impl HirSpanner {
  pub fn new(body: &Body) -> Self {
    let mut spanner = HirSpanner {
      expr_spans: Vec::new(),
      stmt_spans: Vec::new(),
    };

    struct Collector<'a>(&'a mut HirSpanner);

    impl Visitor<'hir> for Collector<'_> {
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
    };

    find(&self.expr_spans)
      .into_iter()
      .chain(find(&self.stmt_spans).into_iter())
      .collect()
  }
}

pub fn expr_to_string(expr: &Expr) -> String {
  rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s.print_expr(expr))
}
