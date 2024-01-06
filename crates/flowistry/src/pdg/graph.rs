//! The representation of the PDG.

use std::{fmt, path::Path};

use flowistry_pdg::CallString;
use internment::Intern;
use petgraph::{dot, graph::DiGraph};
use rustc_middle::{
  mir::{Body, Place},
  ty::TyCtxt,
};
use rustc_utils::PlaceExt;

/// A node in the program dependency graph.
///
/// Represents a place at a particular call-string.
/// The place is in the body of the root of the call-string.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
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
  /// The `tcx` and `body` arguments are used to precompute a pretty string
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

impl fmt::Display for DepNode<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.place_pretty() {
      Some(s) => s.fmt(f)?,
      None => write!(f, "{:?}", self.place)?,
    };
    write!(f, " @ {}", self.at)
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
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
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

impl fmt::Display for DepEdge {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}\n@ {}", self.kind, self.at)
  }
}

/// The top-level PDG.
#[derive(Clone, Debug)]
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
      "{}",
      dot::Dot::with_attr_getters(
        &self.graph,
        &[],
        &|_, _| format!("fontname=\"Courier New\""),
        &|_, (_, _)| format!("fontname=\"Courier New\"")
      )
    );
    rustc_utils::mir::body::run_dot(path.as_ref(), graph_dot.into_bytes())
  }
}
