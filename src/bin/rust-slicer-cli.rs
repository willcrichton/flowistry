use anyhow::{bail, Context, Result};
use clap::clap_app;
use regex::Regex;
use rust_slicer::{Config, Range};
use serde::Serialize;
use std::collections::HashSet;
use std::process::Command;

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>,
}

fn run() -> Result<()> {
  let matches = clap_app!(app =>
    (@arg debug: -d)
    (@arg path:)
    (@arg sysroot:)
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

  let metadata_bytes = Command::new("cargo").args(&["metadata"]).output()?.stdout;
  let metadata_json: serde_json::Value = serde_json::from_slice(&metadata_bytes)?;
  let workspace_members = metadata_json
    .as_object()
    .unwrap()
    .get("workspace_members")
    .unwrap()
    .as_array()
    .unwrap();
  if workspace_members.len() > 1 {
    bail!("Not implemented for workspace with more than 1 member");
  }

  let crate_name = workspace_members[0].as_str().unwrap().split(" ").nth(0).unwrap();

  let cargo_output = {
    let command = format!("rm -r target/debug/.fingerprint/{}*", crate_name);
    Command::new("bash").args(&["-c", &command]).output()?;

    let stderr = Command::new("cargo")
      .args(&["check", "-v"])
      .env("RUSTFLAGS", "-A warnings")
      .output()?
      .stderr;

    String::from_utf8(stderr)?
  };

  // Extract the `rustc` commands from the "Running `...`" stderr output.
  let command_lines = {
    let re = Regex::new(r"^\s*Running `(.*)`").unwrap();
    cargo_output
      .split("\n")
      .filter_map(|line| {
        re.captures(line)
          .map(|cap| cap.get(1).unwrap().as_str().to_string())
      })
      .collect::<Vec<_>>()
  };

  if command_lines.len() == 0 {
    bail!(
      r#"Failed to scrape rustc commands from Cargo. 
  Detected crate name was `{}` 
  Output of check -v was:
{}"#,
      crate_name,
      cargo_output
    );
  }

  let mut args = command_lines[0]
    .split(" ")
    .filter(|s| *s != "--error-format=json" && *s != "--json=diagnostic-rendered-ansi")
    //.map(|s| if s.ends_with(".rs") { path } else { s })
    .chain(vec!["--sysroot", arg!("sysroot")])
    .collect::<Vec<_>>();

  let remove_flags = vec!["--cfg", "--crate-type"]
    .into_iter()
    .collect::<HashSet<_>>();
  let to_remove = args
    .iter()
    .enumerate()
    .filter(|(_, s)| remove_flags.contains(*s))
    .map(|(i, _)| i)
    .collect::<Vec<_>>();
  for i in to_remove.into_iter().rev() {
    args.remove(i + 1);
    args.remove(i);
  }

  args.extend_from_slice(&["--crate-type", "lib"]);

  //println!("{:#?}", args);

  let config = Config {
    path: arg!("path").to_owned(),
    range: Range {
      start_line: arg!("start_line").parse::<usize>()?,
      start_col: arg!("start_col").parse::<usize>()?,
      end_line: arg!("end_line").parse::<usize>()?,
      end_col: arg!("end_col").parse::<usize>()?,
    },
    debug: matches.is_present("debug"),
  };

  let output = rust_slicer::slice(config, args.join(" "))?;
  let cli_output = SliceOutput {
    ranges: output.ranges().clone(),
  };
  println!("{}", serde_json::to_string(&cli_output)?);

  Ok(())
}

fn main() {
  run().unwrap();
}
