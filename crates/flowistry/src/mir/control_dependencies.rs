//! An algorithm to compute control-dependencies between MIR blocks.
//!
//! In a function $f$, a block $Y$ is control-dependent on a block $X$ if the execution of $Y$
//! is conditional on $X$, e.g. if $X$ is a conditional branch and $Y$ is one of the branches.
//!
//! See Section 3.1 of "The Program Dependence Graph and Its Use in Optimization" (Ferrante et al. 1987)
//! for more on how to define and analyze control-dependence.

use std::fmt;

use rustc_data_structures::graph::{
  dominators::{Dominators, Iter as DominatorsIter},
  vec_graph::VecGraph,
  *,
};
use rustc_index::{
  bit_set::{BitSet, HybridBitSet, SparseBitMatrix},
  vec::Idx,
};
use smallvec::SmallVec;

struct ReversedGraph<'a, G: ControlFlowGraph> {
  graph: &'a G,
  exit: G::Node,
  unreachable: BitSet<G::Node>,
}

impl<G: ControlFlowGraph> DirectedGraph for ReversedGraph<'_, G> {
  type Node = G::Node;
}

impl<G: ControlFlowGraph> WithStartNode for ReversedGraph<'_, G> {
  fn start_node(&self) -> Self::Node {
    self.exit
  }
}

impl<G: ControlFlowGraph> WithNumNodes for ReversedGraph<'_, G> {
  fn num_nodes(&self) -> usize {
    self.graph.num_nodes()
  }
}

impl<'graph, G: ControlFlowGraph> GraphSuccessors<'graph> for ReversedGraph<'_, G> {
  type Item = G::Node;
  type Iter = smallvec::IntoIter<[Self::Item; 4]>;
}

impl<G: ControlFlowGraph> WithSuccessors for ReversedGraph<'_, G> {
  fn successors(&self, node: Self::Node) -> <Self as GraphSuccessors<'_>>::Iter {
    self
      .graph
      .predecessors(node)
      .filter(|bb| !self.unreachable.contains(*bb))
      // We have to collect -> immediately into_iter because we need to return
      // an iterator type that doesn't describe closures, which aren't nameable
      // in the GraphSuccessors trait implementation.
      .collect::<SmallVec<[G::Node; 4]>>()
      .into_iter()
  }
}

impl<'graph, G: ControlFlowGraph> GraphPredecessors<'graph> for ReversedGraph<'_, G> {
  type Item = G::Node;
  type Iter = smallvec::IntoIter<[Self::Item; 4]>;
}

impl<G: ControlFlowGraph> WithPredecessors for ReversedGraph<'_, G> {
  fn predecessors(&self, node: Self::Node) -> <Self as GraphPredecessors<'_>>::Iter {
    self
      .graph
      .successors(node)
      .filter(|bb| !self.unreachable.contains(*bb))
      .collect::<SmallVec<[G::Node; 4]>>()
      .into_iter()
  }
}

/// Represents the post-dominators of a graph's nodes with respect to a particular exit
pub struct PostDominators<Node: Idx>(Dominators<Node>);

impl<Node: Idx> PostDominators<Node> {
  /// Constructs the post-dominators by computing the dominators on a reversed graph
  pub fn build<G: ControlFlowGraph<Node = Node>>(graph: &G, exit: Node) -> Self {
    let mut reversed = ReversedGraph {
      graph,
      exit,
      unreachable: BitSet::new_empty(graph.num_nodes()),
    };

    let reachable = iterate::post_order_from(&reversed, exit);
    reversed.unreachable.insert_all();
    for n in &reachable {
      reversed.unreachable.remove(*n);
    }

    let dominators = dominators::dominators(reversed);
    PostDominators::<Node>(dominators)
  }

  /// Gets the node that immediately post-dominators `node`, if one exists
  pub fn immediate_post_dominator(&self, node: Node) -> Option<Node> {
    let reachable = self.0.is_reachable(node);
    reachable.then(|| self.0.immediate_dominator(node))
  }

  /// Gets all nodes that post-dominate `node`, if they exist
  pub fn post_dominators(&self, node: Node) -> Option<DominatorsIter<'_, Node>> {
    let reachable = self.0.is_reachable(node);
    reachable.then(|| self.0.dominators(node))
  }
}

pub struct ControlDependencies<Node: Idx>(SparseBitMatrix<Node, Node>);

impl<Node: Idx> fmt::Debug for ControlDependencies<Node> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (i, (bb, bbs)) in self
      .0
      .rows()
      .enumerate()
      .filter_map(|(i, bb)| self.0.row(bb).map(move |bbs| (i, (bb, bbs))))
    {
      if i > 0 {
        write!(f, ", ")?;
      }
      write!(f, "{bb:?}: {{")?;
      for (j, bb2) in bbs.iter().enumerate() {
        if j > 0 {
          write!(f, ", ")?;
        }
        write!(f, "{bb2:?}")?;
      }
      write!(f, "}}")?;
    }
    Ok(())
  }
}

impl<Node: Idx + Ord> ControlDependencies<Node> {
  /// Compute control dependencies from post-dominator frontier.
  ///
  /// Frontier algorithm from "An Efficient Method of Computing Single Static Assignment Form", Cytron et al. 89
  fn build<G: ControlFlowGraph<Node = Node>>(graph: &G, exit: Node) -> Self {
    let post_dominators = PostDominators::build(graph, exit);
    let idom = |node| post_dominators.immediate_post_dominator(node);

    let n = graph.num_nodes();
    let edges = (0 .. n)
      .filter_map(|i| {
        let node = Node::new(i);
        Some((idom(node)?, node))
      })
      .collect::<Vec<_>>();
    let dominator_tree = VecGraph::new(n, edges);

    let traversal = iterate::post_order_from(&dominator_tree, exit);

    // Only use size = n b/c exit node shouldn't ever have a dominance frontier
    let mut df = SparseBitMatrix::new(n);
    for x in traversal {
      let local = graph.predecessors(x);
      let up = dominator_tree
        .successors(x)
        .iter()
        .flat_map(|z| df.row(*z).into_iter().flat_map(|set| set.iter()));
      let frontier = local
        .chain(up)
        .filter(|y| idom(*y).map(|yd| yd != x).unwrap_or(false))
        .collect::<Vec<_>>();

      for y in frontier {
        df.insert(x, y);
      }
    }

    ControlDependencies(df)
  }

  /// Compute the union of control dependencies from multiple exits.
  pub fn build_many<G: ControlFlowGraph<Node = Node>>(
    graph: &G,
    exits: impl IntoIterator<Item = Node>,
  ) -> Self {
    let mut all_deps = SparseBitMatrix::new(graph.num_nodes());
    for exit in exits {
      let deps = ControlDependencies::build(graph, exit);
      for node in deps.0.rows() {
        if let Some(set) = deps.0.row(node) {
          all_deps.union_row(node, set);
        }
      }
    }
    ControlDependencies(all_deps)
  }

  /// Returns the set of all node that are control-dependent on the given `node`.
  pub fn dependent_on(&self, node: Node) -> Option<&HybridBitSet<Node>> {
    self.0.row(node)
  }
}

#[cfg(test)]
mod test {
  use log::debug;
  use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
  use rustc_middle::mir::Location;
  use test_log::test;

  use crate::{mir::utils::BodyExt, test_utils};

  #[test]
  fn test_control_dependencies() {
    let input = r#"
    fn main() {
      let mut x = 1;
      x = 2;
      if true { x = 3; }
      for _ in 0 .. 1 { x = 4; }
      x = 5;
    }"#;
    test_utils::compile_body(input, move |tcx, _, body_with_facts| {
      let body = &body_with_facts.body;
      let control_deps = body.control_dependencies();

      let mut snippet_to_loc: HashMap<_, Vec<_>> = HashMap::default();
      for loc in body.all_locations() {
        let snippet = tcx
          .sess
          .source_map()
          .span_to_snippet(body.source_info(loc).span)
          .unwrap();
        snippet_to_loc.entry(snippet).or_default().push(loc);
      }
      debug!("snippet_to_loc: {snippet_to_loc:#?}");
      let pair = |s| (s, &snippet_to_loc[s]);

      let x_eq_1 = pair("mut x");
      let x_eq_2 = pair("x = 2");
      let if_true = pair("true");
      let x_eq_3 = pair("x = 3");
      let for_in = pair("0 .. 1");
      let x_eq_4 = pair("x = 4");
      let x_eq_5 = pair("x = 5");

      let is_dep_loc = |l1: Location, l2: Location| {
        let is_terminator = body.stmt_at(l2).is_right();
        is_terminator
          && control_deps
            .dependent_on(l1.block)
            .map(|deps| deps.contains(l2.block))
            .unwrap_or(false)
      };

      let is_dep = |l1: &[Location], l2: &[Location]| {
        l1.iter().any(|l1| l2.iter().any(|l2| is_dep_loc(*l1, *l2)))
      };

      let all_locs = [x_eq_1, x_eq_2, if_true, x_eq_3, for_in, x_eq_4, x_eq_5]
        .into_iter()
        .collect::<HashSet<_>>();
      let all_deps: &[(_, &[_])] = &[
        (x_eq_1, &[]),
        (x_eq_2, &[]),
        (if_true, &[x_eq_3]),
        (x_eq_3, &[]),
        (for_in, &[x_eq_4]),
        (x_eq_5, &[]),
      ];

      for ((parent_name, parent_locs), desired) in all_deps {
        let desired = desired.iter().copied().collect::<HashSet<_>>();
        for (child_name, child_locs) in &desired {
          assert!(
            is_dep(child_locs, parent_locs),
            "{child_name} was not dependent on {parent_name}, but should be"
          );
        }

        let remaining = &all_locs - &desired;
        for (child_name, child_locs) in remaining {
          if *parent_name == child_name {
            continue;
          }

          assert!(
            !is_dep(child_locs, parent_locs),
            "{child_name} was dependent on {parent_name}, but should not be"
          );
        }
      }
    });
  }
}
