use anyhow::{Context, Result};
use clap::clap_app;
use generate_rustc_flags::{generate_rustc_flags, CliFeatures};
use log::debug;
use rust_slicer::{
  config::{ContextMode, EvalMode, MutabilityMode, PointerMode},
  Config, Range,
};
use serde::Serialize;
use std::env;

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>,
}

fn run() -> Result<()> {
  let _ = env_logger::try_init();

  let matches = clap_app!(app =>
    (@arg debug: -d)
    (@arg nomut: --nomut)
    (@arg recurse: --recurse)
    (@arg conserv: --conserv)
    (@arg path:)
    (@arg start_line:)
    (@arg start_col:)
    (@arg end_line:)
    (@arg end_col:)
  )
  .get_matches();

  macro_rules! arg {
    ($key:expr) => {
      matches.value_of($key).context($key)?
    };
  }

  let features = CliFeatures::from_command_line(&[], false, true)?;
  let (flags, env) = generate_rustc_flags(arg!("path"), features, false)?;
  for (k, v) in env {
    env::set_var(k, v);
  }

  debug!("Generated rustc command:\n{}", flags.join(" "));

  let config = Config {
    range: Range {
      start_line: arg!("start_line").parse::<usize>()?,
      start_col: arg!("start_col").parse::<usize>()?,
      end_line: arg!("end_line").parse::<usize>()?,
      end_col: arg!("end_col").parse::<usize>()?,
      filename: arg!("path").to_owned(),
    },
    debug: matches.is_present("debug"),
    eval_mode: EvalMode {
      mutability_mode: if matches.is_present("nomut") {
        MutabilityMode::IgnoreMut
      } else {
        MutabilityMode::DistinguishMut
      },
      context_mode: if matches.is_present("recurse") {
        ContextMode::Recurse
      } else {
        ContextMode::SigOnly
      },
      pointer_mode: if matches.is_present("conserv") {
        PointerMode::Conservative
      } else {
        PointerMode::Precise
      },
    },
  };

  let output = rust_slicer::slice(config, &flags)?;
  let cli_output = SliceOutput {
    ranges: output.ranges().clone(),
  };
  println!("{}", serde_json::to_string(&cli_output)?);

  Ok(())
}

fn main() {
  run().unwrap();
}
