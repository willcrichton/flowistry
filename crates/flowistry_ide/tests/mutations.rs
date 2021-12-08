use test_env_log::test;

mod utils;

#[test]
fn basic_variable_reassign() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[1]`;]`
  `[`[x = 2]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn tuple_field_write_find_whole_tuple() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[(0, 0)]`;]`
  `[`[x.0 += 1]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn tuple_field_write_find_only_field() {
  let src = r#"
fn main() {
  `[let mut x = `[(0, 0)]`;]`
  `[`[x.0 += 1]`;]`
  x.1 += 1;
  `(x.0)`;
}
"#;

  utils::find_mutations(src);
}

#[test]
fn update_mutable_borrow() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[1]`;]`
  `[let y = `[&mut x]`;]`
  `[`[*y += 1]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn mutable_function_arg() {
  let src = r#"
fn update(x: &mut i32) {}
fn main() {
  `[let `(mut x)` = `[1]`;]`
  `[update(`[&mut x]`);]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_write_find_whole_struct() {
  let src = r#"
fn main() {
  struct Foo { x: i32, y: i32 }
  `[let mut x = `[Foo { x: 1, y: 2 }]`;]`
  `[`[x.y = 3]`;]`
  `(x)`;
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_write_find_only_field() {
  let src = r#"
fn main() {
  struct Foo { x: i32, y: i32 }
  `[let mut x = `[Foo { x: 1, y: 2 }]`;]`
  `[`[x.y = 3]`;]`
  x.x = 3;
  `(x.y)`;
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_method_mutable_self() {
  let src = r#"
struct Foo();
impl Foo { fn bar(&mut self) {} }
fn main() {
  `[let mut x = `[Foo {}]`;]`
  `[`[x.bar()]`;]`
  `(x)`;
}
"#;

  utils::find_mutations(src);
}
