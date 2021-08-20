use test_env_log::test;

mod utils;

#[test]
fn basic_slice_constant() {
  let src = r#"
fn main() {
  let `[mut x]` = `(1)`;
  let `[y]` = `[`[x]` + 2]`;
  let `[z]` = `[y]`;
}
"#;

  utils::forward_slice(src);
}

#[test]
fn basic_slice_variable() {
  let src = r#"
fn main() {
  let `(mut x)` = `[1]`;
  let `[y]` = `[`[x]` + 2]`;
  let `[z]` = `[y]`;
}
"#;

  utils::forward_slice(src);
}

#[test]
fn basic_unused() {
  let src = r#"
fn main() {
  let `(x)` = `[1]`;
  let y = 1 + 2;
  let `[z]` = `[`[x]` + y]`;
}
"#;

  utils::forward_slice(src);
}

#[test]
fn pointer_write() {
  let src = r#"
fn main() {
  let `(mut x)` = `[1]`;
  let `[y]` = `[&mut x]`;
  `[*y += 2]`;
}
"#;

  utils::forward_slice(src);
}
