use std::{
  borrow::Cow,
  env,
  path::PathBuf,
  process::{exit, Command},
  time::Instant,
};

use anyhow::Context;
use base64::Engine;
use clap::{Parser, Subcommand};
use flowistry::extensions::{
  ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE,
};
use fluid_let::fluid_set;
use log::{debug, info};
use rustc_hir::BodyId;
use rustc_interface::interface::Result as RustcResult;
use rustc_middle::ty::TyCtxt;
use rustc_plugin::{CrateFilter, RustcPlugin, RustcPluginArgs, Utf8Path};
use rustc_utils::{
  mir::borrowck_facts,
  source_map::{
    filename::Filename,
    find_bodies::find_enclosing_bodies,
    range::{CharPos, CharRange, FunctionIdentifier, ToSpan},
  },
  timer::elapsed,
};
use serde::{Deserialize, Serialize};

#[derive(Parser, Serialize, Deserialize)]
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
  },

  Focus {
    file: String,
    pos: usize,
  },

  Decompose {
    file: String,
    pos: usize,
  },

  Playground {
    file: String,
    start: usize,
    end: usize,
  },

  Preload,

  RustcVersion,
}

pub struct FlowistryPlugin;
impl RustcPlugin for FlowistryPlugin {
  type Args = FlowistryPluginArgs;

  fn driver_name(&self) -> Cow<'static, str> {
    "flowistry-driver".into()
  }

  fn version(&self) -> Cow<'static, str> {
    env!("CARGO_PKG_VERSION").into()
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
          .args(["check", "--all", "--all-features", "--target-dir"])
          .arg(target_dir);
        let exit_status = cmd.status().expect("could not run cargo");
        exit(exit_status.code().unwrap_or(-1));
      }
      RustcVersion => {
        let version_str = rustc_interface::util::rustc_version_str().unwrap_or("unknown");
        println!("{version_str}");
        exit(0);
      }
      _ => {}
    };

    let file = match &args.command {
      Spans { file, .. } => file,
      Focus { file, .. } => file,
      Decompose { file, .. } => file,
      Playground { file, .. } => file,
      _ => unreachable!(),
    };

    RustcPluginArgs {
      filter: CrateFilter::CrateContainingFile(PathBuf::from(file)),
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
        let compute_target = || CharRange {
          start: CharPos(start),
          end: CharPos(end),
          filename: Filename::intern(&file),
        };
        postprocess(run(
          crate::playground::playground,
          compute_target,
          &compiler_args,
        ))
      }
      Focus { file, pos, .. } => {
        let compute_target = || {
          let range = CharRange {
            start: CharPos(pos),
            end: CharPos(pos),
            filename: Filename::intern(&file),
          };
          FunctionIdentifier::Range(range)
        };
        postprocess(run(crate::focus::focus, compute_target, &compiler_args))
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
              FunctionIdentifier::Range(ByteRange::from_char_range(_pos, _pos, &_file, &indices));
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
      e => Err(e),
    },
  };

  let mut encoder =
    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
  serde_json::to_writer(&mut encoder, &result).unwrap();
  let buffer = encoder.finish().unwrap();
  print!(
    "{}",
    base64::engine::general_purpose::STANDARD.encode(buffer)
  );

  Ok(())
}

pub fn run_with_callbacks(
  args: &[String],
  callbacks: &mut (dyn rustc_driver::Callbacks + Send),
) -> FlowistryResult<()> {
  let mut args = args.to_vec();
  args.extend(
    "-Z identify-regions -Z mir-opt-level=0 -A warnings -Z maximal-hir-to-mir-coverage"
      .split(' ')
      .map(|s| s.to_owned()),
  );

  let compiler = rustc_driver::RunCompiler::new(&args, callbacks);
  compiler.run().map_err(|_| FlowistryError::BuildError)
}

fn run<A: FlowistryAnalysis, T: ToSpan>(
  analysis: A,
  compute_target: impl FnOnce() -> T + Send,
  args: &[String],
) -> FlowistryResult<A::Output> {
  let mut callbacks = FlowistryCallbacks {
    analysis: Some(analysis),
    compute_target: Some(compute_target),
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
    .map_err(|e| FlowistryError::AnalysisError {
      error: e.to_string(),
    })
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum FlowistryError {
  BuildError,
  AnalysisError { error: String },
  FileNotFound,
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

struct FlowistryCallbacks<A: FlowistryAnalysis, T: ToSpan, F: FnOnce() -> T> {
  analysis: Option<A>,
  compute_target: Option<F>,
  output: Option<anyhow::Result<A::Output>>,
  rustc_start: Instant,
  eval_mode: Option<EvalMode>,
}

impl<A: FlowistryAnalysis, T: ToSpan, F: FnOnce() -> T> rustc_driver::Callbacks
  for FlowistryCallbacks<A, T, F>
{
  fn config(&mut self, config: &mut rustc_interface::Config) {
    borrowck_facts::enable_mir_simplification();
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_expansion<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    elapsed("rustc", self.rustc_start);
    fluid_set!(EVAL_MODE, self.eval_mode.unwrap_or_default());

    let start = Instant::now();
    queries.global_ctxt().unwrap().enter(|tcx| {
      elapsed("global_ctxt", start);
      let mut analysis = self.analysis.take().unwrap();
      self.output = Some((|| {
        let target = (self.compute_target.take().unwrap())().to_span(tcx)?;
        let mut bodies = find_enclosing_bodies(tcx, target);
        let body = bodies.next().context("Selection did not map to a body")?;
        analysis.analyze(tcx, body)
      })());
    });

    rustc_driver::Compilation::Stop
  }
}
