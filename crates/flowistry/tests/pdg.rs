#![feature(rustc_private)]

use flowistry::pdg::graph::DepGraph;

fn pdg(input: impl Into<String>, f: impl FnOnce(DepGraph<'_>) + Send) {
  env_logger::init();
  flowistry::test_utils::compile_body(input, move |tcx, body_id, body_with_facts| {
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
