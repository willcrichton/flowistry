use std::fmt;

use rustc_data_structures::{
  fx::FxHashMap as HashMap,
  graph::{vec_graph::VecGraph, *},
};
use rustc_index::bit_set::{BitSet, HybridBitSet, SparseBitMatrix};
use rustc_middle::mir::*;

use super::utils::BodyExt;

#[derive(Clone)]
pub struct BodyReversed<'a, 'tcx> {
  body: &'a Body<'tcx>,
  ret: BasicBlock,
  unreachable: BitSet<BasicBlock>,
}

pub fn compute_immediate_post_dominators(
  body: &Body,
  ret: BasicBlock,
) -> HashMap<BasicBlock, BasicBlock> {
  let nblocks = body.basic_blocks().len();
  let mut graph = BodyReversed {
    body,
    ret,
    unreachable: BitSet::new_empty(nblocks),
  };

  let reachable = iterate::post_order_from(&graph, ret);
  graph.unreachable.insert_all();
  for n in &reachable {
    graph.unreachable.remove(*n);
  }

  let dominators = dominators::dominators(graph);
  reachable
    .into_iter()
    .map(|n| (n, dominators.immediate_dominator(n)))
    .collect()
}

impl DirectedGraph for BodyReversed<'_, '_> {
  type Node = BasicBlock;
}

impl WithStartNode for BodyReversed<'_, '_> {
  fn start_node(&self) -> Self::Node {
    self.ret
  }
}

impl WithNumNodes for BodyReversed<'_, '_> {
  fn num_nodes(&self) -> usize {
    self.body.basic_blocks().len()
  }
}

impl GraphSuccessors<'graph> for BodyReversed<'_, '_> {
  type Item = BasicBlock;
  type Iter = Box<dyn Iterator<Item = BasicBlock> + 'graph>;
}

impl WithSuccessors for BodyReversed<'_, '_> {
  fn successors(&self, node: Self::Node) -> <Self as GraphSuccessors<'_>>::Iter {
    Box::new(
      self.body.predecessors()[node]
        .iter()
        .filter(|bb| !self.unreachable.contains(**bb))
        .copied(),
    )
  }
}

impl GraphPredecessors<'graph> for BodyReversed<'_, '_> {
  type Item = BasicBlock;
  type Iter = Box<dyn Iterator<Item = BasicBlock> + 'graph>;
}

impl WithPredecessors for BodyReversed<'_, '_> {
  fn predecessors(&self, node: Self::Node) -> <Self as GraphPredecessors<'_>>::Iter {
    Box::new(
      self.body.basic_blocks()[node]
        .terminator()
        .successors()
        .filter(|bb| !self.unreachable.contains(**bb))
        .copied(),
    )
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
  /// Compute control dependencies for body.
  ///
  /// This computes union of the control dependencies for each return in the body.
  pub fn build(body: &Body) -> Self {
    ControlDependencies(
      body
        .all_returns()
        .map(|loc| ControlDependencies::build_for_return(body, loc.block))
        .fold(
          SparseBitMatrix::new(body.basic_blocks().len()),
          |mut deps1, deps2| {
            for block in deps2.rows() {
              if let Some(set) = deps2.row(block) {
                deps1.union_row(block, set);
              }
            }
            deps1
          },
        ),
    )
  }

  /// Compute control dependencies from post-dominator frontier.
  ///
  /// Frontier algorithm from "An Efficient Method of Computing Single Static Assignment Form", Cytron et al. 89
  fn build_for_return(
    body: &Body,
    ret: BasicBlock,
  ) -> SparseBitMatrix<BasicBlock, BasicBlock> {
    let idom = compute_immediate_post_dominators(body, ret);
    log::debug!("idom={idom:?}");

    let edges = body
      .basic_blocks()
      .indices()
      .filter_map(|bb| Some((*idom.get(&bb)?, bb)))
      .collect::<Vec<_>>();
    let n = body.basic_blocks().len();
    let dominator_tree = VecGraph::new(n, edges);

    let traversal = iterate::post_order_from(&dominator_tree, ret);

    // Only use size = n b/c exit node shouldn't ever have a dominance frontier
    let mut df = SparseBitMatrix::new(n);
    for x in traversal {
      let local = body.predecessors()[x].iter().copied();
      let up = dominator_tree
        .successors(x)
        .iter()
        .flat_map(|z| df.row(*z).into_iter().flat_map(|set| set.iter()));
      let frontier = local
        .chain(up)
        .filter(|y| idom.get(y).map(|yd| *yd != x).unwrap_or(false))
        .collect::<Vec<_>>();

      for y in frontier {
        df.insert(x, y);
      }
    }

    df
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
      let control_deps = ControlDependencies::build(body);

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
