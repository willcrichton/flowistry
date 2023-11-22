use std::{fmt, path::Path};

use petgraph::{dot, graph::DiGraph};
use rustc_middle::mir::{Location, Place};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum LocationOrStart {
  Location(Location),
  Start,
}

impl fmt::Debug for LocationOrStart {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LocationOrStart::Location(loc) => write!(f, "{loc:?}"),
      LocationOrStart::Start => write!(f, "start"),
    }
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum DepNode<'tcx> {
  Place {
    place: Place<'tcx>,
    at: LocationOrStart,
  },
  Op(Location),
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
