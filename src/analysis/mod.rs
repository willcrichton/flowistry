use crate::config::{Config, CONFIG};
use anyhow::Result;
use rustc_hir::{itemlikevisit::ItemLikeVisitor, ForeignItem, ImplItem, Item, ItemKind, TraitItem};
use rustc_middle::ty::TyCtxt;

pub use intraprocedural::SliceOutput;

mod intraprocedural;
mod points_to;
mod relevance;

struct SliceVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  output: Result<SliceOutput>,
}

impl<'hir, 'tcx> ItemLikeVisitor<'hir> for SliceVisitor<'tcx> {
  fn visit_item(&mut self, item: &'hir Item<'hir>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        let tcx = self.tcx;
        take_mut::take(&mut self.output, move |output| {
          output.and_then(move |mut output| {
            let fn_output = intraprocedural::analyze_function(tcx, body_id)?;
            output.merge(fn_output);
            Ok(output)
          })
        });
      }
      _ => {}
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'hir TraitItem<'hir>) {}
  fn visit_impl_item(&mut self, _impl_item: &'hir ImplItem<'hir>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'hir ForeignItem<'hir>) {}
}

struct Callbacks {
  output: Option<Result<SliceOutput>>,
}

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let mut slice_visitor = SliceVisitor {
        tcx,
        output: Ok(SliceOutput::new()),
      };
      tcx.hir().krate().visit_all_item_likes(&mut slice_visitor);
      self.output = Some(slice_visitor.output);
    });

    rustc_driver::Compilation::Stop
  }
}

pub fn slice(config: Config, args: impl AsRef<str>) -> Result<SliceOutput> {
  env_logger::init();
  CONFIG.set(config).expect("Could not set config");

  // mir-opt-level ensures that mir_promoted doesn't apply optimizations
  let args = format!("{} -Z mir-opt-level=0", args.as_ref())
    .split(" ")
    .map(str::to_string)
    .collect::<Vec<_>>();

  let mut callbacks = Callbacks { output: None };
  rustc_driver::catch_fatal_errors(|| {
    rustc_driver::RunCompiler::new(&args, &mut callbacks)
      .run()
      .unwrap();
  })
  .unwrap();

  callbacks.output.unwrap()
}
