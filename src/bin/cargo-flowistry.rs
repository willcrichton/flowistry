use clap::clap_app;
use std::env;
use std::process::{exit, Command};

fn main() {
  let flowistry_rustc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("flowistry-rustc");
  let cargo_path = env::var("CARGO_PATH").unwrap_or("cargo".to_string());

  // let matches = App::new("flowistry")
  //   .setting(AppSettings::TrailingVarArgs)
  //   .arg(Arg::from_usage("<flags>..."))

  let matches = clap_app!(app =>
    (version: "0.1")
    (author: "Will Crichton <wcrichto@cs.stanford.edu>")
    (@setting TrailingVarArg)
    (@subcommand backward_slice =>
      (@arg file:)
      (@arg start_line:)
      (@arg start_col:)
      (@arg end_line:)
      (@arg end_col:)
      (@arg flags: ...)
    )
  )
  .get_matches_from(env::args().skip(1));

  let (args, flags) = match matches.subcommand() {
    ("backward_slice", Some(sub_m)) => (
      vec![
        ("COMMAND", "backward_slice"),
        ("FILE", sub_m.value_of("file").unwrap()),
        ("START_LINE", sub_m.value_of("start_line").unwrap()),
        ("START_COL", sub_m.value_of("start_col").unwrap()),
        ("END_LINE", sub_m.value_of("end_line").unwrap()),
        ("END_COL", sub_m.value_of("end_col").unwrap()),
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
