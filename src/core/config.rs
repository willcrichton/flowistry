use crate::core::extensions::{ContextMode, MutabilityMode, PointerMode};
use anyhow::{bail, Context, Result};
use rustc_macros::Encodable;
use rustc_span::{
  source_map::{monotonic::MonotonicVec, SourceMap},
  BytePos, FileName, RealFileName, SourceFile, Span,
};
use std::{cell::Ref, default::Default, path::Path, rc::Rc};

#[derive(Encodable, Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct Range {
  pub start: usize,
  pub end: usize,
  pub filename: String,
}

impl Range {
  pub fn substr(&self, s: &str) -> String {
    String::from_utf8(
      s.bytes()
        .skip(self.start)
        .take(self.end - self.start)
        .collect::<Vec<_>>(),
    )
    .unwrap()
  }
}

impl Range {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Result<Self> {
    let file = source_map.lookup_source_file(span.lo());
    let filename = match &file.name {
      FileName::Real(RealFileName::LocalPath(filename)) => filename.to_string_lossy().into_owned(),
      filename => bail!("Range::from_span doesn't support {:?}", filename),
    };

    let offset = file.start_pos;
    Ok(Range {
      start: (span.lo() - offset).0 as usize,
      end: (span.hi() - offset).0 as usize,
      filename,
    })
  }

  pub fn source_file<'a>(
    &self,
    files: &'a Ref<'_, MonotonicVec<Rc<SourceFile>>>,
  ) -> Result<&'a SourceFile> {
    let filename = Path::new(&self.filename);
    files
      .iter()
      .find(|file| match &file.name {
        // rustc seems to store relative paths to files in the workspace, so if filename is absolute,
        // we can compare them using Path::ends_with
        FileName::Real(RealFileName::LocalPath(other)) => filename.ends_with(other),
        _ => false,
      })
      .map(|f| &**f)
      .with_context(|| format!("Could not find SourceFile for path: {}", self.filename))
  }

  pub fn to_span(&self, source_map: &SourceMap) -> Result<Span> {
    let files = source_map.files();
    let source_file = self.source_file(&files)?;
    let offset = source_file.start_pos;

    Ok(Span::with_root_ctxt(
      offset + BytePos(self.start as u32),
      offset + BytePos(self.end as u32),
    ))
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Encodable, Hash)]
pub struct EvalMode {
  pub mutability_mode: MutabilityMode,
  pub context_mode: ContextMode,
  pub pointer_mode: PointerMode,
}

impl Default for EvalMode {
  fn default() -> Self {
    EvalMode {
      mutability_mode: MutabilityMode::DistinguishMut,
      context_mode: ContextMode::SigOnly,
      pointer_mode: PointerMode::Precise,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Config {
  pub range: Range,
  pub debug: bool,
  pub eval_mode: EvalMode,
  pub local: Option<usize>,
}
