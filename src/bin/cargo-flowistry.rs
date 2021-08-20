use clap::clap_app;
use std::{
  env,
  process::{exit, Command},
};

fn main() {
  let flowistry_rustc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("flowistry-rustc");
  let cargo_path = env::var("CARGO_PATH").unwrap_or("cargo".to_string());

  let matches = clap_app!(app =>
    (version: "0.1")
    (author: "Will Crichton <wcrichto@cs.stanford.edu>")
    (@setting TrailingVarArg)
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
  )
  .get_matches_from(env::args().skip(1));

  let (args, flags) = match matches.subcommand() {
    (cmd @ ("backward_slice" | "forward_slice"), Some(sub_m)) => (
      vec![
        ("COMMAND", cmd),
        ("FILE", sub_m.value_of("file").unwrap()),
        ("START", sub_m.value_of("start").unwrap()),
        ("END", sub_m.value_of("end").unwrap()),
      ],
      sub_m.value_of("flags"),
    ),
    _ => {
      unimplemented!()
    }
  };

  let mut cmd = Command::new(cargo_path);
  cmd
    .arg("check")
    .arg("-q")
    .args(flags)
    .env("RUSTC_WRAPPER", flowistry_rustc_path);

  for (k, v) in args {
    cmd.env(format!("FLOWISTRY_{}", k), v);
  }

  let exit_status = cmd.status().expect("could not run cargo");
  if !exit_status.success() {
    exit(exit_status.code().unwrap_or(-1));
  }
}
