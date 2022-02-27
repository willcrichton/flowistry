#![feature(rustc_private)]

extern crate rustc_interface;

use std::{
  env,
  path::PathBuf,
  process::{exit, Command},
};

use clap::clap_app;
use rand::prelude::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const TARGET_DIR: &str = "target/flowistry";

fn main() {
  env_logger::init();

  let flowistry_rustc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("flowistry-driver");
  let cargo_path = env::var("CARGO_PATH").unwrap_or_else(|_| "cargo".to_string());

  let matches = clap_app!(flowistry =>
    (version: VERSION)
    (author: "Will Crichton <wcrichto@cs.stanford.edu>")
    (@setting TrailingVarArg)
    (@arg BENCH: -b --bench)
    (@subcommand rustc_version =>)
    (@subcommand preload =>)
    (@subcommand spans =>
      (@arg file:)
      (@arg flags: ...))
    (@subcommand focus =>
      (@arg file:)
      (@arg pos:)
      (@arg flags: ...))
    (@subcommand decompose =>
        (@arg file:)
        (@arg pos:)
        (@arg flags: ...))
    (@subcommand playground =>
      (@arg file:)
      (@arg start:)
      (@arg end:)
      (@arg flags: ...)
    )
  )
  .get_matches_from(env::args().skip(1));

  let (mut args, file_name) = match matches.subcommand() {
    ("rustc_version", _) => {
      let commit_hash = rustc_interface::util::commit_hash_str().unwrap_or("unknown");
      println!("{commit_hash}");
      exit(0);
    }
    ("preload", _) => {
      let mut cmd = Command::new(cargo_path);
      cmd.args(&[
        "check",
        "--all",
        "--all-features",
        "--target-dir",
        TARGET_DIR,
      ]);
      let exit_status = cmd.status().expect("could not run cargo");
      exit(exit_status.code().unwrap_or(-1));
    }
    ("spans", Some(sub_m)) => (
      vec![("FILE", sub_m.value_of("file").unwrap())],
      sub_m.value_of("file").unwrap(),
    ),
    ("playground", Some(sub_m)) => (
      vec![
        ("FILE", sub_m.value_of("file").unwrap()),
        ("START", sub_m.value_of("start").unwrap()),
        ("END", sub_m.value_of("end").unwrap()),
      ],
      sub_m.value_of("file").unwrap(),
    ),
    ("decompose" | "focus", Some(sub_m)) => (
      vec![
        ("FILE", sub_m.value_of("file").unwrap()),
        ("POS", sub_m.value_of("pos").unwrap()),
      ],
      sub_m.value_of("file").unwrap(),
    ),
    _ => {
      unimplemented!()
    }
  };

  let sub_m = match matches.subcommand() {
    (_, Some(sub_m)) => sub_m,
    _ => unreachable!(),
  };
  args.extend([
    (
      "CONTEXT_MODE",
      sub_m.value_of("context_mode").unwrap_or("SigOnly"),
    ),
    (
      "MUTABILITY_MODE",
      sub_m
        .value_of("mutability_mode")
        .unwrap_or("DistinguishMut"),
    ),
    (
      "POINTER_MODE",
      sub_m.value_of("pointer_mode").unwrap_or("Precise"),
    ),
  ]);

  let (cmd, flags) = match matches.subcommand() {
    (cmd, Some(sub_m)) => (cmd, sub_m.value_of("flags")),
    _ => unimplemented!(),
  };
  log::debug!("Command: {cmd}");
  args.push(("COMMAND", cmd));

  let file_path = PathBuf::from(file_name).canonicalize().unwrap();
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

  // Find the package and target that corresponds to a given file path
  let (pkg, target) = workspace_members
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

      // If there are multiple targets that match a given directory, e.g. `examples/whatever.rs`, then
      // find the target whose name matches the file stem
      let target = (match targets.len() {
        0 => None,
        1 => Some(targets[0]),
        _ => targets
          .into_iter()
          .find(|target| target.name == file_path.file_stem().unwrap().to_string_lossy()),
      })?;

      Some((pkg, target))
    })
    .next()
    .unwrap_or_else(|| panic!("Could not find target for path: {file_name}"));

  let mut cmd = Command::new(cargo_path);
  cmd
    .env("RUSTC_WORKSPACE_WRAPPER", flowistry_rustc_path)
    .args(&[
      "rustc",
      "--profile",
      "check",
      "--all-features",
      "--target-dir",
      TARGET_DIR,
    ]);

  let bench = matches.is_present("BENCH");
  cmd.arg(if bench { "-v" } else { "-q" });

  // Add compile filter to specify the target corresponding to the given file
  cmd.arg("-p").arg(&pkg.name);
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

  // RNG is necessary to avoid caching
  let n = thread_rng().gen::<u64>();
  cmd.args(&["--", &format!("--flowistry={n}")]);

  // Add args passed from CLI
  cmd.args(flags);

  // TODO: need to figure out how to download / compile dev-dependencies
  // // Pass --test to rustc so #[test] functions can be analyzed
  // cmd.arg("--test");

  // FIXME(wcrichto): we should make these CLI args as well, then do
  //   caching on VSCode's side
  for (k, v) in args {
    cmd.env(format!("FLOWISTRY_{k}"), v);
  }

  // HACK: if running flowistry on the rustc codebase, this env var needs to exist
  // for the code to compile
  if workspace_members.iter().any(|pkg| pkg.name == "rustc-main") {
    cmd.env("CFG_RELEASE", "");
  }

  if bench {
    eprintln!(
      "{:?}",
      cmd
        .get_envs()
        .map(|(k, v)| vec![k.to_string_lossy(), v.unwrap().to_string_lossy()])
        .collect::<Vec<_>>()
    );
  }

  let exit_status = cmd.status().expect("could not run cargo");
  if !exit_status.success() {
    exit(exit_status.code().unwrap_or(-1));
  }
}
