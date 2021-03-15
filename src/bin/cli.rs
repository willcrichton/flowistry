use anyhow::{Context, Result};
use clap::clap_app;
use rust_slicer::{Config, Range};
use serde::Serialize;

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>,
}

fn run() -> Result<()> {
  let sysroot = "/Users/will/Code/rust/build/x86_64-apple-darwin/stage1".to_string();
  // let sysroot = String::from_utf8(
  //   Command::new("rustc")
  //     .args(&["--print", "sysroot"])
  //     .output()
  //     .unwrap()
  //     .stdout,
  // )
  // .unwrap()
  // .trim()
  // .to_string();

  let matches = clap_app!(app =>
    (@arg debug: -d)
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

  let args = format!(
    "--edition=2018 {path} --sysroot {sysroot}",
    path = arg!("path"),
    sysroot = sysroot
  );

  let config = Config {
    range: Range {
      start_line: arg!("start_line").parse::<usize>()?,
      start_col: arg!("start_col").parse::<usize>()?,
      end_line: arg!("end_line").parse::<usize>()?,
      end_col: arg!("end_col").parse::<usize>()?,
    },
    debug: matches.is_present("debug"),
  };

  let output = rust_slicer::slice(config, args)?;
  let cli_output = SliceOutput {
    ranges: output.ranges().clone(),
  };
  println!("{}", serde_json::to_string(&cli_output)?);

  Ok(())
}

fn main() {
  run().unwrap();
}
