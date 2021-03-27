use rustc_data_structures::graph::{self, dominators};
use rustc_index::bit_set::{BitIter, BitSet};
use rustc_middle::mir::*;
use smallvec::SmallVec;
use std::{iter, option};

pub struct BodyReversed<'tcx> {
  body: Body<'tcx>,
  exit_node: BasicBlock,
  exit_set: BitSet<BasicBlock>,
  dummy_set: BitSet<BasicBlock>,
}

pub fn compute_post_dominators(body: Body) -> dominators::Dominators<BasicBlock> {
  let nblocks = body.basic_blocks().len();
  let exit_node = BasicBlock::from_usize(nblocks);

  let mut exit_set = BitSet::new_empty(nblocks);
  let dummy_set = BitSet::new_empty(nblocks);
  let exit_nodes = body
    .basic_blocks()
    .iter_enumerated()
    .filter_map(|(bb_index, bb_data)| {
      // Specifically DO NOT check that #successors == 0, b/c that would include
      // panic/unwind blocks which ruin the value of the post-dominator tree
      if let TerminatorKind::Return = bb_data.terminator().kind {
        Some(bb_index)
      } else {
        None
      }
    });
  for node in exit_nodes {
    exit_set.insert(node);
  }

  let graph = BodyReversed {
    body,
    exit_node,
    exit_set,
    dummy_set,
  };
  dominators::dominators(graph)
}

impl<'tcx> graph::DirectedGraph for BodyReversed<'tcx> {
  type Node = BasicBlock;
}

impl<'tcx> graph::WithNumNodes for BodyReversed<'tcx> {
  fn num_nodes(&self) -> usize {
    // +1 for exit node
    self.body.basic_blocks().len() + 1
  }
}

impl<'tcx> graph::WithStartNode for BodyReversed<'tcx> {
  fn start_node(&self) -> Self::Node {
    self.exit_node
  }
}

impl<'tcx> graph::WithSuccessors for BodyReversed<'tcx> {
  fn successors(&self, node: Self::Node) -> <Self as graph::GraphSuccessors<'_>>::Iter {
    if node == self.exit_node {
      SmallVec::new().into_iter().chain(self.exit_set.iter())
    } else {
      self.body.predecessors()[node]
        .clone()
        .into_iter()
        .chain(self.dummy_set.iter())
    }
  }
}

impl<'a, 'b> graph::GraphSuccessors<'b> for BodyReversed<'a> {
  type Item = BasicBlock;
  type Iter = iter::Chain<smallvec::IntoIter<[BasicBlock; 4]>, BitIter<'b, BasicBlock>>;
}

impl<'tcx, 'graph> graph::GraphPredecessors<'graph> for BodyReversed<'tcx> {
  type Item = BasicBlock;
  type Iter = iter::Chain<option::IntoIter<BasicBlock>, iter::Cloned<Successors<'graph>>>;
}

impl<'tcx> graph::WithPredecessors for BodyReversed<'tcx> {
  #[inline]
  fn predecessors(&self, node: Self::Node) -> <Self as graph::GraphPredecessors<'_>>::Iter {
    assert!(node != self.exit_node);

    let exit_pred = if self.exit_set.contains(node) {
      Some(self.exit_node)
    } else {
      None
    };
    let preds = self.body.basic_blocks()[node]
      .terminator()
      .successors()
      .cloned();

    exit_pred.into_iter().chain(preds)
  }
}
