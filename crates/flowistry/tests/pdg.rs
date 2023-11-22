#![feature(rustc_private)]

use flowistry::pdg::graph::DepGraph;
use log::debug;
use rustc_utils::{mir::borrowck_facts, source_map::find_bodies::find_bodies, BodyExt};

fn pdg(input: impl Into<String>, f: impl FnOnce(DepGraph<'_>) + Send) {
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
    f(pdg)
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

#[test]
fn simple() {
  let input = r#"
fn main() {
  let mut x = 1;
  let y = if x > 0 {
    2
  } else {
    3
  };
  let z = y;
}
"#;
  pdg(input, |_graph| {
    // println!("{graph:#?}");
  })
}

#[test]
fn aliases() {
  let input = r#"
fn main() {
  let mut x = 1;
  let y = &mut x;
  *y += 1;
  let z = x;
}"#;
  pdg(input, |_graph| {
    // todo
  });
}

#[test]
fn fields() {
  let input = r#"
fn main() {
  let mut x = (1, 2);
  x.0 += 1;
  let y = x.0;
  let z = x.1;
  x = (3, 4);
  let w = x.0;
}"#;
  pdg(input, |_graph| {
    // todo
  });
}

#[test]
fn inline_basic() {
  let input = r#"
fn foo(x: i32) -> i32 {
  let y = x + 1;
  y
}
fn main() {
  let a = 1;
  let b = foo(a);
}"#;
  pdg(input, |_graph| {
    // todo
  });
}

#[test]
fn inline_refs() {
  let input = r#"
fn foo(x: &mut i32) {
  *x += 1;  
}
fn main() {
  let mut a = 1;
  foo(&mut a);
  let b = a;
}"#;
  pdg(input, |_graph| {
    // todo
  });
}

#[test]
fn inline_fields() {
  let input = r#"
fn foo(x: &mut (i32, i32)) {
  x.0 += 1;  
}
fn main() {
  let mut a = (0, 1);
  foo(&mut a);
  let b = a.0;
  let c = a.1;
}"#;
  pdg(input, |_graph| {
    // todo
  })
}
