#![feature(
  rustc_private,
  in_band_lifetimes,
  unboxed_closures,
  box_patterns,
  trait_alias
)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return,
  clippy::len_zero,
  clippy::let_and_return
)]

use std::time::Instant;

use anyhow::Context;
use flowistry::{
  extensions::{EvalMode, EVAL_MODE},
  mir::borrowck_facts,
  range::ToSpan,
  source_map,
  timer::elapsed,
};
use fluid_let::fluid_set;
use log::{debug, info};
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_serialize::{json, Encodable};

extern crate either;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_serialize;
extern crate rustc_span;

#[cfg(decompose)]
pub mod decompose;
pub mod focus;
pub mod playground;
pub mod spans;

#[derive(Debug)]
pub enum FlowistryError {
  BuildError,
  AnalysisError(String),
}

pub type FlowistryResult<T> = Result<T, FlowistryError>;

pub trait JsonEncodable = for<'a> Encodable<json::Encoder<'a>>;

pub trait FlowistryAnalysis: Sized + Send + Sync {
  type Output: JsonEncodable + Send + Sync;
  fn analyze(&mut self, tcx: TyCtxt<'tcx>, id: BodyId) -> anyhow::Result<Self::Output>;
}

// Implement FlowistryAnalysis for all functions with a type signature that matches
// FlowistryAnalysis::analyze
impl<F, O> FlowistryAnalysis for F
where
  F: for<'tcx> Fn<(TyCtxt<'tcx>, BodyId), Output = anyhow::Result<O>> + Send + Sync,
  O: JsonEncodable + Send + Sync,
{
  type Output = O;
  fn analyze(&mut self, tcx: TyCtxt<'tcx>, id: BodyId) -> anyhow::Result<Self::Output> {
    (self)(tcx, id)
  }
}

pub fn run_with_callbacks(
  args: &[String],
  callbacks: &mut (dyn rustc_driver::Callbacks + Send),
) -> FlowistryResult<()> {
  let mut args = args.to_vec();
  args.extend(
    "-Z identify-regions -Z mir-opt-level=0 -A warnings"
      .split(' ')
      .map(|s| s.to_owned()),
  );

  let compiler = rustc_driver::RunCompiler::new(&args, callbacks);
  compiler.run().map_err(|_| FlowistryError::BuildError)
}

pub fn run<A: FlowistryAnalysis, T: ToSpan>(
  analysis: A,
  target: T,
  args: &[String],
) -> FlowistryResult<A::Output> {
  let mut callbacks = FlowistryCallbacks {
    analysis: Some(analysis),
    target,
    output: None,
    rustc_start: Instant::now(),
    eval_mode: EVAL_MODE.copied(),
  };

  info!("Starting rustc analysis...");
  debug!("Eval mode: {:?}", callbacks.eval_mode);

  run_with_callbacks(args, &mut callbacks)?;

  callbacks
    .output
    .unwrap()
    .map_err(|e| FlowistryError::AnalysisError(e.to_string()))
}

struct FlowistryCallbacks<A: FlowistryAnalysis, T: ToSpan> {
  analysis: Option<A>,
  target: T,
  output: Option<anyhow::Result<A::Output>>,
  rustc_start: Instant,
  eval_mode: Option<EvalMode>,
}

impl<A: FlowistryAnalysis, T: ToSpan> rustc_driver::Callbacks
  for FlowistryCallbacks<A, T>
{
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    elapsed("rustc", self.rustc_start);
    fluid_set!(EVAL_MODE, self.eval_mode.unwrap_or_default());

    let start = Instant::now();
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      elapsed("global_ctxt", start);
      let mut analysis = self.analysis.take().unwrap();
      self.output = Some((|| {
        let target = self.target.to_span(tcx)?;
        let mut bodies = source_map::find_enclosing_bodies(tcx, target);
        let body = bodies.next().context("Selection did not map to a body")?;
        analysis.analyze(tcx, body)
      })());
    });

    rustc_driver::Compilation::Stop
  }
}
