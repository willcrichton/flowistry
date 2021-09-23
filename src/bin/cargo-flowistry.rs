#![feature(rustc_private)]

extern crate rustc_interface;

use clap::clap_app;
use std::{
  env,
  path::PathBuf,
  process::{exit, Command},
};

fn main() {
  let flowistry_rustc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("flowistry-driver");
  let cargo_path = env::var("CARGO_PATH").unwrap_or_else(|_| "cargo".to_string());

  // FIXME(wcrichto): can we read the version directly from Cargo.toml?
  let matches = clap_app!(flowistry =>
    (version: "0.3.9")
    (author: "Will Crichton <wcrichto@cs.stanford.edu>")
    (@setting TrailingVarArg)
    (@subcommand rustc_version =>)
    (@subcommand backward_slice =>
      (@arg file:)
      (@arg start:)
      (@arg end:)
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

  let mut file_path = PathBuf::from(file_name);
  let mut package = None;
  while let Some(parent) = file_path.parent() {
    file_path = parent.to_path_buf();

    let manifest_path = file_path.join("Cargo.toml");
    if manifest_path.exists() {
      let manifest = cargo_toml::Manifest::from_path(manifest_path).unwrap();
      package = Some(manifest.package.unwrap().name);
      break;
    }
  }

  let mut cmd = Command::new(cargo_path);
  cmd
    .arg("check")
    .arg("-q")
    .args(flags)
    .env("RUSTC_WORKSPACE_WRAPPER", flowistry_rustc_path);

  match package {
    Some(package) => {
      cmd.arg("-p").arg(package);
    }
    None => panic!("Could not find package for file: {}", file_path.display()),
  };

  for (k, v) in args {
    cmd.env(format!("FLOWISTRY_{}", k), v);
  }

  let exit_status = cmd.status().expect("could not run cargo");
  if !exit_status.success() {
    exit(exit_status.code().unwrap_or(-1));
  }
}
