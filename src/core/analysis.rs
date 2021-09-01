use anyhow::Result;
use log::info;
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId, ForeignItem, ImplItem, Item, TraitItem,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::Span;
use std::time::Instant;

use crate::core::utils::elapsed;

use super::utils::block_timer;

pub trait FlowistryOutput: Send + Sync {
  fn empty() -> Self;
  fn merge(&mut self, other: Self);
}

pub trait FlowistryAnalysis: Send + Sync + Sized {
  type Output: FlowistryOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>>;
  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output>;

  fn run(self, compiler_args: &[String]) -> Result<Self::Output> {
    let mut compiler_args = compiler_args.to_vec();

    compiler_args.extend(
      "-Z identify-regions -Z polonius -Z mir-opt-level=0 -A warnings"
        .split(' ')
        .map(|s| s.to_owned()),
    );

    let mut callbacks = Callbacks {
      analysis: Some(self),
      output: None,
      rustc_start: Instant::now(),
    };

    info!("Starting rustc analysis...");
    rustc_driver::RunCompiler::new(&compiler_args, &mut callbacks)
      .run()
      .unwrap();

    callbacks.output.unwrap()
  }
}

struct VisitorContext<'tcx, A: FlowistryAnalysis> {
  tcx: TyCtxt<'tcx>,
  analysis: A,
  locations: Vec<Span>,
  output: Result<A::Output>,
}

impl<A> VisitorContext<'_, A>
where
  A: FlowistryAnalysis,
{
  fn analyze(&mut self, item_span: Span, body_id: BodyId) {
    if !self.locations.iter().any(|span| item_span.contains(*span)) {
      return;
    }

    let tcx = self.tcx;
    let analysis = &mut self.analysis;
    take_mut::take(&mut self.output, move |output| {
      output.and_then(move |mut output| {
        let fn_name = tcx.def_path_debug_str(tcx.hir().body_owner_def_id(body_id).to_def_id());
        let timer_name = format!("Flowistry ({})", fn_name);
        let _timer = block_timer(&timer_name);

        let new_output = analysis.analyze_function(tcx, body_id)?;
        output.merge(new_output);
        Ok(output)
      })
    });
  }
}

struct AnalysisItemVisitor<'a, 'tcx, A: FlowistryAnalysis>(&'a mut VisitorContext<'tcx, A>);

impl<A: FlowistryAnalysis> Visitor<'tcx> for AnalysisItemVisitor<'_, 'tcx, A> {
  type Map = Map<'tcx>;

  fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
    NestedVisitorMap::OnlyBodies(self.0.tcx.hir())
  }

  fn visit_nested_body(&mut self, id: BodyId) {
    let tcx = self.0.tcx;
    intravisit::walk_body(self, tcx.hir().body(id));
    let header_span = tcx.def_span(tcx.hir().body_owner_def_id(id));
    let body_span = tcx.hir().span(id.hir_id);
    let full_span = header_span.to(body_span);

    self.0.analyze(full_span, id);
  }
}

struct AnalysisVisitor<'tcx, A: FlowistryAnalysis>(VisitorContext<'tcx, A>);

impl<A: FlowistryAnalysis> ItemLikeVisitor<'tcx> for AnalysisVisitor<'tcx, A> {
  fn visit_item(&mut self, item: &'tcx Item<'tcx>) {
    let mut item_visitor = AnalysisItemVisitor(&mut self.0);
    item_visitor.visit_item(item);
  }

  fn visit_impl_item(&mut self, impl_item: &'tcx ImplItem<'tcx>) {
    let mut item_visitor = AnalysisItemVisitor(&mut self.0);
    item_visitor.visit_impl_item(impl_item);
  }

  fn visit_trait_item(&mut self, _trait_item: &'tcx TraitItem<'tcx>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'tcx ForeignItem<'tcx>) {}
}

struct Callbacks<A: FlowistryAnalysis> {
  analysis: Option<A>,
  output: Option<Result<A::Output>>,
  rustc_start: Instant,
}

impl<A: FlowistryAnalysis> rustc_driver::Callbacks for Callbacks<A> {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    elapsed("rustc", self.rustc_start);

    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let analysis = self.analysis.take().unwrap();
      let locations = analysis.locations(tcx).unwrap();
      let output = Ok(A::Output::empty());
      let mut visitor = AnalysisVisitor(VisitorContext {
        tcx,
        locations,
        analysis,
        output,
      });

      tcx.hir().krate().visit_all_item_likes(&mut visitor);
      self.output = Some(visitor.0.output);
    });

    rustc_driver::Compilation::Stop
  }
}
