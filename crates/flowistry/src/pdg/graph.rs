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
  Location(Location),
  Start,
}

impl LocationOrStart {
  pub fn unwrap_location(self) -> Location {
    self
      .as_location()
      .expect("LocationOrStart was unexpectedly Start")
  }

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

/// A [`LocationOrStart`] localized to a specific [`LocalDefId`].
#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct GlobalLocation {
  pub function: LocalDefId,
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
  pub fn new(locs: Vec<GlobalLocation>) -> Self {
    CallString(Intern::new(locs))
  }

  pub fn root(self) -> GlobalLocation {
    self.0[0]
  }

  pub fn caller(self) -> Self {
    CallString::new(self.iter().skip(1).collect())
  }

  pub fn iter(&self) -> impl Iterator<Item = GlobalLocation> + '_ {
    self.0.iter().copied()
  }

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

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct DepNode<'tcx> {
  pub place: Place<'tcx>,
  pub at: CallString,
  place_pretty: Option<Intern<String>>,
}

impl<'tcx> DepNode<'tcx> {
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

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct DepEdge {
  pub kind: DepEdgeKind,
  pub at: CallString,
}

impl DepEdge {
  pub fn data(at: CallString) -> Self {
    DepEdge {
      kind: DepEdgeKind::Data,
      at,
    }
  }

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

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum DepEdgeKind {
  Control,
  Data,
}

pub struct DepGraph<'tcx> {
  pub graph: DiGraph<DepNode<'tcx>, DepEdge>,
}

impl<'tcx> DepGraph<'tcx> {
  pub fn new(graph: DiGraph<DepNode<'tcx>, DepEdge>) -> Self {
    Self { graph }
  }
}

impl<'tcx> DepGraph<'tcx> {
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
