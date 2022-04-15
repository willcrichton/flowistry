//! A framework for running up custom cargo commands that use rustc_private.
//!
//! Most of this file is either directly copy/pasted, or otherwise generalized
//! from the Clippy driver: https://github.com/rust-lang/rust-clippy/tree/master/src

#![feature(rustc_private, trait_alias)]

extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_serialize;

use std::{
  env,
  ops::Deref,
  path::{Path, PathBuf},
  process::{exit, Command},
};

use rustc_serialize::{json, Decodable, Encodable};
use rustc_tools_util::VersionInfo;

const TARGET_DIR: &str = "target/flowistry";

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

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

pub trait JsonEncodable = for<'a> Encodable<json::Encoder<'a>>;
pub trait JsonDecodable = Decodable<json::Decoder>;

pub struct RustcPluginArgs<Args> {
  pub args: Args,
  pub flags: Option<Vec<String>>,
  pub file: Option<PathBuf>,
}

pub trait RustcPlugin: Sized {
  type Args: JsonEncodable + JsonDecodable;

  fn bin_name() -> String;

  fn args(&self) -> RustcPluginArgs<Self::Args>;

  fn run(
    self,
    compiler_args: Vec<String>,
    plugin_args: Self::Args,
  ) -> rustc_interface::interface::Result<()>;
}

const PLUGIN_ARGS: &str = "PLUGIN_ARGS";

pub fn cli_main<T: RustcPlugin>(plugin: T) {
  let mut cmd = Command::new("cargo");

  let mut path = env::current_exe()
    .expect("current executable path invalid")
    .with_file_name(T::bin_name());

  if cfg!(windows) {
    path.set_extension("exe");
  }

  cmd.env("RUSTC_WORKSPACE_WRAPPER", path).args(&[
    "check",
    "-q",
    "--target-dir",
    TARGET_DIR,
  ]);

  let args = plugin.args();

  let metadata = cargo_metadata::MetadataCommand::new()
    .no_deps()
    .other_options(["--all-features".to_string(), "--offline".to_string()])
    .exec()
    .unwrap();

  let workspace_members = metadata
    .workspace_members
    .iter()
    .map(|pkg_id| {
      metadata
        .packages
        .iter()
        .find(|pkg| &pkg.id == pkg_id)
        .unwrap()
    })
    .collect::<Vec<_>>();

  if let Some(file_path) = args.file {
    // Find the package and target that corresponds to a given file path
    let mut matching = workspace_members
      .iter()
      .filter_map(|pkg| {
        let targets = pkg
          .targets
          .iter()
          .filter(|target| {
            let src_path = target.src_path.canonicalize().unwrap();
            file_path.starts_with(src_path.parent().unwrap())
          })
          .collect::<Vec<_>>();

        let target = (match targets.len() {
          0 => None,
          1 => Some(targets[0]),
          _ => {
            // If there are multiple targets that match a given directory, e.g. `examples/whatever.rs`, then
            // find the target whose name matches the file stem
            let stem = file_path.file_stem().unwrap().to_string_lossy();
            let name_matches_stem = targets
              .clone()
              .into_iter()
              .find(|target| target.name == stem);

            // Otherwise we're in a special case, e.g. "main.rs" corresponds to the bin target.
            name_matches_stem.or_else(|| {
              let kind = (if stem == "main" { "bin" } else { "lib" }).to_string();
              targets
                .into_iter()
                .find(|target| target.kind.contains(&kind))
            })
          }
        })?;

        Some((pkg, target))
      })
      .collect::<Vec<_>>();
    let (pkg, target) = match matching.len() {
      0 => panic!("Could not find target for path: {}", file_path.display()),
      1 => matching.remove(0),
      _ => panic!("Too many matching targets: {matching:?}"),
    };

    // Add compile filter to specify the target corresponding to the given file
    cmd.arg("-p").arg(format!("{}:{}", pkg.name, pkg.version));
    let kind = &target.kind[0];
    if kind != "proc-macro" {
      cmd.arg(format!("--{kind}"));
    }
    match kind.as_str() {
      "lib" | "proc-macro" => {}
      _ => {
        cmd.arg(&target.name);
      }
    };
    log::debug!(
      "Package: {}, target kind {}, target name {}",
      pkg.name,
      kind,
      target.name
    );
  } else {
    cmd.arg("--all");
  }

  let args_str = json::encode(&args.args).unwrap();
  cmd.env(PLUGIN_ARGS, args_str);

  // HACK: if running flowistry on the rustc codebase, this env var needs to exist
  // for the code to compile
  if workspace_members.iter().any(|pkg| pkg.name == "rustc-main") {
    cmd.env("CFG_RELEASE", "");
  }

  cmd.arg("--");
  if let Some(flags) = args.flags {
    cmd.args(flags);
  }

  cmd.env("RUSTFLAGS", "-Awarnings");

  let exit_status = cmd
    .spawn()
    .expect("could not run cargo")
    .wait()
    .expect("failed to wait for cargo?");

  exit(exit_status.code().unwrap_or(-1));
}

pub fn driver_main<T: RustcPlugin>(plugin: T) {
  rustc_driver::init_rustc_env_logger();

  exit(rustc_driver::catch_with_exit_code(move || {
    let mut orig_args: Vec<String> = env::args().collect();

    // Get the sysroot, looking from most specific to this invocation to the least:
    // - command line
    // - runtime environment
    //    - SYSROOT
    //    - RUSTUP_HOME, MULTIRUST_HOME, RUSTUP_TOOLCHAIN, MULTIRUST_TOOLCHAIN
    // - sysroot from rustc in the path
    // - compile-time environment
    //    - SYSROOT
    //    - RUSTUP_HOME, MULTIRUST_HOME, RUSTUP_TOOLCHAIN, MULTIRUST_TOOLCHAIN
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
        .expect("need to specify SYSROOT env var during clippy compilation, or use rustup or multirust");

    if orig_args.iter().any(|a| a == "--version" || a == "-V") {
      let version_info = rustc_tools_util::get_version_info!();
      println!("{}", version_info);
      exit(0);
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
    // the driver directly without having to pass --sysroot or anything
    let mut args: Vec<String> = orig_args.clone();
    if !have_sys_root_arg {
      args.extend(vec!["--sysroot".into(), sys_root]);
    };

    let primary_package = env::var("CARGO_PRIMARY_PACKAGE").is_ok();
    let normal_rustc = args.iter().any(|arg| arg.starts_with("--print"));
    let run_plugin = primary_package && !normal_rustc;

    if run_plugin {
      let plugin_args = json::decode::<T::Args>(&env::var(PLUGIN_ARGS).unwrap());
      plugin.run(args, plugin_args)
    } else {
      rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run()
    }
  }))
}
