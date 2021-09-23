use rustc_macros::Encodable;

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
