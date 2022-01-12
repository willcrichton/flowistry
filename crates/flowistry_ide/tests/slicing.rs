use flowistry::infoflow::Direction;
use test_env_log::test;
use utils::{run_tests, slice};

mod utils;

#[test]
fn test_backward_slice() {
  run_tests("backward_slice", |path, expected| {
    slice(path, expected, Direction::Backward)
  });
}

#[test]
fn test_forward_slice() {
  run_tests("forward_slice", |path, expected| {
    slice(path, expected, Direction::Forward)
  });
}

#[test]
fn test_extensions() {
  run_tests("extensions", |path, expected| {
    slice(path, expected, Direction::Backward)
  });
}
