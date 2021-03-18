use anyhow::Result;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

use rust_slicer::{Config, Range, SliceOutput};

fn check_lines(expected: Vec<usize>, actual: SliceOutput, src: &str) {
  let expected = expected.into_iter().collect::<HashSet<_>>();
  let mut in_slice = HashSet::new();

  for range in actual.ranges() {
    let lines = range.start_line..=range.end_line;
    if !lines.clone().all(|line| expected.contains(&line)) {
      panic!("Unexpected slice:\n {} ({:?})", range.substr(src), range);
    }

    in_slice = &in_slice | &lines.collect::<HashSet<_>>();
  }

  let expected_not_in_slice = &expected - &in_slice;
  if expected_not_in_slice.len() > 0 {
    panic!(
      "Slice did not include expected lines:\n {:?}",
      expected_not_in_slice
        .into_iter()
        .map(|expected| src.split("\n").nth(expected).unwrap())
        .collect::<Vec<_>>()
    );
  }
}

lazy_static! {
  static ref SYSROOT: String = String::from_utf8(
    Command::new("rustc")
      .args(&["--print", "sysroot"])
      .output()
      .unwrap()
      .stdout
  )
  .unwrap()
  .trim()
  .to_owned();
}

pub fn run(src: impl AsRef<str>, mut range: Range, lines: Vec<usize>) {
  let lines = lines.into_iter().map(|i| i - 1).collect::<Vec<_>>();
  let inner = move || -> Result<()> {
    let mut f = NamedTempFile::new()?;
    let src = src.as_ref().trim();
    f.as_file_mut().write(src.as_bytes())?;

    range.start_line -= 1;
    range.end_line -= 1;
    range.start_col -= 1;
    range.end_col -= 1;

    let path = f.path();
    let config = Config {
      range,
      path: path.to_string_lossy().to_string(),
      debug: false,
    };

    let args = format!(
      "--edition=2018 --crate-name tmp {} -A warnings --sysroot {}",
      path.display(),
      *SYSROOT
    );

    let output = rust_slicer::slice(config, args)?;
    check_lines(lines, output, &src);

    f.close()?;
    Ok(())
  };

  inner().unwrap();
}
