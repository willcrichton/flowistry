//! Adapted from Clippy's driver: https://github.com/rust-lang/rust-clippy/blob/fd30241281333d73d504355b2f4d0ecd94f27b0e/src/driver.rs

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_interface;
extern crate rustc_serialize;

use flowistry::{
  extensions::{ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE},
  infoflow::Direction,
};
use flowistry_ide::{
  analysis::{FlowistryError, FlowistryResult},
  range::Range,
};
use fluid_let::fluid_set;
use log::debug;
use rustc_interface::interface::Result as RustcResult;
use rustc_serialize::{json, Encodable};
use std::{
  env,
  fmt::Debug,
  ops::Deref,
  path::PathBuf,
  process::{exit, Command},
  str::FromStr,
};

fn arg<T>(s: &str) -> T
where
  T: FromStr,
  T::Err: Debug,
{
  env::var(format!("FLOWISTRY_{}", s))
    .unwrap_or_else(|_| panic!("Missing argument: {}", s))
    .parse()
    .unwrap()
}

/// If a command-line option matches `find_arg`, then apply the predicate `pred` on its value. If
/// true, then return it. The parameter is assumed to be either `--arg=value` or `--arg value`.
fn arg_value<'a, T: Deref<Target = str>>(
  args: &'a [T],
  find_arg: &str,
  pred: impl Fn(&str) -> bool,
) -> Option<&'a str> {
  let mut args = args.iter().map(Deref::deref);
  while let Some(arg) = args.next() {
    let mut arg = arg.splitn(2, '=');
    if arg.next() != Some(find_arg) {
      continue;
    }

    match arg.next().or_else(|| args.next()) {
      Some(v) if pred(v) => return Some(v),
      _ => {}
    }
  }
  None
}

fn toolchain_path(home: Option<String>, toolchain: Option<String>) -> Option<PathBuf> {
  home.and_then(|home| {
    toolchain.map(|toolchain| {
      let mut path = PathBuf::from(home);
      path.push("toolchains");
      path.push(toolchain);
      path
    })
  })
}

fn try_analysis<T: for<'a> Encodable<json::Encoder<'a>>>(
  f: impl FnOnce() -> FlowistryResult<T>,
) -> RustcResult<()> {
  let result = match f() {
    Ok(output) => Ok(output),
    Err(e) => match e {
      FlowistryError::BuildError => {
        return Err(rustc_errors::ErrorReported);
      }
      FlowistryError::AnalysisError(msg) => Err(msg),
    },
  };

  println!("{}", json::encode(&result).unwrap());

  Ok(())
}

fn run_flowistry(args: &[String]) -> RustcResult<()> {
  debug!("Running flowistry with args: {}", args.join(" "));

  match arg::<String>("COMMAND").as_str() {
    cmd @ ("backward_slice" | "forward_slice") => {
      let range = Range {
        start: arg::<usize>("START"),
        end: arg::<usize>("END"),
        filename: arg::<String>("FILE"),
      };

      let direction = if cmd == "backward_slice" {
        Direction::Backward
      } else {
        Direction::Forward
      };

      let context_mode = match arg::<String>("CONTEXT_MODE").as_str() {
        "Recurse" => ContextMode::Recurse,
        "SigOnly" => ContextMode::SigOnly,
        flag => panic!("Bad value of context mode: {}", flag),
      };

      let mutability_mode = match arg::<String>("MUTABILITY_MODE").as_str() {
        "DistinguishMut" => MutabilityMode::DistinguishMut,
        "IgnoreMut" => MutabilityMode::IgnoreMut,
        flag => panic!("Bad value of context mode: {}", flag),
      };

      let pointer_mode = match arg::<String>("POINTER_MODE").as_str() {
        "Precise" => PointerMode::Precise,
        "Conservative" => PointerMode::Conservative,
        flag => panic!("Bad value of pointer mode: {}", flag),
      };

      let eval_mode = EvalMode {
        context_mode,
        mutability_mode,
        pointer_mode,
      };
      fluid_set!(EVAL_MODE, eval_mode);

      try_analysis(move || flowistry_ide::slicing::slice(direction, range, args))
    }
    "effects" => {
      let _pos = arg::<usize>("POS");
      todo!()
      // let id = flowistry::FunctionIdentifier::Range(Range {
      //   start: pos,
      //   end: pos,
      //   filename: arg::<String>("FILE"),
      // });
      // try_analysis(move || flowistry_ide::effects::effects(id, args))
    }
    _ => unimplemented!(),
  }
}

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

fn main() {
  rustc_driver::init_rustc_env_logger();
  env_logger::init();

  exit(rustc_driver::catch_with_exit_code(move || {
    let orig_args: Vec<String> = env::args().collect();

    let sys_root_arg = arg_value(&orig_args, "--sysroot", |_| true);
    let have_sys_root_arg = sys_root_arg.is_some();
    let sys_root = sys_root_arg
      .map(PathBuf::from)
      .or_else(|| std::env::var("SYSROOT").ok().map(PathBuf::from))
      .or_else(|| {
        let home = std::env::var("RUSTUP_HOME")
          .or_else(|_| std::env::var("MULTIRUST_HOME"))
          .ok();
        let toolchain = std::env::var("RUSTUP_TOOLCHAIN")
          .or_else(|_| std::env::var("MULTIRUST_TOOLCHAIN"))
          .ok();
        toolchain_path(home, toolchain)
      })
      .or_else(|| {
        Command::new("rustc")
          .arg("--print")
          .arg("sysroot")
          .output()
          .ok()
          .and_then(|out| String::from_utf8(out.stdout).ok())
          .map(|s| PathBuf::from(s.trim()))
      })
      .or_else(|| option_env!("SYSROOT").map(PathBuf::from))
      .or_else(|| {
        let home = option_env!("RUSTUP_HOME")
          .or(option_env!("MULTIRUST_HOME"))
          .map(ToString::to_string);
        let toolchain = option_env!("RUSTUP_TOOLCHAIN")
          .or(option_env!("MULTIRUST_TOOLCHAIN"))
          .map(ToString::to_string);
        toolchain_path(home, toolchain)
      })
      .map(|pb| pb.to_string_lossy().to_string())
      .expect(
        "need to specify SYSROOT env var during flowistry compilation, or use rustup or multirust",
      );

    let mut args: Vec<String> = orig_args.clone();

    // remove flowistry-driver from invocation
    args.remove(0);

    // this conditional check for the --sysroot flag is there so users can call
    // `flowistry-driver` directly without having to pass --sysroot or anything
    if !have_sys_root_arg {
      args.extend(["--sysroot".into(), sys_root]);
    }

    let mut is_flowistry = false;
    args.retain(|arg| {
      if arg.starts_with("--flowistry") {
        is_flowistry = true;
        false
      } else {
        true
      }
    });

    if is_flowistry {
      run_flowistry(&args)
    } else {
      rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run()
    }
  }))
}
