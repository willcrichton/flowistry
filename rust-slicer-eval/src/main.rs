#![recursion_limit = "256"]
#![feature(rustc_private, in_band_lifetimes)]

use anyhow::{Context, Error, Result};
use clap::clap_app;
use generate_rustc_flags::{generate_rustc_flags, CliFeatures};
use log::debug;
use serde::Serialize;
use std::env;
use std::fs::File;

use crate::visitor::EvalCrateVisitor;

extern crate rustc_ast;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_parse;
extern crate rustc_span;

mod visitor;

struct Callbacks {
  output_path: String,
}

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

      let results = eval_visitor.eval_results.lock().unwrap();
      let mut file = File::create(&self.output_path).unwrap();
      results
        .serialize(&mut serde_json::Serializer::new(&mut file))
        .unwrap();
    });

    rustc_driver::Compilation::Stop
  }
}

fn run() -> Result<()> {
  let _ = env_logger::try_init();

  let matches = clap_app!(app =>
    (@arg threads: -j +takes_value)
    (@arg all_features: --("all-features"))
    (@arg features: --features +takes_value)
    (@arg input_path:)
    (@arg output_path:)
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

  let input_path = matches
    .value_of("input_path")
    .context("Missing input_path")?;
  let (mut flags, env) = generate_rustc_flags(input_path, features, true)?;
  for (k, v) in env {
    env::set_var(k, v);
  }

  flags.extend_from_slice(&[
    "-Z".to_string(),
    format!("threads={}", matches.value_of("threads").unwrap_or("8")),
  ]);

  debug!("Rustc command:\n{}", flags.join(" "));

  let mut callbacks = Callbacks {
    output_path: matches
      .value_of("output_path")
      .context("Missing output_path")?
      .to_owned(),
  };
  rustc_driver::catch_fatal_errors(|| rustc_driver::RunCompiler::new(&flags, &mut callbacks).run())
    .map_err(|_| Error::msg("rustc panicked"))?
    .map_err(|_| Error::msg("driver failed"))?;

  Ok(())
}

fn main() {
  run().unwrap();
}
