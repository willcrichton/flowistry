//! Running rustc and Flowistry in tests.

#![allow(missing_docs)]

use std::{cell::RefCell, fs, io, panic, path::Path};

use anyhow::Result;
use fluid_let::fluid_set;
use log::info;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_span::Span;
pub use rustc_utils::test_utils::{compare_ranges, fmt_ranges, parse_ranges};
use rustc_utils::{
  mir::borrowck_facts,
  source_map::{
    range::{ByteRange, CharPos, ToSpan},
    spanner::Spanner,
  },
  test_utils::{self, CompileBuilder},
};

use crate::{
  extensions::{ContextMode, EVAL_MODE, EvalMode, MutabilityMode, PointerMode},
  infoflow,
};

pub fn compile_body_with_range(
  input: impl Into<String>,
  compute_target: impl FnOnce() -> ByteRange + Send,
  callback: impl for<'tcx> FnOnce(
    TyCtxt<'tcx>,
    BodyId,
    &'tcx BodyWithBorrowckFacts<'tcx>,
    ByteRange,
  ) + Send,
) {
  borrowck_facts::enable_mir_simplification();
  CompileBuilder::new(input).compile(|result| {
    let target = compute_target();
    let tcx = result.tcx;
    let (body_id, body_with_facts) = result.as_body_with_range(target);
    callback(tcx, body_id, body_with_facts, target)
  })
}

pub fn compile_body(
  input: impl Into<String>,
  callback: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &BodyWithBorrowckFacts<'tcx>) + Send,
) {
  borrowck_facts::enable_mir_simplification();
  test_utils::compile_body(input, callback)
}

pub fn bless(
  tcx: TyCtxt,
  path: &Path,
  contents: String,
  actual: HashSet<ByteRange>,
) -> Result<()> {
  let mut delims = actual
    .into_iter()
    .flat_map(|byte_range| {
      let char_range = byte_range.as_char_range(tcx.sess.source_map());
      dbg!((byte_range, char_range));
      [("`[", char_range.start), ("]`", char_range.end)]
    })
    .collect::<Vec<_>>();
  delims.sort_by_key(|(_, i)| (i.line, i.column));

  let output = RefCell::new(String::new());
  let mut flush = |pos: CharPos| {
    while !delims.is_empty() && delims[0].1 == pos {
      let (delim, _) = delims.remove(0);
      output.borrow_mut().push_str(delim);
    }
  };

  let line_count = contents.lines().count();
  for (line, line_str) in contents.lines().enumerate() {
    for (column, chr) in line_str.chars().enumerate() {
      flush(CharPos { line, column });
      output.borrow_mut().push(chr);
    }
    flush(CharPos {
      line,
      column: line_str.chars().count(),
    });
    if line != line_count - 1 {
      output.borrow_mut().push('\n');
    }
  }

  fs::write(path.with_extension("txt.expected"), output.into_inner())?;

  Ok(())
}

pub fn test_command_output(
  path: &Path,
  expected: Option<&Path>,
  output_fn: impl for<'a, 'tcx> Fn(
    infoflow::FlowResults<'a, 'tcx>,
    Spanner<'tcx>,
    Span,
  ) -> Vec<Span>
  + Send
  + Sync,
) {
  let inner = move || -> Result<()> {
    info!("Testing {}", path.file_name().unwrap().to_string_lossy());
    let input = String::from_utf8(fs::read(path)?)?;

    // We have to do a hacky thing where we call `parse_ranges` twice.
    // Once to clean up the input to pass to rustc to start the session.
    // A second time to get the `ByteRange`s, which *must* happen *within*
    // the session thread bc filenames are interned.
    let (input_clean, _) = parse_ranges(&input, vec![("`(", ")`")])?;
    compile_body_with_range(
      input_clean.clone(),
      || {
        let (_, input_ranges) = parse_ranges(&input, vec![("`(", ")`")]).unwrap();
        input_ranges["`("][0]
      },
      |tcx, body_id, body_with_facts, target: ByteRange| {
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

        let target = target.to_span(tcx).unwrap();
        let results = infoflow::compute_flow(tcx, body_id, body_with_facts);
        let spanner = Spanner::new(tcx, body_id, &body_with_facts.body);

        let actual = output_fn(results, spanner, target)
          .into_iter()
          .map(|span| ByteRange::from_span(span, tcx.sess.source_map()))
          .collect::<Result<HashSet<_>>>()
          .unwrap();

        match expected {
          Some(expected_path) => {
            let expected_file = fs::read_to_string(expected_path);
            match expected_file {
              Ok(file) => {
                let (_output_clean, output_ranges) =
                  parse_ranges(&file, vec![("`[", "]`")]).unwrap();

                let expected = match output_ranges.get("`[") {
                  Some(ranges) => ranges.clone().into_iter().collect::<HashSet<_>>(),
                  None => HashSet::default(),
                };

                compare_ranges(&expected, &actual, &input_clean);
              }
              Err(err) if matches!(err.kind(), io::ErrorKind::NotFound) => {
                println!("{}", fmt_ranges(&input_clean, &actual));
                panic!("Expected file not generated yet.");
              }
              err => {
                err.unwrap();
              }
            }
          }
          None => {
            bless(tcx, path, input_clean, actual).unwrap();
          }
        }
      },
    );

    Ok(())
  };

  inner().unwrap();
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
