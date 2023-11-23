#![feature(rustc_private)]

extern crate rustc_middle;

use flowistry::pdg::graph::{DepEdge, DepGraph, DepNode};
use itertools::Itertools;
use log::debug;
use petgraph::{
  algo::DfsSpace,
  graph::DiGraph,
  visit::{GraphBase, Visitable},
};
use rustc_middle::ty::TyCtxt;
use rustc_utils::{
  mir::borrowck_facts, source_map::find_bodies::find_bodies, BodyExt, PlaceExt,
};

fn pdg(
  input: impl Into<String>,
  f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, DepGraph<'tcx>) + Send,
) {
  let _ = env_logger::try_init();
  flowistry::test_utils::compile(input, move |tcx| {
    let (body_id, def_id) = find_bodies(tcx)
      .into_iter()
      .map(|(_, body_id)| (body_id, tcx.hir().body_owner_def_id(body_id)))
      .find(|(_, def_id)| tcx.item_name(def_id.to_def_id()).as_str() == "main")
      .expect("Missing main");

    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
    debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

    let pdg = flowistry::pdg::compute_pdg(tcx, body_id, body_with_facts);
    f(tcx, pdg)
  })
}

#[allow(unused)]
fn viz(g: &DepGraph<'_>) {
  g.generate_graphviz(format!(
    "{}/../../target/graph.pdf",
    env!("CARGO_MANIFEST_DIR")
  ))
  .unwrap();
}

fn connects<'tcx>(
  tcx: TyCtxt<'tcx>,
  g: &DepGraph<'tcx>,
  space: &mut DfsSpace<
    <DiGraph<DepNode<'tcx>, DepEdge> as GraphBase>::NodeId,
    <DiGraph<DepNode<'tcx>, DepEdge> as Visitable>::Map,
  >,
  src: &str,
  dst: &str,
) -> bool {
  let mut node_map = g
    .graph
    .node_indices()
    .filter_map(|node| match &g.graph[node] {
      DepNode::Place { place, at } => {
        let def_id = at.function.as_local()?;
        let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
        Some((place.to_string(tcx, &body_with_facts.body)?, node))
      }
      DepNode::Op(loc) => Some((tcx.item_name(loc.function).to_string(), node)),
    })
    .into_group_map();

  let mut lookup = |mut k: &str| {
    k = k.trim_matches(|c| c == '(' || c == ')');
    node_map.remove(k).unwrap_or_else(|| {
      panic!(
        "Could not find node `{k}`. Options were: {:?}",
        node_map.keys().collect::<Vec<_>>()
      )
    })
  };
  let srcs = lookup(src);
  let dsts = lookup(dst);

  srcs.iter().any(|src| {
    dsts
      .iter()
      .any(|dst| petgraph::algo::has_path_connecting(&g.graph, *src, *dst, Some(space)))
  })
}

macro_rules! pdg_constraint {
  (($src:tt -> $dst:tt), $($arg:expr),*) => {{
    let src = stringify!($src);
    let dst = stringify!($dst);
    assert!(connects($($arg),*, src, dst), "{src} -> {dst}")
  }};
  (($src:tt -/> $dst:tt), $($arg:expr),*) => {{
    let src = stringify!($src);
    let dst = stringify!($dst);
    assert!(!connects($($arg),*, src, dst), "{src} -/> {dst}")
  }};
}

macro_rules! pdg_test {
  ($name:ident, { $($i:item)* }, $($cs:tt),*) => {
    #[test]
    fn $name() {
      let input = stringify!($($i)*);
      pdg(input, |tcx, g| {
        // g.generate_graphviz("../../target/graph.pdf").unwrap();
        let mut space = DfsSpace::new(&g.graph);
        $(pdg_constraint!($cs, tcx, &g, &mut space));*
      })
    }
  }
}

pdg_test! {
  simple,
  {
    fn main() {
      let mut x = 1;
      let y = if x > 0 {
        2
      } else {
        3
      };
      let z = y;
    }
  },
  (x -> y),
  (y -/> x),
  (y -> z),
  (z -/> y),
  (z -/> x)
}

pdg_test! {
  aliases,
  {
    fn main() {
      let mut x = 1;
      let y = &mut x;
      *y += 1;
      let z = x;
    }
  },
  (x -> y),
  (x -> z),
  (y -> z)
}

pdg_test! {
  fields,
  {
    fn main() {
      let mut x = (1, 2);
      x.0 += 1;
      let y = x.0;
      let z = x.1;
      x = (3, 4);
      let w = x.0;
    }
  },
  ((x.0) -> y),
  ((x.1) -> z),
  ((x.0) -/> z),
  ((x.1) -/> y)
}

pdg_test! {
  inline_simple,
  {
    fn foo(x: i32) -> i32 {
      let y = x + 1;
      y
    }
    fn main() {
      let a = 1;
      let b = foo(a);
    }
  },
  (a -> x),
  (x -> y),
  (a -> y),
  (y -> b),
  (a -> b),
  (a -> foo),
  (foo -> b)
}

pdg_test! {
  inline_refs,
  {
    fn foo(x: &mut i32) {
      *x += 1;
    }
    fn main() {
      let mut a = 1;
      foo(&mut a);
      let b = a;
    }
  },
  (x -> a),
  (foo -> b)
}

pdg_test! {
  inline_fields,
  {
    fn foo(x: &mut (i32, i32)) {
      x.0 += 1;
    }
    fn main() {
      let mut a = (0, 1);
      foo(&mut a);
      let b = a.0;
      let c = a.1;
    }
  },
  (foo -> b),
  (foo -/> c)
}
