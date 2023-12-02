#![feature(rustc_private)]

extern crate either;
extern crate rustc_middle;

use either::Either;
use flowistry::pdg::graph::{DepEdge, DepGraph, DepNode};
use itertools::Itertools;
use petgraph::{
  algo::DfsSpace,
  graph::DiGraph,
  visit::{GraphBase, Visitable},
};
use rustc_middle::{mir::TerminatorKind, ty::TyCtxt};
use rustc_utils::{mir::borrowck_facts, source_map::find_bodies::find_bodies, PlaceExt};

fn pdg(
  input: impl Into<String>,
  f: impl for<'tcx> FnOnce(TyCtxt<'tcx>, DepGraph<'tcx>) + Send,
) {
  let _ = env_logger::try_init();
  flowistry::test_utils::compile(input, move |tcx| {
    let def_id = find_bodies(tcx)
      .into_iter()
      .map(|(_, body_id)| tcx.hir().body_owner_def_id(body_id))
      .find(|def_id| match tcx.opt_item_name(def_id.to_def_id()) {
        Some(name) => name.as_str() == "main",
        None => false,
      })
      .expect("Missing main");

    let pdg = flowistry::pdg::compute_pdg(tcx, def_id);
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
        let body_with_facts =
          borrowck_facts::get_body_with_borrowck_facts(tcx, at.root().function);
        Some(vec![(place.to_string(tcx, &body_with_facts.body)?, node)])
      }
      DepNode::Op { at } => {
        let root = at.root();
        let mut pairs = vec![(
          tcx.opt_item_name(root.function.to_def_id())?.to_string(),
          node,
        )];

        let body_with_facts =
          borrowck_facts::get_body_with_borrowck_facts(tcx, root.function);
        let stmt = body_with_facts
          .body
          .stmt_at(root.location.expect_location());
        if let Either::Right(term) = stmt {
          if let TerminatorKind::Call { func, .. } = &term.kind {
            if let Some((def_id, _)) = func.const_fn_def() {
              pairs.push((tcx.item_name(def_id).to_string(), node));
            }
          }
        }

        Some(pairs)
      }
    })
    .flatten()
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
        if std::env::var("VIZ").is_ok() {
            g.generate_graphviz(format!("../../target/{}.pdf", stringify!($name))).unwrap();
        }

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
      let c = foo(a);
      let b = c;
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

pdg_test! {
  external_funcs,
  {
    fn main() {
      let mut v = vec![1, 2, 3];
      v.push(4);
      v.len();
    }
  },
  (push -> v),
  (len -/> v),
  (push -> len)
}

pdg_test! {
  function_cloning,
  {
    fn id(t: i32) -> i32 { t }

    fn main() {
      let x = 1;
      let y = 2;

      let a = id(x);
      let b = id(y);
    }
  },
  (x -/> b)
}

pdg_test! {
  closure_simple,
  {
    fn main() {
      let a = 0;
      let b = 1;
      let c = 2;
      let d = 3;
      let f = (|x, y| {
        let e = a;
        b + x
      })(c, d);
    }
  },
  (a -/> f),
  (d -/> f),
  (b -> f),
  (c -> f)
}

pdg_test! {
  cfa_simple,
  {
    fn call(f: impl Fn() -> i32)  -> i32 { f() }
    fn main() {
      let a = 0;
      let b = 1;
      let d = call(|| {
        let c = a;
        b
      });
    }
  },
  // TOD: (a -/> d),
  (b -> d)
}

pdg_test! {
  async_simple,
  {
    async fn main() {
      let a = 1;
      let b = a;
      let c = a;
    }
  },
  (a -> b),
  (a -> c),
  (b -/> c)
}

pdg_test! {
  async_inline,
  {
    async fn foo(x: i32, y: i32) -> i32 {
      x
    }

    async fn main() {
      let a = 1;
      let b = 2;
      let c = foo(a, b).await;
    }
  },
  (a -> c),
  (b -/> c)
}
