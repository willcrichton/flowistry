use std::env;
use std::path::Path;
use std::process::{exit, Command};

fn main() {
  let mut sysroot = String::from_utf8(
    Command::new("rustc")
      .args(&["--print", "sysroot"])
      .output()
      .expect("rustc --print sysroot failed")
      .stdout,
  )
  .unwrap();
  sysroot = sysroot.trim().to_owned();

  let mut args = env::args().skip(1).collect::<Vec<_>>();

  // Setting RUSTC_WRAPPER causes Cargo to pass 'rustc' as the first argument.
  // We're invoking the compiler programmatically, so we ignore this
  if args.len() > 0 && Path::new(&args[0]).file_stem() == Some("rustc".as_ref()) {
    args.remove(0);
  }

  let exit_code = Command::new("flowistry-driver")
    .args(args)
    .args(vec!["--sysroot".into(), sysroot])
    .status()
    .expect("flowistry-driver failed to execute");

  if !exit_code.success() {
    exit(exit_code.code().unwrap_or(-1));
  }
}
