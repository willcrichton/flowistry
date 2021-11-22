use test_env_log::test;

mod utils;

#[test]
fn basic_slice_constant() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `(1)`;]`
  `[let `[y]` = `[`[x]` + 2]`;]`
  `[let `[z]` = `[y]`;]`
}
"#;

  utils::forward_slice(src);
}

#[test]
fn basic_slice_variable() {
  let src = r#"
fn main() {
  `(let `[mut x]` = `[1]`;)`
  `[let `[y]` = `[`[x]` + 2]`;]`
  `[let `[z]` = `[y]`;]`
}
"#;

  utils::forward_slice(src);
}

#[test]
fn basic_unused() {
  let src = r#"
fn main() {
  `(let `[x]` = `[1]`;)`
  let y = 1 + 2;
  `[let `[z]` = `[`[x]` + y]`;]`
}
"#;

  utils::forward_slice(src);
}

#[test]
fn basic_update() {
  let src = r#"
fn main() {
  `(let `[mut x]` = `[1]`;)`
  `[`[x += 1]`;]`
  `[let `[y]` = `[x]`;]`
}
"#;

  utils::forward_slice(src);
}

#[test]
fn condition() {
  let src = r#"
fn main() {
  `(let `[x]` = `[1]`;)`
  let y = 2;
  `[let `[z]` = if true {
    `[x]`
  } else {
    y
  };]`
  `[let `[w]` = `[z]`;]`
}"#;

  utils::forward_slice(src);
}

#[test]
fn pointer_write() {
  let src = r#"
fn main() {
  `(let `[mut x]` = `[1]`;)`
  `[let `[y]` = `[&mut x]`;]`
  `[`[*y += 2]`;]`
}
"#;

  utils::forward_slice(src);
}

#[test]
fn function_params() {
  let src = r#"
fn foo(`(x)`: i32) {
  `[let `[y]` = `[`[x]` + 1]`;]`
  `[let `[z]` = `[y]`;]`
}
fn main() {}
"#;

  utils::forward_slice(src);
}

#[test]
fn struct_param() {
  let src = r#"
struct Point(i32, i32);
fn foo(`(p)`: &mut Point) {
  `[`[p.0 += 1]`;]`
}
fn main() {}
"#;

  utils::forward_slice(src);
}
