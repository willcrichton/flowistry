//! The representation of the PDG.

use std::{fmt, iter, path::Path};

use internment::Intern;
use petgraph::{dot, graph::DiGraph};
use rustc_hir::def_id::LocalDefId;
use rustc_middle::{
  mir::{Body, Location, Place},
  ty::{tls, TyCtxt},
};
use rustc_utils::PlaceExt;

/// Extends a MIR body's `Location` with `Start` to represent facts about arguments before the first instruction.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum LocationOrStart {
  /// The point *after* a location in a body.
  Location(Location),
  /// The start of the body.
  ///
  /// Note that [`Location::START`] is different from [`LocationOrStart::Start`]!
  /// The latter is *before* the former in time.
  Start,
}

impl LocationOrStart {
  /// Returns the [`Location`] in `self`, panicking otherwise.
  pub fn unwrap_location(self) -> Location {
    self
      .as_location()
      .expect("LocationOrStart was unexpectedly Start")
  }

  /// Returns the [`Location`] in `self`, returning `None` otherwise.
  pub fn as_location(self) -> Option<Location> {
    match self {
      LocationOrStart::Location(location) => Some(location),
      LocationOrStart::Start => None,
    }
  }
}

impl fmt::Debug for LocationOrStart {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LocationOrStart::Location(loc) => loc.fmt(f),
      LocationOrStart::Start => write!(f, "start"),
    }
  }
}

impl From<Location> for LocationOrStart {
  fn from(value: Location) -> Self {
    LocationOrStart::Location(value)
  }
}

/// A [`LocationOrStart`] within a specific point in a codebase.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct GlobalLocation {
  /// The function containing the location.
  pub function: LocalDefId,

  /// The location of an instruction in the function, or the function's start.
  pub location: LocationOrStart,
}

impl fmt::Debug for GlobalLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    tls::with_opt(|opt_tcx| match opt_tcx {
      Some(tcx) => match tcx.opt_item_name(self.function.to_def_id()) {
        Some(name) => write!(f, "{name}"),
        None => write!(f, "<closure>"),
      },
      None => write!(f, "{:?}", self.function),
    })?;
    write!(f, "::{:?}", self.location)
  }
}

/// A location within the global call-graph.
///
/// The 0-th location is the root location, and every subsequent location
/// is a call-site to the one before it.
///
/// This type is copyable due to interning.
#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct CallString(Intern<Vec<GlobalLocation>>);

impl CallString {
  /// Create a new call string from a list of global locations.
  pub fn new(locs: Vec<GlobalLocation>) -> Self {
    CallString(Intern::new(locs))
  }

  /// Returns the root of the call string.
  pub fn root(self) -> GlobalLocation {
    self.0[0]
  }

  /// Returns the call string minus the root.
  pub fn caller(self) -> Self {
    CallString::new(self.iter().skip(1).collect())
  }

  /// Returns an iterator over the locations in the call string, starting at the root.
  pub fn iter(&self) -> impl Iterator<Item = GlobalLocation> + '_ {
    self.0.iter().copied()
  }

  /// Adds a new root location to the call string.
  pub fn extend(self, loc: GlobalLocation) -> Self {
    CallString::new(iter::once(loc).chain(self.iter()).collect())
  }
}

impl fmt::Debug for CallString {
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

/// A node in the program dependency graph.
///
/// Represents a place at a particular call-string.
/// The place is in the body of the root of the call-string.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct DepNode<'tcx> {
  /// A place in memory in a particular body.
  pub place: Place<'tcx>,
  /// The point in the execution of the program.
  pub at: CallString,

  /// Pretty representation of the place.
  /// This is cached as an interned string on [`DepNode`] because to compute it later,
  /// we would have to regenerate the entire monomorphized body for a given place.
  place_pretty: Option<Intern<String>>,
}

impl<'tcx> DepNode<'tcx> {
  /// Constructs a new [`DepNode`].
  ///
  /// The [`tcx`] and [`body`] arguments are used to precompute a pretty string
  /// representation of the [`DepNode`].
  pub fn new(
    place: Place<'tcx>,
    at: CallString,
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
  ) -> Self {
    DepNode {
      place,
      at,
      place_pretty: place.to_string(tcx, body).map(Intern::new),
    }
  }
}

impl DepNode<'_> {
  /// Returns a pretty string representation of the place, if one exists.
  pub fn place_pretty(&self) -> Option<&str> {
    self.place_pretty.map(|s| s.as_ref().as_str())
  }
}

impl fmt::Debug for DepNode<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.place_pretty() {
      // Some(s) => write!(f, "{s}")?,
      _ => write!(f, "{:?}", self.place)?,
    };
    write!(f, " @ {:?}", self.at)
  }
}

/// A kind of edge in the program dependence graph.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum DepEdgeKind {
  /// X is control-dependent on Y if the value of Y influences the execution
  /// of statements that affect the value of X.
  Control,

  /// X is data-dependent on Y if the value of Y is an input to statements that affect
  /// the value of X.
  Data,
}

/// An edge in the program dependence graph.
///
/// Represents an operation that induces a dependency between places.
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct DepEdge {
  /// Either data or control.
  pub kind: DepEdgeKind,

  /// The location of the operation.
  pub at: CallString,
}

impl DepEdge {
  /// Constructs a data edge.
  pub fn data(at: CallString) -> Self {
    DepEdge {
      kind: DepEdgeKind::Data,
      at,
    }
  }

  /// Constructs a control edge.
  pub fn control(at: CallString) -> Self {
    DepEdge {
      kind: DepEdgeKind::Control,
      at,
    }
  }
}

impl fmt::Debug for DepEdge {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?} @ {:?}", self.kind, self.at)
  }
}

/// The top-level PDG.
pub struct DepGraph<'tcx> {
  /// The petgraph representation of the PDG.
  pub graph: DiGraph<DepNode<'tcx>, DepEdge>,
}

impl<'tcx> DepGraph<'tcx> {
  /// Constructs a new [`DepGraph`].
  pub fn new(graph: DiGraph<DepNode<'tcx>, DepEdge>) -> Self {
    Self { graph }
  }
}

impl<'tcx> DepGraph<'tcx> {
  /// Generates a graphviz visualization of the PDG and saves it to `path`.
  pub fn generate_graphviz(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let graph_dot = format!(
      "{:?}",
      dot::Dot::with_attr_getters(
        &self.graph,
        &[],
        &|_, _| String::new(),
        &|_, (_, _)| format!("fontname=\"Courier New\"")
      )
    );
    rustc_utils::mir::body::run_dot(path.as_ref(), graph_dot.into_bytes())
  }
}
