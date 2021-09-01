//! Adapted from Clippy's driver: https://github.com/rust-lang/rust-clippy/blob/fd30241281333d73d504355b2f4d0ecd94f27b0e/src/driver.rs

#![feature(rustc_private)]

extern crate rustc_driver;

use anyhow::Result;
use flowistry::Direction;
use log::debug;
use std::{
  env,
  fmt::Debug,
  ops::Deref,
  path::{Path, PathBuf},
  process::{exit, Command},
  str::FromStr,
};

fn arg<T>(s: &str) -> T
where
  T: FromStr,
  T::Err: Debug,
{
  env::var(format!("FLOWISTRY_{}", s))
    .expect(&format!("Missing argument: {}", s))
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

fn run_flowistry(args: &[String]) -> Result<()> {
  debug!("Running flowistry with args: {}", args.join(" "));
  match arg::<String>("COMMAND").as_str() {
    cmd @ ("backward_slice" | "forward_slice") => {
      let range = flowistry::Range {
        start: arg::<usize>("START"),
        end: arg::<usize>("END"),
        filename: arg::<String>("FILE"),
      };

      let direction = if cmd == "backward_slice" {
        Direction::Backward
      } else {
        Direction::Forward
      };

      let slice = flowistry::slice(direction, range, &args).unwrap();
      println!("{}", serde_json::to_string(&slice).unwrap());
      Ok(())
    }
    "effects" => {
      let pos = arg::<usize>("POS");
      let id = flowistry::FunctionIdentifier::Range(flowistry::Range {
        start: pos,
        end: pos,
        filename: arg::<String>("FILE"),
      });
      let effects = flowistry::effects(id, &args).unwrap();
      println!("{}", serde_json::to_string(&effects).unwrap());
      Ok(())
    }
    _ => unimplemented!(),
  }
}

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

fn main() {
  rustc_driver::init_rustc_env_logger();
  pretty_env_logger::init();

  exit(rustc_driver::catch_with_exit_code(move || {
    let mut orig_args: Vec<String> = env::args().collect();

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

    if let Some(pos) = orig_args.iter().position(|arg| arg == "--rustc") {
      orig_args.remove(pos);
      orig_args[0] = "rustc".to_string();

      // if we call "rustc", we need to pass --sysroot here as well
      let mut args: Vec<String> = orig_args.clone();
      if !have_sys_root_arg {
        args.extend(vec!["--sysroot".into(), sys_root]);
      };

      return rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run();
    }

    // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
    // We're invoking the compiler programmatically, so we ignore this/
    let wrapper_mode =
      orig_args.get(1).map(Path::new).and_then(Path::file_stem) == Some("rustc".as_ref());

    if wrapper_mode {
      // we still want to be able to invoke it normally though
      orig_args.remove(1);
    }

    // this conditional check for the --sysroot flag is there so users can call
    // `flowistry-driver` directly
    // without having to pass --sysroot or anything
    let mut args: Vec<String> = orig_args.clone();
    if !have_sys_root_arg {
      args.extend(vec!["--sysroot".into(), sys_root]);
    }

    let in_primary_package = env::var("CARGO_PRIMARY_PACKAGE").is_ok();
    if in_primary_package {
      run_flowistry(&args).unwrap();
      Ok(())
    } else {
      rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run()
    }
  }))
}
