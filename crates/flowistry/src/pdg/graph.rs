use std::{fmt, path::Path};

use petgraph::{dot, graph::DiGraph};
use rustc_hir::def_id::LocalDefId;
use rustc_middle::{
  mir::{Location, Place},
  ty::tls,
};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum LocationOrStart {
  Location(Location),
  Start,
}

impl fmt::Debug for LocationOrStart {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LocationOrStart::Location(loc) => loc.fmt(f),
      LocationOrStart::Start => write!(f, "start"),
    }
  }
}

impl Into<LocationOrStart> for Location {
  fn into(self) -> LocationOrStart {
    LocationOrStart::Location(self)
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct GlobalLocation {
  pub function: LocalDefId,
  pub location: LocationOrStart,
}

impl fmt::Debug for GlobalLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}::", self.location)?;
    tls::with_opt(|opt_tcx| match opt_tcx {
      Some(tcx) => write!(f, "{}", tcx.item_name(self.function.to_def_id())),
      None => write!(f, "{:?}", self.function),
    })
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum DepNode<'tcx> {
  Place {
    place: Place<'tcx>,
    at: GlobalLocation,
  },
  Op(GlobalLocation),
}

impl<'tcx> DepNode<'tcx> {
  pub fn expect_place(self) -> Place<'tcx> {
    match self {
      DepNode::Place { place, .. } => place,
      DepNode::Op(..) => panic!("Expected a place, got an op"),
    }
  }
}

impl fmt::Debug for DepNode<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      DepNode::Place { place, at } => write!(f, "{place:?}@{at:?}"),
      DepNode::Op(loc) => write!(f, "OP@{loc:?}"),
    }
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum DepEdge {
  Control,
  Data,
}

pub struct DepGraph<'tcx> {
  pub graph: DiGraph<DepNode<'tcx>, DepEdge>,
}

impl<'tcx> DepGraph<'tcx> {
  pub fn generate_graphviz(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let graph_dot = format!("{:?}", dot::Dot::with_config(&self.graph, &[]));
    rustc_utils::mir::body::run_dot(path.as_ref(), graph_dot.into_bytes())
  }
}
