use rustc_data_structures::graph::{
  self, dominators, iterate, vec_graph::VecGraph, WithSuccessors,
};
use rustc_index::{
  bit_set::{BitIter, BitSet, HybridBitSet, SparseBitMatrix},
  vec::IndexVec,
};
use rustc_middle::mir::*;
use smallvec::SmallVec;
use std::{fmt, iter, option};

#[derive(Clone)]
pub struct BodyReversed<'tcx> {
  body: Body<'tcx>,
  exit_node: BasicBlock,
  exit_set: BitSet<BasicBlock>,
  dummy_set: BitSet<BasicBlock>,
}

pub fn compute_post_dominators(body: Body) -> (dominators::Dominators<BasicBlock>, BodyReversed) {
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

  (dominators::dominators(graph.clone()), graph)
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
      write!(f, "{:?}: {{", bb)?;
      for (j, bb2) in bbs.iter().enumerate() {
        if j > 0 {
          write!(f, ", ")?;
        }
        write!(f, "{:?}", bb2)?;
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

  // pub fn is_dependent(&self, child: BasicBlock, parent: BasicBlock) -> bool {
  //   self
  //     .0
  //     .row(parent)
  //     .map(|row| row.contains(child))
  //     .unwrap_or(false)
  // }
}
