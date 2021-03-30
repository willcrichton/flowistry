use crate::config::Config;
use anyhow::{Error, Result};
use log::debug;
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId, ForeignItem, ImplItem, Item, TraitItem,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::{FileName, RealFileName, Span};
use std::time::Instant;

pub use intraprocedural::SliceOutput;

mod aliases;
pub mod intraprocedural;
mod place_index;
mod post_dominators;
mod relevance;

struct VisitorContext<'tcx> {
  tcx: TyCtxt<'tcx>,
  slice_span: Span,
  output: Result<SliceOutput>,
  config: Config,
}

impl VisitorContext<'_> {
  fn analyze(&mut self, body_span: Span, body_id: BodyId) {
    if !body_span.contains(self.slice_span) {
      return;
    }

    let tcx = self.tcx;
    let slice_span = self.slice_span;
    let config = &self.config;
    take_mut::take(&mut self.output, move |output| {
      output.and_then(move |mut output| {
        let start = Instant::now();
        let fn_output = intraprocedural::analyze_function(config, tcx, body_id, slice_span)?;
        debug!(
          "Finished in {} seconds",
          start.elapsed().as_nanos() as f64 / 1e9
        );
        output.merge(fn_output);
        Ok(output)
      })
    });
  }
}

struct SliceItemVisitor<'a, 'tcx>(&'a mut VisitorContext<'tcx>);

impl Visitor<'tcx> for SliceItemVisitor<'_, 'tcx> {
  type Map = Map<'tcx>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::OnlyBodies(self.0.tcx.hir())
  }

  fn visit_nested_body(&mut self, id: BodyId) {
    intravisit::walk_body(self, self.0.tcx.hir().body(id));
    self.0.analyze(self.0.tcx.hir().span(id.hir_id), id);
  }
}

struct SliceVisitor<'tcx>(VisitorContext<'tcx>);

impl ItemLikeVisitor<'tcx> for SliceVisitor<'tcx> {
  fn visit_item(&mut self, item: &'tcx Item<'tcx>) {
    let mut item_visitor = SliceItemVisitor(&mut self.0);
    item_visitor.visit_item(item);
  }

  fn visit_impl_item(&mut self, impl_item: &'tcx ImplItem<'tcx>) {
    let mut item_visitor = SliceItemVisitor(&mut self.0);
    item_visitor.visit_impl_item(impl_item);
  }

  fn visit_trait_item(&mut self, _trait_item: &'tcx TraitItem<'tcx>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'tcx ForeignItem<'tcx>) {}
}

struct Callbacks {
  config: Option<Config>,
  output: Option<Result<SliceOutput>>,
}

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let config = self.config.take().unwrap();

      let slice_span = {
        let source_map = tcx.sess.source_map();
        let files = source_map.files();
        let source_file = files
          .iter()
          .find(|file| {
            if let FileName::Real(RealFileName::Named(other_path)) = &file.name {
              config.range.filename == other_path.to_string_lossy()
            } else {
              false
            }
          })
          .expect(&format!(
            "Could not find file {} out of files {:#?}",
            config.range.filename, **files
          ));
        config.range.to_span(source_file)
      };

      let mut slice_visitor = SliceVisitor(VisitorContext {
        tcx,
        slice_span,
        config,
        output: Ok(SliceOutput::new()),
      });
      tcx.hir().krate().visit_all_item_likes(&mut slice_visitor);
      self.output = Some(slice_visitor.0.output);
    });

    rustc_driver::Compilation::Stop
  }
}

pub fn slice(config: Config, args: &[String]) -> Result<SliceOutput> {
  let mut args = args.to_vec();

  args.extend(
    "-Z identify-regions -A warnings"
      .split(" ")
      .map(|s| s.to_owned()),
  );

  let mut callbacks = Callbacks {
    config: Some(config),
    output: None,
  };

  rustc_driver::catch_fatal_errors(|| rustc_driver::RunCompiler::new(&args, &mut callbacks).run())
    .map_err(|_| Error::msg("rustc panicked"))?
    .map_err(|_| Error::msg("driver failed"))?;

  callbacks.output.unwrap()
}
