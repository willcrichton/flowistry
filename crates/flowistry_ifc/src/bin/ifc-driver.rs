#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_interface;

use rustc_interface::interface::Result as RustcResult;
use std::env;

use std::process::exit;

struct DefaultCallbacks;
impl rustc_driver::Callbacks for DefaultCallbacks {}

fn run_ifc(args: &[String]) -> RustcResult<()> {
  let mut callbacks = flowistry_ifc::Callbacks;
  rustc_driver::RunCompiler::new(args, &mut callbacks).run()
}

fn main() {
  rustc_driver::init_rustc_env_logger();
  env_logger::init();

  exit(rustc_driver::catch_with_exit_code(move || {
    let orig_args: Vec<String> = env::args().collect();
    let sysroot = env::var("SYSROOT").unwrap();

    let mut args: Vec<String> = orig_args;
    args.remove(0);
    args.extend(["--sysroot".into(), sysroot]);

    let mut is_ifc = false;
    args.retain(|arg| {
      if arg.starts_with("--ifc") {
        is_ifc = true;
        false
      } else {
        true
      }
    });

    if is_ifc {
      run_ifc(&args)
    } else {
      rustc_driver::RunCompiler::new(&args, &mut DefaultCallbacks).run()
    }
  }))
}
