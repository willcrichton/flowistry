use crate::mir::borrowck_facts;
use anyhow::{anyhow, bail, Context, Result};
use rustc_borrowck::BodyWithBorrowckFacts;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::{BodyId, ItemKind};
use rustc_middle::ty::TyCtxt;
use rustc_span::{source_map::FileLoader, BytePos, Span, SyntaxContext};
use std::{io, path::Path, process::Command};

struct StringLoader(String);
impl FileLoader for StringLoader {
  fn file_exists(&self, _: &Path) -> bool {
    true
  }
  fn read_file(&self, _: &Path) -> io::Result<String> {
    Ok(self.0.clone())
  }
}

lazy_static::lazy_static! {
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

pub fn compile_body(
  input: impl Into<String>,
  callback: impl for<'tcx> FnOnce(TyCtxt<'tcx>, BodyId, &BodyWithBorrowckFacts<'tcx>) + Send,
) {
  compile(input, |tcx| {
    let body_id = tcx
      .hir()
      .items()
      .filter_map(|item| match item.kind {
        ItemKind::Fn(_, _, body) => Some(body),
        _ => None,
      })
      .next()
      .unwrap();

    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);

    callback(tcx, body_id, body_with_facts);
  })
}

pub fn compile(input: impl Into<String>, callback: impl FnOnce(TyCtxt<'_>) + Send) {
  let mut callbacks = TestCallbacks {
    callback: Some(callback),
  };
  let args = format!(
    "rustc dummy.rs --crate-type lib -Z identify-regions -Z mir-opt-level=0 --sysroot {}",
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

pub fn parse_ranges(
  src: impl AsRef<str>,
  delimiters: impl AsRef<[(&'static str, &'static str)]>,
) -> Result<(String, HashMap<&'static str, Vec<(usize, usize)>>)> {
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
        .with_context(|| anyhow!("Missing open delimiter for \"{}\"", close))?;
      ranges.entry(delim).or_default().push((start, out_idx));
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
  Ok((prog_clean, ranges))
}

pub fn make_span((lo, hi): (usize, usize)) -> Span {
  Span::new(
    BytePos(lo as u32),
    BytePos(hi as u32),
    SyntaxContext::root(),
    None,
  )
}
