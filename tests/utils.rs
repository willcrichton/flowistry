use anyhow::Result;
use std::collections::HashSet;
use std::io::Write;
use tempfile::NamedTempFile;

use rust_slicer::{Config, Range, SliceOutput};

fn check_lines(expected: Vec<usize>, actual: SliceOutput, src: &str) {
  let expected = expected.into_iter().collect::<HashSet<_>>();
  let mut in_slice = HashSet::new();

  for range in actual.ranges() {
    let range_slice = expected
      .clone()
      .into_iter()
      .filter(|line| range.start_line <= *line && *line <= range.end_line)
      .collect::<HashSet<_>>();

    if range_slice.len() == 0 {
      panic!("Unexpected slice:\n {} ({:?})", range.substr(src), range);
    }

    in_slice = &in_slice | &range_slice;
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

pub fn run(src: impl AsRef<str>, mut range: Range, lines: Vec<usize>) {
  let lines = lines.into_iter().map(|i| i - 1).collect::<Vec<_>>();
  let inner = move || -> Result<()> {
    let sysroot = "/Users/will/Code/rust/build/x86_64-apple-darwin/stage1";
    let mut f = NamedTempFile::new()?;
    let src = src.as_ref().trim();
    f.as_file_mut().write(src.as_bytes())?;

    range.start_line -= 1;
    range.end_line -= 1;
    range.start_col -= 1;
    range.end_col -= 1;

    let config = Config {
      range,
      debug: false,
    };

    let path = f.path();
    let args = format!(
      "--edition=2018 --crate-name tmp {} -A warnings --sysroot {}",
      path.display(),
      sysroot
    );

    let output = rust_slicer::slice(config, args)?;
    check_lines(lines, output, &src);

    f.close()?;
    Ok(())
  };

  inner().unwrap();
}