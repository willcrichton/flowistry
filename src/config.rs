use anyhow::{anyhow, bail, Result};
use rustc_span::{
  source_map::{SourceFile, SourceMap},
  BytePos, FileName, RealFileName, Span,
};
use serde::Serialize;
use std::default::Default;

#[derive(Serialize, Debug, Clone, Hash, PartialEq, Eq, Default)]
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
    let filename = source_map.span_to_filename(span);
    let filename = if let FileName::Real(RealFileName::LocalPath(filename)) = filename {
      filename.to_string_lossy().into_owned()
    } else {
      bail!("Range::from_span doesn't support {:?}", filename)
    };

    Ok(Range {
      start: span.lo().0 as usize,
      end: span.hi().0 as usize,
      filename,
    })
  }

  pub fn to_span(&self) -> Span {
    Span::with_root_ctxt(BytePos(self.start as u32), BytePos(self.end as u32))
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Hash)]
pub enum MutabilityMode {
  DistinguishMut,
  IgnoreMut,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Hash)]
pub enum ContextMode {
  SigOnly,
  Recurse,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Hash)]
pub enum PointerMode {
  Precise,
  Conservative,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Hash)]
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

