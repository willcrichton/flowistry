#![feature(rustc_private)]

use anyhow::{Context, Error, Result};
use clap::clap_app;
use generate_rustc_flags::generate_rustc_flags;
use log::debug;

extern crate rustc_driver;
extern crate rustc_interface;

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|_tcx| {
      println!("hi");
    });

    rustc_driver::Compilation::Stop
  }
}

fn run() -> Result<()> {
  let _ = env_logger::try_init();

  let matches = clap_app!(app =>
    (@arg path:)
  )
  .get_matches();

  let source_path = matches.value_of("path").context("Missing path")?;
  let flags = generate_rustc_flags(source_path)?;
  debug!("Rustc command:\n{}", flags.join(" "));

  let mut callbacks = Callbacks;
  rustc_driver::catch_fatal_errors(|| rustc_driver::RunCompiler::new(&flags, &mut callbacks).run())
    .map_err(|_| Error::msg("rustc panicked"))?
    .map_err(|_| Error::msg("driver failed"))?;

  Ok(())
}

fn main() {
  run().unwrap();
}
