#![allow(dead_code)]

use itertools::Itertools;
use log::trace;
use petgraph::{
  graph::{DiGraph, IndexType, Neighbors, NodeIndex},
  unionfind::UnionFind,
  visit::{depth_first_search, Control, DfsEvent, EdgeRef, NodeIndexable},
  EdgeDirection, EdgeType, Graph,
};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::bit_set::{HybridBitSet, SparseBitMatrix};

pub trait GraphExt<E, Ix> {
  fn successors<'a>(&'a self, n: NodeIndex<Ix>) -> Neighbors<'a, E, Ix>;
  fn predecessors<'a>(&'a self, n: NodeIndex<Ix>) -> Neighbors<'a, E, Ix>;
}

impl<N, E, Ty, Ix> GraphExt<E, Ix> for Graph<N, E, Ty, Ix>
where
  Ty: EdgeType,
  Ix: IndexType,
{
  fn successors<'a>(&'a self, n: NodeIndex<Ix>) -> Neighbors<'a, E, Ix> {
    self.neighbors_directed(n, EdgeDirection::Outgoing)
  }

  fn predecessors<'a>(&'a self, n: NodeIndex<Ix>) -> Neighbors<'a, E, Ix> {
    self.neighbors_directed(n, EdgeDirection::Incoming)
  }
}

pub fn find_cut<N, E, Ix>(g: &DiGraph<N, E, Ix>) -> Option<Vec<Vec<NodeIndex<Ix>>>>
where
  Ix: IndexType,
  N: Clone + std::fmt::Debug,
  E: Clone,
{
  let mut g = g.clone();
  let mut deleted = HashSet::default();
  let order = petgraph::algo::toposort(&g, None).unwrap();
  let k = order.len();
  let mut ranks = order
    .into_iter()
    .enumerate()
    .map(|(i, n)| {
      let rank = if i > k / 2 { k - i } else { i };
      (n, k - rank)
    })
    .collect::<HashMap<_, _>>();
  let threshold = k / 6;

  while g.edge_count() > 0 {
    let to_delete = g
      .node_indices()
      .max_by_key(|n| {
        (
          g.successors(*n).count() + g.predecessors(*n).count(),
          ranks[n],
        )
      })
      .unwrap();

    deleted.insert(to_delete);
    let edges = g
      .edges_directed(to_delete, EdgeDirection::Outgoing)
      .chain(g.edges_directed(to_delete, EdgeDirection::Incoming))
      .map(|e| e.id())
      .collect::<HashSet<_>>();
    g.retain_edges(|_, e| !edges.contains(&e));

    let mut components = connected_components(&g);
    components.retain(|c| c.len() > 1 || !deleted.contains(&c[0]));
    let (mut large, small): (Vec<_>, Vec<_>) =
      components.into_iter().partition(|v| v.len() >= threshold);
    if large.len() > 1 {
      large.push(small.into_iter().flatten().chain(deleted).collect());
      return Some(large);
    }
    // let sizes = components.iter().map(|v| v.len()).collect::<Vec<_>>();
    // println!(
    //   "removed {:?}, new components: {:?}, avg size: {:.1}",
    //   g.node_weight(to_delete).unwrap(),
    //   sizes,
    //   (sizes.iter().sum::<usize>() as f64) / (sizes.len() as f64)
    // );
  }

  None
}

pub fn subgraph<N, E, Ix>(
  g: &DiGraph<N, E, Ix>,
  nodes: &[NodeIndex<Ix>],
) -> DiGraph<NodeIndex<Ix>, (), Ix>
where
  Ix: IndexType,
{
  let mut g2 = DiGraph::default();
  let node_map = nodes
    .iter()
    .map(|n| (*n, g2.add_node(*n)))
    .collect::<HashMap<_, _>>();
  for n in node_map.keys() {
    for n2 in g.successors(*n) {
      g2.add_edge(node_map[n], node_map[&n2], ());
    }
  }
  g2
}

pub fn connected_components<N, E, Ix>(g: &DiGraph<N, E, Ix>) -> Vec<Vec<NodeIndex<Ix>>>
where
  Ix: IndexType,
{
  let mut vertex_sets = UnionFind::<NodeIndex<Ix>>::new(g.node_count());
  for edge in g.edge_references() {
    vertex_sets.union(edge.source(), edge.target());
  }

  let pairs = vertex_sets
    .into_labeling()
    .into_iter()
    .zip(g.node_indices());

  pairs.into_group_map().into_values().collect()
}

// Implementation copied almost verbatim from NetworkX:
// https://networkx.org/documentation/stable/_modules/networkx/algorithms/dag.html#transitive_reduction
pub fn transitive_reduction<N, E, Ix>(g: &DiGraph<N, E, Ix>) -> DiGraph<N, (), Ix>
where
  N: Clone,
  Ix: IndexType,
{
  let mut descendants = HashMap::default();
  let mut g_reduced = DiGraph::<N, (), Ix>::default();
  for node in g.raw_nodes() {
    g_reduced.add_node(node.weight.clone());
  }

  let mut check_count = g
    .node_indices()
    .map(|n| (n, g.neighbors_directed(n, EdgeDirection::Incoming).count()))
    .collect::<HashMap<_, _>>();
  for u in g.node_indices() {
    let mut u_nbrs = g.neighbors(u).collect::<HashSet<_>>();
    for v in g.neighbors(u) {
      if u_nbrs.contains(&v) {
        let d = descendants.entry(v).or_insert_with(|| {
          let mut set = HashSet::default();
          depth_first_search(&g, [v], |event| -> Control<()> {
            if let DfsEvent::TreeEdge(_x, y) = event {
              set.insert(y);
            }
            Control::Continue
          });
          set
        });
        u_nbrs = &u_nbrs - d;
      }

      let count = check_count.get_mut(&v).unwrap();
      *count -= 1;
      if *count == 0 {
        descendants.remove_entry(&v).unwrap();
      }
    }

    for v in u_nbrs {
      g_reduced.add_edge(u, v, ());
    }
  }

  g_reduced
}

fn pick2_mut<T>(v: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
  assert!(i != j);
  let lower = i.min(j);
  let upper = i.max(j);
  let mut it = v.iter_mut();

  let lower_ref = it.nth(lower).unwrap();
  let upper_ref = it.nth(upper - lower - 1).unwrap();

  if i < j {
    (lower_ref, upper_ref)
  } else {
    (upper_ref, lower_ref)
  }
}

fn make_modularity<'a, N, E, Ix>(
  g: &'a DiGraph<N, E, Ix>,
  resolution: f64,
) -> impl Fn(&[HybridBitSet<usize>]) -> f64 + 'a
where
  Ix: IndexType,
{
  let mut adj_mtx = SparseBitMatrix::new(g.node_count());
  for edge in g.raw_edges() {
    adj_mtx.insert(edge.source().index(), edge.target().index());
  }

  let (out_degree, in_degree): (Vec<_>, Vec<_>) = g
    .node_indices()
    .map(|n| {
      (
        g.successors(n).count() as f64,
        g.predecessors(n).count() as f64,
      )
    })
    .unzip();

  let m = g.edge_count() as f64;

  let contribution = move |community: &HybridBitSet<usize>| {
    let mut l_c = 0;
    for u in community.iter() {
      if let Some(set) = adj_mtx.row(u) {
        let mut community = community.clone();
        community.intersect(set);
        l_c += community.iter().count();
      }
    }

    let k_c_out = community.iter().map(|n| out_degree[n]).sum::<f64>();
    let k_c_in = community.iter().map(|n| in_degree[n]).sum::<f64>();
    (l_c as f64 - resolution * k_c_out * k_c_in / m) / m
  };

  move |communities| communities.iter().map(|c| contribution(c)).sum::<f64>()
}

pub fn naive_greedy_modularity_communities<N, E, Ix>(
  g: &DiGraph<N, E, Ix>,
  resolution: f64,
) -> Vec<Vec<NodeIndex<Ix>>>
where
  Ix: IndexType,
{
  let size = g.node_count();
  let modularity = make_modularity(g, resolution);

  let mut communities = (0 .. size)
    .map(|i| {
      let mut set = HybridBitSet::new_empty(size);
      set.insert(i);
      set
    })
    .collect::<Vec<_>>();
  let mut merges = Vec::new();
  let mut old_modularity = f64::MIN;
  let mut new_modularity = modularity(&communities);

  while new_modularity > old_modularity {
    old_modularity = new_modularity;
    let mut trial_communities = communities.clone();
    let mut to_merge = None;
    for (i, u) in communities.iter().enumerate() {
      for (j, v) in communities.iter().enumerate().filter(|(j, _)| i > *j) {
        trial_communities[j].union(u);
        trial_communities[i].clear();

        let trial_modularity = modularity(&trial_communities);
        if trial_modularity >= new_modularity {
          if trial_modularity > new_modularity {
            trace!("found good one (trial {trial_modularity:?} new {new_modularity:?}");
            new_modularity = trial_modularity;
            to_merge = Some((i, j, new_modularity - old_modularity));
          } else if let Some((oi, oj, _)) = to_merge {
            if i.min(j) < oi.min(oj) {
              new_modularity = trial_modularity;
              to_merge = Some((i, j, new_modularity - old_modularity));
            }
          }
        }

        trial_communities[i] = u.clone();
        trial_communities[j] = v.clone();
      }
    }

    if let Some((i, j, dq)) = to_merge {
      merges.push((i, j, dq));
      let (ci, cj) = pick2_mut(&mut communities, i, j);
      ci.union(cj);
      communities.remove(j);

      trace!("new:{new_modularity:?} old:{old_modularity:?}");
    }
  }

  communities
    .into_iter()
    .map(|c| c.iter().map(NodeIndex::new).collect())
    .collect()
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_pick2_mut() {
    let mut v = vec![1, 2, 3, 4];
    let (x, y) = pick2_mut(&mut v, 0, 2);
    assert_eq!(*x, 1);
    assert_eq!(*y, 3);

    let (x, y) = pick2_mut(&mut v, 3, 0);
    assert_eq!(*x, 4);
    assert_eq!(*y, 1);
  }
}
