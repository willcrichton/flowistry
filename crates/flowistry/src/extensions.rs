use fluid_let::fluid_let;
use rustc_macros::Encodable;
use std::cell::RefCell;

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

fluid_let!(pub static EVAL_MODE: EvalMode);
fluid_let!(pub static REACHED_LIBRARY: RefCell<bool>);

pub fn is_extension_active(f: impl Fn(EvalMode) -> bool) -> bool {
  EVAL_MODE.copied().map(f).unwrap_or(false)
}
