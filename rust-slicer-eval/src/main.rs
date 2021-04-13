#![feature(rustc_private, in_band_lifetimes)]

use anyhow::{Context, Error, Result};
use clap::clap_app;
use generate_rustc_flags::{generate_rustc_flags, CliFeatures};
use log::debug;
use std::env;

use crate::visitor::EvalCrateVisitor;

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_span;

mod visitor;

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let mut eval_visitor = EvalCrateVisitor::new(tcx);
      tcx
        .hir()
        .krate()
        .par_visit_all_item_likes(&mut eval_visitor);
      println!(
        "{}",
        serde_json::to_string(&eval_visitor.eval_results).unwrap()
      );
    });

    rustc_driver::Compilation::Stop
  }
}

fn run() -> Result<()> {
  let _ = env_logger::try_init();

  let matches = clap_app!(app =>
    (@arg all_features: --("all-features"))
    (@arg features: --features +takes_value)
    (@arg path:)
  )
  .get_matches();

  let features = CliFeatures::from_command_line(
    &matches
      .value_of("features")
      .map(|s| s.split(",").map(|s| s.to_string()).collect())
      .unwrap_or_else(Vec::new),
    matches.is_present("all_features"),
    true,
  )?;

  let source_path = matches.value_of("path").context("Missing path")?;
  let (flags, env) = generate_rustc_flags(source_path, features, true)?;
  for (k, v) in env {
    env::set_var(k, v);
  }

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
