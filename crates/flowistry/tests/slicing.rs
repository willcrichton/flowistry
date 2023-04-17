#![feature(rustc_private)]

extern crate rustc_span;

use flowistry::{
  infoflow::{self, Direction},
  test_utils,
};
use rustc_span::Span;
use rustc_utils::SpanExt;
use test_log::test;

fn slice(dir: &str, direction: Direction) {
  test_utils::run_tests(dir, |path, expected| {
    test_utils::test_command_output(path, expected, |results, spanner, target| {
      let places = spanner.span_to_places(target);
      let targets = places
        .iter()
        .map(|mir_span| {
          mir_span
            .locations
            .iter()
            .map(|location| (mir_span.place, *location))
            .collect::<Vec<_>>()
        })
        .collect();
      log::debug!("targets={targets:#?}");

      let deps =
        infoflow::compute_dependency_spans(&results, targets, direction, &spanner);

      Span::merge_overlaps(deps.into_iter().flatten().collect())
    });
  });
}

#[test]
fn test_backward_slice() {
  slice("backward_slice", Direction::Backward);
}

#[test]
fn test_forward_slice() {
  slice("forward_slice", Direction::Forward);
}

#[test]
fn test_extensions() {
  slice("extensions", Direction::Backward);
}
