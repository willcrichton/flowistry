use anyhow::{Context, Result};
use clap::clap_app;
use generate_rustc_flags::{generate_rustc_flags, CliFeatures};
use log::debug;
use rust_slicer::{
  config::{ContextMode, EvalMode, MutabilityMode, PointerMode},
  Config, Range,
};
use std::env;

fn run() -> Result<()> {
  let _ = env_logger::try_init();

  let matches = clap_app!(app =>
    (@arg debug: -d)
    (@arg nomut: --nomut)
    (@arg recurse: --recurse)
    (@arg conserv: --conserv)
    (@arg local: --local +takes_value)
    (@arg features: --features +takes_value)
    (@arg all_features: --("all-features"))
    (@arg workspace: --workspace +takes_value)
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

  if matches.is_present("workspace") {
    env::set_current_dir(arg!("workspace"))?;
  }

  let features = if matches.is_present("features") {
    arg!("features")
      .split(",")
      .map(|s| s.to_owned())
      .collect::<Vec<_>>()
  } else {
    vec![]
  };
  let features =
    CliFeatures::from_command_line(&features, matches.is_present("all_features"), true)?;
  let flags = generate_rustc_flags(arg!("path"), features, false)?;

  debug!("Generated rustc command:\n{}", flags.join(" "));

  let config = Config {
    range: Range {
      start_line: arg!("start_line").parse::<usize>()?,
      start_col: arg!("start_col").parse::<usize>()?,
      end_line: arg!("end_line").parse::<usize>()?,
      end_col: arg!("end_col").parse::<usize>()?,
      filename: arg!("path").to_owned(),
    },
    local: if matches.is_present("local") {
      Some(arg!("local").parse::<usize>()?)
    } else {
      None
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
  println!("{}", serde_json::to_string(&output)?);

  Ok(())
}

fn main() {
  run().unwrap();
}
