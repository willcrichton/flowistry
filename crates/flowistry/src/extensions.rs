//! Extra features for evaluating / ablating the precision of Flowistry's algorithm.
#![allow(missing_docs)]

use std::{cell::RefCell, str::FromStr};

use fluid_let::fluid_let;
use serde::{Deserialize, Serialize};

/// Whether Flowistry should ignore the distinction between mutable and immtuable references
#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum MutabilityMode {
  /// Precise behavior, distinguish them
  DistinguishMut,
  /// Imprecise behavior, do not distinguish them (assume everything is mutable)
  IgnoreMut,
}

impl FromStr for MutabilityMode {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "DistinguishMut" => Ok(Self::DistinguishMut),
      "IgnoreMut" => Ok(Self::IgnoreMut),
      _ => Err(format!("Could not parse: {s}")),
    }
  }
}

/// Whether Flowistry should attempt to recurse into call-sites to analyze them
#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum ContextMode {
  /// Imprecise behavior, only use the modular approximation
  SigOnly,
  /// Precise behavior, recurse into call sites when possible
  Recurse,
}

impl FromStr for ContextMode {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "SigOnly" => Ok(Self::SigOnly),
      "Recurse" => Ok(Self::Recurse),
      _ => Err(format!("Could not parse: {s}")),
    }
  }
}

/// Whether Flowistry should use lifetimes to distinguish pointers
#[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize, Hash)]
pub enum PointerMode {
  /// Precise behavior, use lifetimes
  Precise,
  /// Imprecise behavior, assume all pointers alias
  Conservative,
}

impl FromStr for PointerMode {
  type Err = String;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "Precise" => Ok(Self::Precise),
      "Conservative" => Ok(Self::Conservative),
      _ => Err(format!("Could not parse: {s}")),
    }
  }
}

/// A combination of all the precision levers.
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

fluid_let!(pub static EVAL_MODE: EvalMode);
fluid_let!(pub static REACHED_LIBRARY: RefCell<bool>);

pub fn is_extension_active(f: impl Fn(EvalMode) -> bool) -> bool {
  EVAL_MODE.copied().map(f).unwrap_or(false)
}
