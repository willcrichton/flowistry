use std::process::{exit, Command};

fn main() {
  let ifc_path = std::env::current_exe()
    .expect("current executable path invalid")
    .with_file_name("ifc-driver");
  let mut cmd = Command::new("cargo");
  cmd.env("RUSTC_WORKSPACE_WRAPPER", ifc_path).args(&[
    "rustc",
    "--profile",
    "check",
    "-q",
    "--",
    "--ifc",
  ]);

  let exit_status = cmd.status().expect("could not run cargo");
  if !exit_status.success() {
    exit(exit_status.code().unwrap_or(-1));
  }
}
