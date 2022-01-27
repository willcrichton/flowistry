use std::{
  collections::{HashMap, HashSet},
  fmt::Debug,
  fs,
  io::Write,
  panic,
  path::Path,
  process::Command,
};

use anyhow::Result;
use flowistry::{
  extensions::{ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE},
  infoflow::Direction,
  test_utils::parse_ranges,
};
use flowistry_ide::{
  analysis::FlowistryResult,
  range::{FunctionIdentifier, Range},
};
use fluid_let::fluid_set;
use lazy_static::lazy_static;
use log::info;
use tempfile::NamedTempFile;

fn color_ranges(prog: &str, all_ranges: Vec<(&str, &HashSet<Range>)>) -> String {
  let mut new_tokens = all_ranges
    .iter()
    .map(|(_, ranges)| {
      ranges
        .iter()
        .map(|range| {
          let contained = all_ranges.iter().any(|(_, ranges)| {
            ranges.iter().any(|other| {
              range != other && other.start <= range.end && range.end < other.end
            })
          });
          let end_marker = if contained { "]" } else { "\x1B[0m]" };
          [("[\x1B[31m", range.start), (end_marker, range.end)]
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

  let fmt_ranges =
    |s: &HashSet<Range>| textwrap::indent(&color_ranges(prog, vec![("", s)]), "  ");

  let check = |s: HashSet<Range>, message: &str| {
    if s.len() > 0 {
      println!("Expected ranges:\n{}", fmt_ranges(&expected));
      println!("Actual ranges:\n{}", fmt_ranges(&actual));
      panic!("{message} ranges:\n{}", fmt_ranges(&s));
    }
  };

  check(missing, "Analysis did NOT have EXPECTED");
  check(extra, "Actual DID have UNEXPECTED");
}

pub fn flow<O: Debug>(
  prog: &str,
  id: FunctionIdentifier,
  cb: impl FnOnce(FunctionIdentifier, &[String]) -> FlowistryResult<O>,
) {
  let inner = move || -> Result<()> {
    let mut f = NamedTempFile::new()?;
    let _filename = f.path().to_string_lossy().to_string();
    f.as_file_mut().write(prog.as_bytes())?;

    let args = format!(
      "--edition=2018 --crate-name tmp {} -A warnings --sysroot {}",
      f.path().display(),
      *SYSROOT
    );
    let args = args.split(" ").map(|s| s.to_owned()).collect::<Vec<_>>();

    let output = cb(id, &args);
    println!("{:?}", output.unwrap());

    Ok(())
  };

  inner().unwrap();
}

fn bless(path: &Path, contents: String, actual: HashSet<Range>) -> Result<()> {
  let mut delims = actual
    .into_iter()
    .map(|range| [("`[", range.start), ("]`", range.end)])
    .flatten()
    .collect::<Vec<_>>();
  delims.sort_by_key(|(_, i)| *i);

  let mut output = String::new();
  for (i, ch) in contents.chars().enumerate() {
    while delims.len() > 0 && delims[0].1 == i {
      let (delim, _) = delims.remove(0);
      output.push_str(delim);
    }
    output.push(ch);
  }

  fs::write(path.with_extension("txt.expected"), output)?;

  Ok(())
}

pub fn test_command_output(
  path: &Path,
  expected: Option<&Path>,
  output_fn: impl Fn(Range, &[String]) -> Vec<Range>,
) {
  let inner = move || -> Result<()> {
    info!("Testing {}", path.file_name().unwrap().to_string_lossy());
    let input = String::from_utf8(fs::read(path)?)?;

    let mut f = NamedTempFile::new()?;
    let filename = f.path().to_string_lossy().to_string();

    let parse_range_map = |src, delims| -> Result<_> {
      let (clean, parsed_ranges) = parse_ranges(src, delims)?;
      let map = parsed_ranges
        .into_iter()
        .map(|(k, vs)| {
          (
            k,
            vs.into_iter()
              .map(|(start, end)| Range {
                start,
                end,
                filename: filename.to_string(),
              })
              .collect::<Vec<_>>(),
          )
        })
        .collect::<HashMap<_, _>>();
      Ok((clean, map))
    };

    let (input_clean, input_ranges) = parse_range_map(&input, vec![("`(", ")`")])?;
    let range = input_ranges["`("][0].clone();

    f.as_file_mut().write(input_clean.as_bytes())?;

    let args = format!(
      "rustc --crate-name tmp --edition=2018 {} -A warnings --sysroot {}",
      f.path().display(),
      *SYSROOT
    );

    let args = args.split(" ").map(|s| s.to_owned()).collect::<Vec<_>>();

    let header = input.lines().next().unwrap();
    let mut mode = EvalMode::default();
    if header.starts_with("/*") {
      if header.contains("recurse") {
        mode.context_mode = ContextMode::Recurse;
      }
      if header.contains("ignoremut") {
        mode.mutability_mode = MutabilityMode::IgnoreMut;
      }
      if header.contains("conservative") {
        mode.pointer_mode = PointerMode::Conservative;
      }
    }

    fluid_set!(EVAL_MODE, &mode);
    let actual = output_fn(range, &args).into_iter().collect::<HashSet<_>>();

    match expected {
      Some(expected_path) => {
        let output = String::from_utf8(fs::read(expected_path)?)?;
        let (_output_clean, output_ranges) =
          parse_range_map(&output, vec![("`[", "]`")])?;

        let expected = output_ranges["`["]
          .clone()
          .into_iter()
          .collect::<HashSet<_>>();
        compare_ranges(expected, actual, &input_clean);
      }
      None => {
        bless(path, input_clean, actual)?;
      }
    }

    Ok(())
  };

  inner().unwrap();
}

pub fn slice(path: &Path, expected: Option<&Path>, direction: Direction) {
  test_command_output(path, expected, |range, args| {
    flowistry_ide::slicing::slice(direction, range, &args)
      .unwrap()
      .ranges
  });
}

pub fn find_mutations(path: &Path, expected: Option<&Path>) {
  test_command_output(path, expected, |range, args| {
    flowistry_ide::mutations::find(range, args).unwrap().ranges
  });
}

pub fn effects(prog: &str, qpath: &str) {
  flow(
    prog,
    FunctionIdentifier::Qpath(qpath.to_owned()),
    flowistry_ide::effects::effects,
  );
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

const BLESS: bool = option_env!("BLESS").is_some();
const ONLY: Option<&'static str> = option_env!("ONLY");
const EXIT: bool = option_env!("EXIT").is_some();

pub fn run_tests(
  dir: impl AsRef<Path>,
  test_fn: impl Fn(&Path, Option<&Path>) + std::panic::RefUnwindSafe,
) {
  let main = || -> Result<()> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("tests")
      .join(dir.as_ref());
    let tests = fs::read_dir(test_dir)?;
    let mut failed = false;
    for test in tests {
      let test = test?.path();
      if test.extension().unwrap() == "expected" {
        continue;
      }
      let test_name = test.file_name().unwrap().to_str().unwrap();
      if let Some(only) = ONLY {
        if !test_name.contains(only) {
          continue;
        }
      }
      let expected_path = test.with_extension("txt.expected");
      let expected = (!BLESS).then(|| expected_path.as_ref());

      let result = panic::catch_unwind(|| test_fn(&test, expected));
      if let Err(e) = result {
        if EXIT {
          panic!("{test_name}:\n{e:?}");
        } else {
          failed = true;
          eprintln!("\n\n{test_name}:\n{e:?}\n\n");
        }
      }
    }

    if failed {
      panic!("Tests failed.")
    }

    Ok(())
  };

  main().unwrap();
}
