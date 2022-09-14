use std::{
  env,
  path::PathBuf,
  process::{exit, Command},
  time::Instant,
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use flowistry::{
  extensions::{ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE},
  mir::borrowck_facts,
  source_map::{self, FunctionIdentifier, GraphemeIndices, Range, ToSpan},
  timer::elapsed,
};
use fluid_let::fluid_set;
use log::{debug, info};
use rustc_hir::BodyId;
use rustc_interface::interface::Result as RustcResult;
use rustc_middle::ty::TyCtxt;
use rustc_plugin::{RustcPlugin, RustcPluginArgs, Utf8Path};
use serde::{Deserialize, Serialize};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Serialize, Deserialize)]
#[clap(version = VERSION)]
pub struct FlowistryPluginArgs {
  #[clap(long)]
  bench: Option<bool>,

  #[clap(long)]
  context_mode: Option<ContextMode>,
  #[clap(long)]
  mutability_mode: Option<MutabilityMode>,
  #[clap(long)]
  pointer_mode: Option<PointerMode>,

  #[clap(subcommand)]
  command: FlowistryCommand,
}

#[derive(Subcommand, Serialize, Deserialize)]
enum FlowistryCommand {
  Spans {
    file: String,

    #[clap(last = true)]
    flags: Vec<String>,
  },

  Focus {
    file: String,
    pos: usize,

    #[clap(last = true)]
    flags: Vec<String>,
  },

  Decompose {
    file: String,
    pos: usize,

    #[clap(last = true)]
    flags: Vec<String>,
  },

  Playground {
    file: String,
    start: usize,
    end: usize,

    #[clap(last = true)]
    flags: Vec<String>,
  },

  Preload,

  RustcVersion,
}

pub struct FlowistryPlugin;
impl RustcPlugin for FlowistryPlugin {
  type Args = FlowistryPluginArgs;

  fn bin_name() -> String {
    "flowistry-driver".into()
  }

  fn args(&self, target_dir: &Utf8Path) -> RustcPluginArgs<FlowistryPluginArgs> {
    let args = FlowistryPluginArgs::parse_from(env::args().skip(1));

    let cargo_path = env::var("CARGO_PATH").unwrap_or_else(|_| "cargo".to_string());

    use FlowistryCommand::*;
    match &args.command {
      Preload => {
        let mut cmd = Command::new(cargo_path);
        // Note: this command must share certain parameters with rustc_plugin so Cargo will not recompute
        // dependencies when actually running the driver, e.g. RUSTFLAGS.
        cmd
          .args(&["check", "--all", "--all-features", "--target-dir"])
          .arg(target_dir);
        let exit_status = cmd.status().expect("could not run cargo");
        exit(exit_status.code().unwrap_or(-1));
      }
      RustcVersion => {
        let commit_hash = rustc_interface::util::commit_hash_str().unwrap_or("unknown");
        println!("{commit_hash}");
        exit(0);
      }
      _ => {}
    };

    let (file, flags) = match &args.command {
      Spans { file, flags } => (file, flags),
      Focus { file, flags, .. } => (file, flags),
      Decompose { file, flags, .. } => (file, flags),
      Playground { file, flags, .. } => (file, flags),
      _ => unreachable!(),
    };

    RustcPluginArgs {
      flags: Some(flags.clone()),
      file: Some(PathBuf::from(file)),
      args,
    }
  }

  fn run(
    self,
    compiler_args: Vec<String>,
    plugin_args: FlowistryPluginArgs,
  ) -> RustcResult<()> {
    let eval_mode = EvalMode {
      context_mode: plugin_args.context_mode.unwrap_or(ContextMode::SigOnly),
      mutability_mode: plugin_args
        .mutability_mode
        .unwrap_or(MutabilityMode::DistinguishMut),
      pointer_mode: plugin_args.pointer_mode.unwrap_or(PointerMode::Precise),
    };
    fluid_set!(EVAL_MODE, eval_mode);

    use FlowistryCommand::*;
    match plugin_args.command {
      Spans { file, .. } => postprocess(crate::spans::spans(&compiler_args, file)),
      Playground {
        file, start, end, ..
      } => {
        let indices = GraphemeIndices::from_path(&file).unwrap();
        let range = Range::from_char_range(start, end, &file, &indices);
        postprocess(run(crate::playground::playground, range, &compiler_args))
      }
      Focus { file, pos, .. } => {
        let indices = GraphemeIndices::from_path(&file).unwrap();
        let id =
          FunctionIdentifier::Range(Range::from_char_range(pos, pos, &file, &indices));
        postprocess(run(crate::focus::focus, id, &compiler_args))
      }
      Decompose {
        file: _file,
        pos: _pos,
        ..
      } => {
        cfg_if::cfg_if! {
          if #[cfg(feature = "decompose")] {
            let indices = GraphemeIndices::from_path(&_file).unwrap();
            let id =
              FunctionIdentifier::Range(Range::from_char_range(_pos, _pos, &_file, &indices));
            postprocess(run(
              crate::decompose::decompose,
              id,
              &compiler_args,
            ))
          } else {
            panic!("Flowistry must be built with the decompose feature")
          }
        }
      }
      _ => unreachable!(),
    }
  }
}

fn postprocess<T: Serialize>(result: FlowistryResult<T>) -> RustcResult<()> {
  let result = match result {
    Ok(output) => Ok(output),
    Err(e) => match e {
      FlowistryError::BuildError => {
        return Err(rustc_errors::ErrorGuaranteed::unchecked_claim_error_was_emitted());
      }
      FlowistryError::AnalysisError(msg) => Err(msg),
    },
  };

  println!("{}", serde_json::to_string(&result).unwrap());

  Ok(())
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

fn run<A: FlowistryAnalysis, T: ToSpan>(
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

#[derive(Debug)]
pub enum FlowistryError {
  BuildError,
  AnalysisError(String),
}

pub type FlowistryResult<T> = Result<T, FlowistryError>;

pub trait FlowistryAnalysis: Sized + Send + Sync {
  type Output: Serialize + Send + Sync;
  fn analyze(&mut self, tcx: TyCtxt, id: BodyId) -> anyhow::Result<Self::Output>;
}

// Implement FlowistryAnalysis for all functions with a type signature that matches
// FlowistryAnalysis::analyze
impl<F, O> FlowistryAnalysis for F
where
  F: for<'tcx> Fn<(TyCtxt<'tcx>, BodyId), Output = anyhow::Result<O>> + Send + Sync,
  O: Serialize + Send + Sync,
{
  type Output = O;
  fn analyze(&mut self, tcx: TyCtxt, id: BodyId) -> anyhow::Result<Self::Output> {
    (self)(tcx, id)
  }
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
