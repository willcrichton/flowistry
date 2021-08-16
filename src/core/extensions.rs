use serde::Serialize;

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