use anyhow::anyhow;
use flowistry::{
  block_timer,
  extensions::{EvalMode, EVAL_MODE},
  mir::borrowck_facts,
  timer::elapsed,
};
use fluid_let::fluid_set;
use log::{debug, info};

use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId, ForeignItem, ImplItem, Item, TraitItem,
};
use rustc_middle::{hir::map::Map, ty::TyCtxt};
use rustc_span::Span;
use std::{panic, time::Instant};

pub trait FlowistryOutput: Send + Sync + Default {
  fn merge(&mut self, other: Self);
}

#[derive(Debug)]
pub enum FlowistryError {
  BuildError,
  AnalysisError(String),
}

pub type FlowistryResult<T> = Result<T, FlowistryError>;

pub trait FlowistryAnalysis: Send + Sync + Sized {
  type Output: FlowistryOutput;

  fn locations(&self, tcx: TyCtxt) -> anyhow::Result<Vec<Span>>;
  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> anyhow::Result<Self::Output>;

  fn run(self, compiler_args: &[String]) -> FlowistryResult<Self::Output> {
    let mut compiler_args = compiler_args.to_vec();

    compiler_args.extend(
      "-Z identify-regions -Z mir-opt-level=0 -A warnings"
        .split(' ')
        .map(|s| s.to_owned()),
    );

    let mut callbacks = Callbacks {
      analysis: Some(self),
      output: None,
      rustc_start: Instant::now(),
      eval_mode: EVAL_MODE.copied(),
    };

    info!("Starting rustc analysis...");
    debug!("Eval mode: {:?}", callbacks.eval_mode);
    let compiler = rustc_driver::RunCompiler::new(&compiler_args, &mut callbacks);
    if compiler.run().is_err() {
      return Err(FlowistryError::BuildError);
    }

    callbacks
      .output
      .unwrap()
      .map_err(|e| FlowistryError::AnalysisError(e.to_string()))
  }
}

struct VisitorContext<'tcx, A: FlowistryAnalysis> {
  tcx: TyCtxt<'tcx>,
  analysis: A,
  locations: Vec<Span>,
  output: anyhow::Result<A::Output>,
}

impl<A> VisitorContext<'_, A>
where
  A: FlowistryAnalysis,
{
  fn analyze(&mut self, item_span: Span, body_id: BodyId) {
    if !self.locations.iter().any(|span| item_span.contains(*span)) || self.output.is_err() {
      return;
    }

    let tcx = self.tcx;
    let analysis = &mut self.analysis;

    let fn_name = tcx.def_path_debug_str(tcx.hir().body_owner_def_id(body_id).to_def_id());
    block_timer!(&format!("Flowistry ({})", fn_name));

    let output = panic::catch_unwind(panic::AssertUnwindSafe(move || {
      analysis.analyze_function(tcx, body_id)
    }))
    .unwrap_or_else(|panic_msg| {
      Err(match panic_msg.downcast_ref::<String>() {
        Some(msg) => anyhow!("{}", msg),
        None => anyhow!("Unknown panic"),
      })
    });

    match output {
      Ok(output) => self.output.as_mut().unwrap().merge(output),
      err => {
        self.output = err;
      }
    }
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
  output: Option<anyhow::Result<A::Output>>,
  rustc_start: Instant,
  eval_mode: Option<EvalMode>,
}

impl<A: FlowistryAnalysis> rustc_driver::Callbacks for Callbacks<A> {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  // TODO: does this need to be after_analysis? or can we do after_parsing
  //   after limited testing this seems to work fine... and is WAY faster
  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    elapsed("rustc", self.rustc_start);
    fluid_set!(EVAL_MODE, self.eval_mode.unwrap_or_default());

    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let analysis = self.analysis.take().unwrap();
      self.output = Some((|| {
        let locations = analysis.locations(tcx)?;
        let output = Ok(A::Output::default());
        let mut visitor = AnalysisVisitor(VisitorContext {
          tcx,
          locations,
          analysis,
          output,
        });

        tcx.hir().visit_all_item_likes(&mut visitor);
        visitor.0.output
      })());
    });

    rustc_driver::Compilation::Stop
  }
}
