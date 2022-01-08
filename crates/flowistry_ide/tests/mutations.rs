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
  x.0 += 1;
  `[`[x.1 += 1]`;]`
  `[`[`(&x.1)`]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn tuple_field_mutable_ref_mutation() {
  let src = r#"
fn main() {
  `[let `(mut y)` = `[0]`;]`
  let mut x = (0, &mut y);
  x.0 += 1;
  `[`[*x.1 += 1]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn update_mutable_borrow() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[1]`;]`
  let y = &mut x;
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
  `[`[update(&mut x)]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_write_find_whole_struct() {
  let src = r#"
fn main() {
  struct Foo { x: i32, y: i32 }
  `[let `(mut x)` = `[Foo { x: 1, y: 2 }]`;]`
  `[`[x.y = 3]`;]`
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
  `[`[`(&x.y)`]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_method_mutable_self() {
  let src = r#"
struct Foo();
impl Foo { fn bar(&mut self) {} fn baz(&self) {} }
fn main() {
  `[let `(mut x)` = `[Foo {}]`;]`
  `[`[x.bar()]`;]`
  x.baz();
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_mut_ptr_field() {
  let src = r#"
fn main() {
  struct Foo<'a>(&'a mut i32);
  `[let `(mut x)` = `[1]`;]`
  let f = Foo(&mut x);
  `[`[*f.0 += 1]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_mut_ptr_function() {
  let src = r#"
struct Foo<'a>(&'a mut i32);
fn foo(f: Foo) {}

fn main() {
  `[let `(mut x)` = `[0]`;]`
  let f = Foo(&mut x);
  `[`[foo(f)]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn struct_mut_ptr_method() {
  let src = r#"
struct Foo<'a>(&'a mut i32);
fn foo(f: Foo) {}

fn main() {
  `[let `(mut x)` = `[0]`;]`
  let f = Foo(&mut x);
  `[`[foo(f)]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn mut_child_closure() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[0]`;]`
  let mut mutate_x = || {
    x += 1;
  };
  `[`[mutate_x()]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn array_mut_ptr_mutate_index() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[0]`;]`
  let mut y = [&mut x];
  `[`[*y[0] += 1]`;]`
}
"#;

  utils::find_mutations(src);
}

#[test]
fn vec_mut_ptr_mutate_index() {
  let src = r#"
fn main() {
  `[let `(mut x)` = `[0]`;]`
  `[let mut y = `[vec![&mut x]]`;]`
  `[`[*y[0] += 1]`;]`
}
"#;

  utils::find_mutations(src);
}
