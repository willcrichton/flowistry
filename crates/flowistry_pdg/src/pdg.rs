//! The representation of the PDG.

use std::fmt;

use internment::Intern;
use serde::{Deserialize, Serialize};

use crate::rustc_portable::*;
#[cfg(feature = "rustc")]
use crate::rustc_proxies;

/// Extends a MIR body's `Location` with `Start` (before the first instruction) and `End` (after all returns).
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum RichLocation {
  /// The point *after* a location in a body.
  #[cfg_attr(feature = "rustc", serde(with = "rustc_proxies::Location"))]
  Location(Location),

  /// The start of the body.
  ///
  /// Note that [`Location::START`] is different from [`RichLocation::Start`]!
  /// The latter is *before* the former in time.
  Start,

  /// The end of the body, after all possible return statements.
  End,
}

impl RichLocation {
  /// Returns true if this is a `Start` location.
  pub fn is_start(self) -> bool {
    matches!(self, RichLocation::Start)
  }

  /// Returns true if this is an `End` location.
  pub fn is_end(self) -> bool {
    matches!(self, RichLocation::End)
  }

  pub fn is_real(self) -> bool {
    matches!(self, RichLocation::Location(_))
  }

  /// Returns the [`Location`] in `self`, panicking otherwise.
  pub fn unwrap_location(self) -> Location {
    self
      .as_location()
      .expect("RichLocation was unexpectedly Start")
  }

  /// Returns the [`Location`] in `self`, returning `None` otherwise.
  pub fn as_location(self) -> Option<Location> {
    match self {
      RichLocation::Location(location) => Some(location),
      RichLocation::Start | RichLocation::End => None,
    }
  }
}

impl fmt::Display for RichLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      RichLocation::Location(loc) => write!(f, "{loc:?}"),
      RichLocation::Start => write!(f, "start"),
      RichLocation::End => write!(f, "end"),
    }
  }
}

impl From<Location> for RichLocation {
  fn from(value: Location) -> Self {
    RichLocation::Location(value)
  }
}

/// A [`RichLocation`] within a specific point in a codebase.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GlobalLocation {
  /// The function containing the location.
  #[cfg_attr(feature = "rustc", serde(with = "rustc_proxies::LocalDefId"))]
  pub function: LocalDefId,

  /// The location of an instruction in the function, or the function's start.
  pub location: RichLocation,
}

#[cfg(not(feature = "rustc"))]

impl fmt::Display for GlobalLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}::{}", self.function, self.location)
  }
}

/// A location within the global call-graph.
///
/// The first location is the root of the call-graph.
/// The last location is the currently-called function.
///
/// Invariant: a call string should never be empty, i.e.,
/// there should always be at least one [`GlobalLocation`] in a call-string.
///
/// Note: This type is copyable due to interning.
#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Serialize, Deserialize)]
pub struct CallString(Intern<Vec<GlobalLocation>>);

impl CallString {
  /// Create a new call string from a list of global locations.
  fn new(locs: Vec<GlobalLocation>) -> Self {
    CallString(Intern::new(locs))
  }

  /// Create an initial call string for the single location `loc`.
  pub fn single(loc: GlobalLocation) -> Self {
    Self::new(vec![loc])
  }

  /// Returns the leaf of the call string (the currently-called function).
  pub fn leaf(self) -> GlobalLocation {
    *self.0.last().unwrap()
  }

  /// Returns the call string minus the root.
  pub fn caller(self) -> Self {
    CallString::new(self.0[.. self.0.len() - 1].to_vec())
  }

  /// Returns an iterator over the locations in the call string, starting at the leaf and going to the root.
  pub fn iter(&self) -> impl DoubleEndedIterator<Item = GlobalLocation> + '_ {
    self.0.iter().rev().copied()
  }

  /// Adds a new call site to the end of the call string.
  pub fn push(self, loc: GlobalLocation) -> Self {
    let mut string = self.0.to_vec();
    string.push(loc);
    CallString::new(string)
  }

  pub fn is_at_root(self) -> bool {
    self.0.len() == 1
  }

  pub fn root(self) -> GlobalLocation {
    *self.0.first().unwrap()
  }

  pub fn stable_id(self) -> usize {
    let r: &'static Vec<GlobalLocation> = self.0.as_ref();
    r as *const Vec<GlobalLocation> as usize
  }

  pub fn iter_from_root(&self) -> impl DoubleEndedIterator<Item = GlobalLocation> + '_ {
    self.0.iter().copied()
  }

  pub fn len(self) -> usize {
    self.0.len()
  }

  pub fn is_empty(self) -> bool {
    self.0.is_empty()
  }
}

impl fmt::Display for CallString {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, loc) in self.0.iter().enumerate() {
      if i > 0 {
        write!(f, "←")?;
      }
      loc.fmt(f)?;
    }
    Ok(())
  }
}
