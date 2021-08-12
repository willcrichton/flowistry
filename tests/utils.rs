use anyhow::{anyhow, bail, Result};
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

use flowistry::{Config, Range};

fn parse_ranges(
  prog: &str,
  delimiters: Vec<(&'static str, &'static str)>,
  filename: &str,
) -> Result<(String, HashMap<&'static str, Vec<Range>>)> {
  let mut in_idx = 0;
  let mut out_idx = 0;
  let mut buf = Vec::new();
  let bytes = prog.bytes().collect::<Vec<_>>();
  let mut stack = vec![];

  let (opens, closes): (Vec<_>, Vec<_>) = delimiters.into_iter().unzip();
  let mut ranges = HashMap::new();

  macro_rules! check_token {
    ($tokens:expr) => {
      $tokens
        .iter()
        .find(|t| {
          in_idx + t.len() <= bytes.len() && t.as_bytes() == &bytes[in_idx..in_idx + t.len()]
        })
        .map(|t| *t)
    };
  }

  while in_idx < bytes.len() {
    if let Some(open) = check_token!(&opens) {
      stack.push((out_idx, open));
      in_idx += open.len();
      continue;
    }

    if let Some(close) = check_token!(&closes) {
      let (start, delim) = stack
        .pop()
        .ok_or_else(|| anyhow!("Missing open delimiter for \"{}\"", close))?;
      ranges.entry(delim).or_insert_with(Vec::new).push(Range {
        start,
        end: out_idx,
        filename: filename.to_owned(),
      });
      in_idx += close.len();
      continue;
    }

    buf.push(bytes[in_idx]);
    in_idx += 1;
    out_idx += 1;
  }

  if stack.len() > 0 {
    bail!("Unclosed delimiters: {:?}", stack);
  }

  let prog_clean = String::from_utf8(buf)?;
  return Ok((prog_clean, ranges));
}

fn color_ranges(prog: &str, all_ranges: Vec<(&str, &HashSet<Range>)>) -> String {
  let mut new_tokens = all_ranges
    .iter()
    .map(|(_, ranges)| {
      ranges
        .iter()
        .map(|range| {
          let contained = all_ranges.iter().any(|(_, ranges)| {
            ranges
              .iter()
              .any(|other| range != other && other.start <= range.end && range.end < other.end)
          });
          let end_marker = if contained { "]" } else { "\x1B[0m]" };
          vec![("[\x1B[31m", range.start), (end_marker, range.end)].into_iter()
        })
        .flatten()
    })
    .flatten()
    .collect::<Vec<_>>();
  new_tokens.sort_by_key(|(_, i)| -(*i as isize));

  let mut output = prog.to_owned();
  for (s, i) in new_tokens {
    output.insert_str(i, s);
  }

  return output;
}

fn compare_ranges(expected: HashSet<Range>, actual: HashSet<Range>, prog: &str) {
  let missing = &expected - &actual;
  let extra = &actual - &expected;

  let fmt_ranges = |s: &HashSet<Range>| textwrap::indent(&color_ranges(prog, vec![("", s)]), "  ");

  let check = |s: HashSet<Range>, message: &str| {
    if s.len() > 0 {
      println!("In program:\n{}", textwrap::indent(prog.trim(), "  "));
      println!("Expected ranges:\n{}", fmt_ranges(&expected));
      println!("Actual ranges:\n{}", fmt_ranges(&actual));
      panic!("{} ranges:\n{}", message, fmt_ranges(&s));
    }
  };

  check(missing, "Missing");
  check(extra, "Extra");
}

pub fn backward_slice(prog: &str) {
  let inner = move || -> Result<()> {
    let mut f = NamedTempFile::new()?;
    let filename = f.path().to_string_lossy().to_string();

    let (prog_clean, ranges) = parse_ranges(prog, vec![("`[", "]`"), ("`(", ")`")], &filename)?;
    let range = ranges["`("][0].clone();
    let mut expected = ranges["`["].clone().into_iter().collect::<HashSet<_>>();
    expected.insert(range.clone());

    f.as_file_mut().write(prog_clean.as_bytes())?;

    let config = Config {
      range: range.clone(),
      ..Default::default()
    };

    let args = format!(
      "--edition=2018 --crate-name tmp {} -A warnings --sysroot {}",
      f.path().display(),
      *SYSROOT
    );

    let args = args.split(" ").map(|s| s.to_owned()).collect::<Vec<_>>();

    let output = flowistry::slice(config, &args)?;
    let actual = output.ranges().into_iter().cloned().collect::<HashSet<_>>();

    compare_ranges(expected, actual, &prog_clean);

    Ok(())
  };

  inner().unwrap();
}

// fn check_lines(expected: Vec<usize>, actual: SliceOutput, src: &str, filename: String) {
//   let expected = expected.into_iter().collect::<HashSet<_>>();
//   let mut in_slice = HashSet::new();

//   for range in actual.ranges() {
//     if range.filename != filename {
//       continue;
//     }

//     let lines = range.start_line..=range.end_line;
//     if !lines.clone().all(|line| expected.contains(&line)) {
//       panic!("Unexpected slice:\n {} ({:?})", range.substr(src), range);
//     }

//     in_slice = &in_slice | &lines.collect::<HashSet<_>>();
//   }

//   let expected_not_in_slice = &expected - &in_slice;
//   if expected_not_in_slice.len() > 0 {
//     panic!(
//       "Slice did not include expected lines:\n {:?}",
//       expected_not_in_slice
//         .into_iter()
//         .map(|expected| src.split("\n").nth(expected).unwrap())
//         .collect::<Vec<_>>()
//     );
//   }
// }

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

// pub fn run(src: impl AsRef<str>, mut range: Range, lines: Vec<usize>) {
//   let lines = lines.into_iter().map(|i| i - 1).collect::<Vec<_>>();
//   let inner = move || -> Result<()> {
//     let mut f = NamedTempFile::new()?;
//     let src = src.as_ref().trim();
//     f.as_file_mut().write(src.as_bytes())?;

//     let path = f.path();
//     range.filename = path.to_string_lossy().to_string();

//     range.start_line -= 1;
//     range.end_line -= 1;
//     range.start_col -= 1;
//     range.end_col -= 1;

//     let config = Config {
//       range,
//       debug: false,
//       ..Default::default()
//     };

//     let args = format!(
//       "--edition=2018 --crate-name tmp {} -A warnings --sysroot {}",
//       path.display(),
//       *SYSROOT
//     );

//     let args = args.split(" ").map(|s| s.to_owned()).collect::<Vec<_>>();

//     let output = flowistry::slice(config, &args)?;
//     check_lines(lines, output, &src, path.to_string_lossy().into_owned());

//     f.close()?;
//     Ok(())
//   };

//   inner().unwrap();
// }
