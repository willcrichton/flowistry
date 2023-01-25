//! Running rustc and Flowistry in tests.

use std::{fs, io, panic, path::Path, process::Command};

use anyhow::{anyhow, bail, Context, Result};
use fluid_let::fluid_set;
use log::{debug, info};
use rustc_borrowck::BodyWithBorrowckFacts;
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::{BodyId, ItemKind};
use rustc_middle::ty::TyCtxt;
use rustc_span::{source_map::FileLoader, Span, SyntaxContext};

use crate::{
  extensions::{ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE},
  indexed::impls::{Filename, FilenameIndex},
  infoflow,
  mir::{borrowck_facts, utils::BodyExt},
  source_map::{
    find_enclosing_bodies, BytePos, ByteRange, CharPos, CharRange, Spanner, ToSpan,
  },
};

struct StringLoader(String);
impl FileLoader for StringLoader {
  fn file_exists(&self, _: &Path) -> bool {
    true
  }
  fn read_file(&self, _: &Path) -> io::Result<String> {
    Ok(self.0.clone())
  }
}

pub const DUMMY_FILE_NAME: &str = "dummy.rs";

lazy_static::lazy_static! {
  static ref SYSROOT: String = {
    let rustc_output =
      Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap()
        .stdout;
    String::from_utf8(rustc_output)
      .unwrap()
      .trim()
      .to_owned()
  };
  pub static ref DUMMY_FILE: FilenameIndex = Filename::intern(DUMMY_FILE_NAME);
  pub static ref DUMMY_BYTE_RANGE: ByteRange = ByteRange {
    start: BytePos(0),
    end: BytePos(0),
    filename: *DUMMY_FILE,
  };
  pub static ref DUMMY_CHAR_RANGE: CharRange = CharRange {
    start: CharPos(0),
    end: CharPos(0),
    filename: *DUMMY_FILE,
  };
}

pub fn compile_body_with_range(
  input: impl Into<String>,
  target: ByteRange,
  callback: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &BodyWithBorrowckFacts<'tcx>) + Send,
) {
  compile(input, |tcx| {
    let body_id = find_enclosing_bodies(tcx, target.to_span(tcx).unwrap())
      .next()
      .unwrap();
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

    callback(tcx, body_id, body_with_facts);
  })
}

pub fn compile_body(
  input: impl Into<String>,
  callback: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &BodyWithBorrowckFacts<'tcx>) + Send,
) {
  compile(input, |tcx| {
    let hir = tcx.hir();
    let body_id = hir
      .items()
      .filter_map(|id| match hir.item(id).kind {
        ItemKind::Fn(_, _, body) => Some(body),
        _ => None,
      })
      .next()
      .unwrap();

    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

    callback(tcx, body_id, body_with_facts);
  })
}

pub fn compile(input: impl Into<String>, callback: impl FnOnce(TyCtxt<'_>) + Send) {
  let mut callbacks = TestCallbacks {
    callback: Some(callback),
  };
  let args = format!(
    "rustc {DUMMY_FILE_NAME} --crate-type lib --edition=2021 -Z identify-regions -Z mir-opt-level=0 -Z maximal-hir-to-mir-coverage --allow warnings --sysroot {}",
    &*SYSROOT
  );
  let args = args.split(' ').map(|s| s.to_string()).collect::<Vec<_>>();

  rustc_driver::catch_fatal_errors(|| {
    let mut compiler = rustc_driver::RunCompiler::new(&args, &mut callbacks);
    compiler.set_file_loader(Some(Box::new(StringLoader(input.into()))));
    compiler.run()
  })
  .unwrap()
  .unwrap();
}

struct TestCallbacks<Cb> {
  callback: Option<Cb>,
}

impl<Cb> rustc_driver::Callbacks for TestCallbacks<Cb>
where
  Cb: FnOnce(TyCtxt<'_>),
{
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let callback = self.callback.take().unwrap();
      callback(tcx);
    });
    rustc_driver::Compilation::Stop
  }
}

pub type RangeMap = HashMap<&'static str, Vec<ByteRange>>;

pub fn parse_ranges(
  src: impl AsRef<str>,
  delimiters: impl AsRef<[(&'static str, &'static str)]>,
) -> Result<(String, RangeMap)> {
  let src = src.as_ref();
  let delimiters = delimiters.as_ref();

  let mut in_idx = 0;
  let mut out_idx = 0;
  let mut buf = Vec::new();
  let bytes = src.bytes().collect::<Vec<_>>();
  let mut stack = vec![];

  let (opens, closes): (Vec<_>, Vec<_>) = delimiters.iter().copied().unzip();
  let mut ranges: HashMap<_, Vec<_>> = HashMap::default();

  macro_rules! check_token {
    ($tokens:expr) => {
      $tokens
        .iter()
        .find(|t| {
          in_idx + t.len() <= bytes.len()
            && t.as_bytes() == &bytes[in_idx .. in_idx + t.len()]
        })
        .map(|t| *t)
    };
  }

  while in_idx < bytes.len() {
    if let Some(open) = check_token!(opens) {
      stack.push((out_idx, open));
      in_idx += open.len();
      continue;
    }

    if let Some(close) = check_token!(closes) {
      let (start, delim) = stack
        .pop()
        .with_context(|| anyhow!("Missing open delimiter for \"{close}\""))?;
      ranges.entry(delim).or_default().push(ByteRange {
        start: BytePos(start),
        end: BytePos(out_idx),
        filename: *DUMMY_FILE,
      });
      in_idx += close.len();
      continue;
    }

    buf.push(bytes[in_idx]);
    in_idx += 1;
    out_idx += 1;
  }

  if stack.len() > 0 {
    bail!("Unclosed delimiters: {stack:?}");
  }

  let prog_clean = String::from_utf8(buf)?;
  Ok((prog_clean, ranges))
}

pub fn make_span(range: ByteRange) -> Span {
  Span::new(
    rustc_span::BytePos(range.start.0 as u32),
    rustc_span::BytePos(range.end.0 as u32),
    SyntaxContext::root(),
    None,
  )
}

pub fn color_ranges(prog: &str, all_ranges: Vec<(&str, &HashSet<ByteRange>)>) -> String {
  let mut new_tokens = all_ranges
    .iter()
    .flat_map(|(_, ranges)| {
      ranges.iter().flat_map(|range| {
        let contained = all_ranges.iter().any(|(_, ranges)| {
          ranges.iter().any(|other| {
            range != other && other.start.0 <= range.end.0 && range.end.0 < other.end.0
          })
        });
        let end_marker = if contained { "]" } else { "\x1B[0m]" };
        [("[\x1B[31m", range.start), (end_marker, range.end)]
      })
    })
    .collect::<Vec<_>>();
  new_tokens.sort_by_key(|(_, i)| -(i.0 as isize));

  let mut output = prog.to_owned();
  for (s, i) in new_tokens {
    output.insert_str(i.0, s);
  }

  return output;
}

fn fmt_ranges(prog: &str, s: &HashSet<ByteRange>) -> String {
  textwrap::indent(&color_ranges(prog, vec![("", s)]), "  ")
}

pub fn compare_ranges(
  expected: HashSet<ByteRange>,
  actual: HashSet<ByteRange>,
  prog: &str,
) {
  let missing = &expected - &actual;
  let extra = &actual - &expected;

  let check = |s: HashSet<ByteRange>, message: &str| {
    if s.len() > 0 {
      println!("Expected ranges:\n{}", fmt_ranges(prog, &expected));
      println!("Actual ranges:\n{}", fmt_ranges(prog, &actual));
      panic!("{message} ranges:\n{}", fmt_ranges(prog, &s));
    }
  };

  check(missing, "Analysis did NOT have EXPECTED");
  check(extra, "Actual DID have UNEXPECTED");
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
  delims.sort_by_key(|(_, i)| i.0);

  let mut output = String::new();
  for (i, g) in contents.chars().enumerate() {
    while delims.len() > 0 && delims[0].1 .0 == i {
      let (delim, _) = delims.remove(0);
      output.push_str(delim);
    }
    output.push(g);
  }

  fs::write(path.with_extension("txt.expected"), output)?;

  Ok(())
}

pub fn test_command_output(
  path: &Path,
  expected: Option<&Path>,
  output_fn: impl for<'a, 'tcx> Fn(infoflow::FlowResults<'a, 'tcx>, Spanner<'tcx>, Span) -> Vec<Span>
    + Send
    + Sync,
) {
  let inner = move || -> Result<()> {
    info!("Testing {}", path.file_name().unwrap().to_string_lossy());
    let input = String::from_utf8(fs::read(path)?)?;

    let (input_clean, input_ranges) = parse_ranges(&input, vec![("`(", ")`")])?;
    let target = input_ranges["`("][0];

    compile_body_with_range(
      input_clean.clone(),
      target,
      move |tcx, body_id, body_with_facts| {
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

                compare_ranges(expected, actual, &input_clean);
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

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_parse_ranges() {
    let s = "`[`[f]`oo]`";
    let (clean, ranges) = parse_ranges(s, vec![("`[", "]`")]).unwrap();
    assert_eq!(clean, "foo");
    assert_eq!(ranges["`["], vec![
      ByteRange {
        start: BytePos(0),
        end: BytePos(1),
        filename: *DUMMY_FILE
      },
      ByteRange {
        start: BytePos(0),
        end: BytePos(3),
        filename: *DUMMY_FILE
      },
    ])
  }
}
