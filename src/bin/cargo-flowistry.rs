#![feature(rustc_private)]

extern crate rustc_interface;

use clap::clap_app;
use rand::prelude::*;
use std::{
  env,
  path::PathBuf,
  process::{exit, Command},
};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
  let flowistry_rustc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("flowistry-driver");
  let cargo_path = env::var("CARGO_PATH").unwrap_or_else(|_| "cargo".to_string());

  let matches = clap_app!(flowistry =>
    (version: VERSION)
    (author: "Will Crichton <wcrichto@cs.stanford.edu>")
    (@setting TrailingVarArg)
    (@subcommand rustc_version =>)
    (@subcommand backward_slice =>
      (@arg file:)
      (@arg start:)
      (@arg end:)
      (@arg context_mode: --contextmode +takes_value)
      (@arg mutability_mode: --mutabilitymode +takes_value)
      (@arg pointer_mode: --pointermode +takes_value)
      (@arg flags: ...)
    )
    (@subcommand forward_slice =>
      (@arg file:)
      (@arg start:)
      (@arg end:)
      (@arg flags: ...)
    )
    (@subcommand effects =>
      (@arg file:)
      (@arg pos:)
      (@arg flags: ...)
    )
  )
  .get_matches_from(env::args().skip(1));

  let (mut args, file_name) = match matches.subcommand() {
    ("rustc_version", _) => {
      let commit_hash = rustc_interface::util::commit_hash_str().unwrap_or("unknown");
      println!("{}", commit_hash);
      exit(0);
    }
    ("backward_slice" | "forward_slice", Some(sub_m)) => (
      vec![
        ("FILE", sub_m.value_of("file").unwrap()),
        ("START", sub_m.value_of("start").unwrap()),
        ("END", sub_m.value_of("end").unwrap()),
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
      ],
      sub_m.value_of("file").unwrap(),
    ),
    ("effects", Some(sub_m)) => (
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

  let (cmd, flags) = match matches.subcommand() {
    (cmd, Some(sub_m)) => (cmd, sub_m.value_of("flags")),
    _ => unimplemented!(),
  };
  args.push(("COMMAND", cmd));

  let file_path = PathBuf::from(file_name);
  let metadata = cargo_metadata::MetadataCommand::new()
    .no_deps()
    .other_options(["--offline".to_string()])
    .exec()
    .unwrap();
  let (pkg, target) = metadata
    .workspace_members
    .iter()
    .filter_map(|pkg_id| {
      let pkg = metadata
        .packages
        .iter()
        .find(|pkg| &pkg.id == pkg_id)
        .unwrap();

      let target = pkg
        .targets
        .iter()
        .filter(|target| file_path.starts_with(target.src_path.parent().unwrap()))
        .max_by_key(|target| target.src_path.components().count());

      target.map(move |target| (pkg, target))
    })
    .next()
    .unwrap_or_else(|| panic!("Could not find target for path: {}", file_name));

  let mut cmd = Command::new(cargo_path);
  cmd
    .env("RUSTC_WORKSPACE_WRAPPER", flowistry_rustc_path)
    .args(&["rustc", "--profile", "check", "-q"]);

  // Add compile filter to specify the target corresponding to the given file
  cmd.arg("-p").arg(&pkg.name);
  let kind = &target.kind[0];
  cmd.arg(format!("--{}", kind));
  match kind.as_str() {
    "lib" => {}
    _ => {
      cmd.arg(&target.name);
    }
  };

  // RNG is necessary to avoid caching
  let n = thread_rng().gen::<u64>();
  cmd.args(&["--", &format!("--flowistry={}", n)]).args(flags);

  // FIXME(wcrichto): we should make these CLI args as well, then do
  //   caching on VSCode's side
  for (k, v) in args {
    cmd.env(format!("FLOWISTRY_{}", k), v);
  }

  let exit_status = cmd.status().expect("could not run cargo");
  if !exit_status.success() {
    exit(exit_status.code().unwrap_or(-1));
  }
}
