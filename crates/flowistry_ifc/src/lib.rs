#![feature(rustc_private, in_band_lifetimes)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_span;
extern crate rustc_trait_selection;
extern crate rustc_traits;

mod analysis;

use flowistry::{infoflow, mir::borrowck_facts};
use rustc_hir::{itemlikevisit::ItemLikeVisitor, ImplItemKind, ItemKind};
use rustc_middle::ty::TyCtxt;

pub struct Visitor<'tcx> {
  tcx: TyCtxt<'tcx>,
}

impl Visitor<'tcx> {
  fn analyze(&mut self, body_id: &rustc_hir::BodyId) {
    let tcx = self.tcx;
    let local_def_id = tcx.hir().body_owner_def_id(*body_id);
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, local_def_id);
    let flow = &infoflow::compute_flow(tcx, *body_id, body_with_facts);
    analysis::analyze(body_id, flow).unwrap();
  }
}

impl ItemLikeVisitor<'tcx> for Visitor<'tcx> {
  fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
    if let ItemKind::Fn(_, _, body_id) = &item.kind {
      self.analyze(body_id);
    }
  }

  fn visit_impl_item(&mut self, impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
    if let ImplItemKind::Fn(_, body_id) = &impl_item.kind {
      self.analyze(body_id);
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {}

  fn visit_foreign_item(&mut self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {}
}

pub struct Callbacks;
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let mut visitor = Visitor { tcx };
      tcx.hir().visit_all_item_likes(&mut visitor);
    });

    rustc_driver::Compilation::Stop
  }
}
