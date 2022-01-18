use std::fmt;

use rustc_data_structures::graph::{
  self, dominators, iterate, vec_graph::VecGraph, WithSuccessors,
};
use rustc_index::{
  bit_set::{BitSet, HybridBitSet, SparseBitMatrix},
  vec::IndexVec,
};
use rustc_middle::mir::*;

#[derive(Clone)]
pub struct BodyReversed<'tcx> {
  body: Body<'tcx>,
  exit_node: BasicBlock,
  exit_set: BitSet<BasicBlock>,
  unreachable: BitSet<BasicBlock>,
}

pub fn compute_post_dominators(
  body: Body,
) -> (dominators::Dominators<BasicBlock>, BodyReversed) {
  let nblocks = body.basic_blocks().len();
  let exit_node = BasicBlock::from_usize(nblocks);

  let mut exit_set = BitSet::new_empty(nblocks);
  for (bb_index, bb_data) in body.basic_blocks().iter_enumerated() {
    if matches!(bb_data.terminator().kind, TerminatorKind::Return) {
      exit_set.insert(bb_index);
    }
  }

  let mut graph = BodyReversed {
    body,
    exit_node,
    exit_set,
    unreachable: BitSet::new_empty(nblocks),
  };

  let reachable = iterate::post_order_from(&graph, graph.exit_node);
  graph.unreachable.insert_all();
  for n in reachable {
    if n != graph.exit_node {
      graph.unreachable.remove(n);
    }
  }

  (dominators::dominators(graph.clone()), graph)
}

impl graph::DirectedGraph for BodyReversed<'_> {
  type Node = BasicBlock;
}

impl graph::WithNumNodes for BodyReversed<'_> {
  fn num_nodes(&self) -> usize {
    // +1 for exit node
    self.body.basic_blocks().len() + 1
  }
}

impl graph::WithStartNode for BodyReversed<'_> {
  fn start_node(&self) -> Self::Node {
    self.exit_node
  }
}

impl graph::WithSuccessors for BodyReversed<'_> {
  fn successors(&self, node: Self::Node) -> <Self as graph::GraphSuccessors<'_>>::Iter {
    if node == self.exit_node {
      Box::new(self.exit_set.iter())
    } else {
      Box::new(self.body.predecessors()[node].iter().copied())
    }
  }
}

impl graph::GraphSuccessors<'graph> for BodyReversed<'_> {
  type Item = BasicBlock;
  type Iter = Box<dyn Iterator<Item = BasicBlock> + 'graph>;
}

impl graph::GraphPredecessors<'graph> for BodyReversed<'_> {
  type Item = BasicBlock;
  type Iter = Box<dyn Iterator<Item = BasicBlock> + 'graph>;
}

impl graph::WithPredecessors for BodyReversed<'_> {
  fn predecessors(
    &self,
    node: Self::Node,
  ) -> <Self as graph::GraphPredecessors<'_>>::Iter {
    assert!(node != self.exit_node);

    let exit_pred = self.exit_set.contains(node).then(|| self.exit_node);
    let preds = self.body.basic_blocks()[node]
      .terminator()
      .successors()
      .filter(|bb| !self.unreachable.contains(**bb))
      .copied();

    Box::new(exit_pred.into_iter().chain(preds))
  }
}

pub struct ControlDependencies(SparseBitMatrix<BasicBlock, BasicBlock>);

impl fmt::Debug for ControlDependencies {
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

impl ControlDependencies {
  /// Compute control dependencies from post-dominator frontier.
  ///
  /// Frontier algorithm from "An Efficient Method of Computing Single Static Assignment Form", Cytron et al. 89
  pub fn build(body: Body) -> Self {
    let (post_dominators, body_reversed) = compute_post_dominators(body);
    let body = &body_reversed.body;

    let idom = |x| {
      post_dominators
        .is_reachable(x)
        .then(|| post_dominators.immediate_dominator(x))
    };
    let edges = body
      .basic_blocks()
      .indices()
      .filter_map(|bb| idom(bb).map(|dom| (dom, bb)))
      .collect::<Vec<_>>();
    let n = body.basic_blocks().len();
    let dominator_tree = VecGraph::new(n + 1, edges);
    let traversal = iterate::post_order_from(&dominator_tree, body_reversed.exit_node);

    // Only use size = n b/c exit node shouldn't ever have a dominance frontier
    let mut df = IndexVec::from_elem_n(HybridBitSet::new_empty(n), n);
    for x in traversal {
      let local = body_reversed.successors(x);
      let up = dominator_tree
        .successors(x)
        .iter()
        .map(|z| df[*z].iter())
        .flatten();
      let frontier = local
        .chain(up)
        .filter(|y| idom(*y).map(|yd| yd != x).unwrap_or(false))
        .collect::<Vec<_>>();

      for y in frontier {
        df[x].insert(y);
      }
    }

    let mut cd = SparseBitMatrix::new(n);
    for (y, xs) in df.into_iter_enumerated() {
      for x in xs.iter() {
        cd.insert(x, y);
      }
    }

    let mut cd_transpose = SparseBitMatrix::new(n);
    for row in cd.rows() {
      if let Some(cols) = cd.row(row) {
        for col in cols.iter() {
          cd_transpose.insert(col, row);
        }
      }
    }

    ControlDependencies(cd_transpose)
  }

  pub fn dependent_on(&self, block: BasicBlock) -> Option<&HybridBitSet<BasicBlock>> {
    self.0.row(block)
  }
}

#[cfg(test)]
mod test {
  use log::debug;
  use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
  use test_log::test;

  use super::*;
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
      let control_deps = ControlDependencies::build(body.clone());

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
        let is_terminator =
          l2.statement_index == body.basic_blocks()[l2.block].statements.len();

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
