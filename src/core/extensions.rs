use rustc_macros::Encodable;

use crate::core::config::{EvalMode, EVAL_MODE};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Encodable, Hash)]
pub enum MutabilityMode {
  DistinguishMut,
  IgnoreMut,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Encodable, Hash)]
pub enum ContextMode {
  SigOnly,
  Recurse,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Encodable, Hash)]
pub enum PointerMode {
  Precise,
  Conservative,
}

pub fn is_extension_active(f: impl Fn(EvalMode) -> bool) -> bool {
  EVAL_MODE.copied().map(f).unwrap_or(false)
}
