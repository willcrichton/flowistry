use std::{env, fs, path::Path};

use anyhow::Result;
use flowistry::infoflow::Direction;
use test_env_log::test;
use utils::slice;

mod utils;

const BLESS: bool = option_env!("BLESS").is_some();
const ONLY: Option<&'static str> = option_env!("ONLY");

fn run_tests(dir: impl AsRef<Path>, direction: Direction) {
  let main = || -> Result<()> {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("tests")
      .join(dir.as_ref());
    let tests = fs::read_dir(test_dir)?;
    for test in tests {
      let test = test?.path();
      if test.extension().unwrap() == "expected" {
        continue;
      }
      if let Some(only) = ONLY {
        if !test.file_name().unwrap().to_str().unwrap().contains(only) {
          continue;
        }
      }
      let expected_path = test.with_extension("txt.expected");
      let expected = (!BLESS).then(|| expected_path.as_ref());
      slice(&test, expected, direction);
    }
    Ok(())
  };

  main().unwrap();
}

#[test]
fn test_backward_slice() {
  run_tests("backward_slice", Direction::Backward);
}

#[test]
fn test_forward_slice() {
  run_tests("forward_slice", Direction::Forward);
}

#[test]
fn test_extensions() {
  run_tests("extensions", Direction::Backward);
}
