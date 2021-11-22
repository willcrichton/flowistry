use test_env_log::test;
use utils::backward_slice;

mod utils;

#[test]
fn variable_read() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn variable_assign() {
  // should not include line 1 b/c of overriding assignment
  let src = r#"
fn main() {
  let `[mut x]` = 1;
  `[`[x = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn variable_reassign() {
  // should include line 1 b/c of reassign
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[x += 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn variable_read_multiple() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[`(`[x]` + `[y]`)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn variable_slice_from_middle() {
  // should not include x += 3
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[x += 2]`;]`
  `[`(x)`;]`
  x += 3;
}
"#;

  backward_slice(src);
}

// FIXME: y not part of slice
// #[test]
// fn variable_select_lhs() {
//   let src = r#"
// fn main() {
//   `[let `[x]` = `[1]`;]`
//   `[let `(y)` = `[x]`;]`
// }
// "#;

//   backward_slice(src);
// }

#[test]
fn if_both_paths_relevant() {
  let src = r#"
fn main() {
  `[let `[x]` = if `[true]` {
    `[1]`
  } else {
    `[2]`
  };]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn if_all_paths_irrelevant() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  let y = if true { 1 } else { 2 };
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn if_one_path_relevant() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  let mut y = 2;
  `[if `[true]` {
    `[`[x = 3]`;]`
  } else {
    y = x;
  }]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn while_cond_relevant() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[while `[`[x]` < `[y]`]` {
    `[`[x += 1]`;]`
  }]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn while_cond_irrelevant() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  let mut y = 2;
  while x < y {
    y -= 1;
  }
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn loop_break() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[0]`;]`
  `[loop {
    `[if `[`[x]` == 10]` {
      break;
    }]`
    `[`[x += 1]`;]`
  }]`
  `[`(x)`;]`
}
"#;

  // TODO: possible to include the `break`?
  backward_slice(src);
}

#[test]
fn tuple_write_field_read_whole() {
  // should include line 1 because x.1 is relevant
  let src = r#"
fn main() {
  `[let `[mut x]` = `[(0, 1)]`;]`
  `[`[x.0 = 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_write_field_read_field() {
  // shouldn't include line 1 b/c x.1 isn't relevant
  let src = r#"
fn main() {
  let `[mut x]` = (0, 1);
  `[`[x.0 = 1]`;]`
  `[`(x.0)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_field_independence_owned() {
  // shouldn't include line 1 b/c x.1 isn't relevant
  let src = r#"
fn main() {
  `[let `[mut x]` = `[(0, 1)]`;]`
  `[`[x.0 += 1]`;]`
  x.1 += 1;
  `[`(x.0)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_field_independence_ref() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[&mut `[(0, 1)]`]`;]`
  `[`[x.0 += 1]`;]`
  x.1 += 1;
  `[`(x.0)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_field_independence_ref2() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[&mut `[(0, 1)]`]`;]`
  x.0 += 1;
  `[`[x.1 += 1]`;]`
  `[`(x.1)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_write_whole_read_whole() {
  let src = r#"
fn main() {
  let `[mut x]` = (0, 1);
  `[`[x = (2, 3)]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_write_whole_read_field() {
  let src = r#"
fn main() {
  let `[mut x]` = (0, 1);
  `[`[x = (2, 3)]`;]`
  `[`(x.0)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn struct_write() {
  let src = r#"
fn main() {
  struct Foo { x: i32, y: i32 }
  `[let `[mut x]` = `[Foo { x: 1, y: 2 }]`;]`
  `[`[x.y = 3]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn struct_mut_ptr_1() {
  let src = r#"
fn main() {
  struct Foo<'a>(&'a mut i32);
  `[let `[mut x]` = `[1]`;]`
  `[let `[f]` = `[Foo(`[&mut x]`)]`;]`
  `[`[*f.0 += 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn struct_mut_ptr_function() {
  let src = r#"
struct Foo<'a>(&'a mut i32);
fn foo(f: Foo) {}

fn main() {
  `[let `[mut x]` = `[0]`;]`
  `[let `[f]` = `[Foo(`[&mut x]`)]`;]`
  `[`[foo(`[f]`)]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn enum_write_branch_read_whole() {
  // Both if-lets should be relevant
  let src = r#"
fn main() {
  enum Foo { X(i32), Y(i32) }
  `[let `[mut x]` = `[Foo::X(1)]`;]`
  `[if `[let `[Foo::X(`[z]`)]` = `[&mut x]`]` {
    `[`[*z += 1]`;]`
  }]`
  `[if `[let `[Foo::Y(`[z]`)]` = `[&mut x]`]` {
    `[`[*z += 1]`;]`
  }]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn enum_write_branch_read_branch() {
  // Foo::Y code should be irrelevant
  let src = r#"
fn main() {
  enum Foo { X(i32), Y(i32) }
  `[let `[mut x]` = `[Foo::X(1)]`;]`
  `[if `[let `[Foo::X(`[z]`)]` = `[&mut x]`]` {
    `[`[*z += 1]`;]`
  }]`
  `[if `[let `[Foo::Y(`[z]`)]` = `[&mut x]`]` {
    `[`[*z += 1]`;]`
  }]`
  if `[let `[Foo::X(`[z]`)]` = x]` `[{
    `[`(z)`;]`
  }]`
}
"#;

  /*
   * TODO!
   * Issue is that switch on discriminant(x) adds x to relevant set,
   * and then any mutations to subfields of x are relevant.
   * Not sure what the solution is beyond some kind of fancy flow-sensitivity,
   * or maybe including discriminant(x) as a first-class PlacePrim
   */
  backward_slice(src);
}

#[test]
fn array_write() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[[0; 1]]`;]`
  `[`[x[0] = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn array_read() {
  let src = r#"
fn main() {
  `[let `[x]` = `[[0; 1]]`;]`
  `[let `[y]` = `[x[0]]`;]`
  `[`(y)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn slice_write() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[[0u8; 2]]`;]`
  `[let `[y]` = `[&mut `[`[x]`[`[..1]`]]`]`;]`
  `[`[y[0] = 0]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn slice_ptr_elem_write() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[[`[&mut x]`]]`;]`
  `[`[*y[0] = 0]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_write() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[&mut x]`;]`
  `[`[*y = 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_slice_includes_deref() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[&mut x]`;]`
  `[`[*y = 1]`;]`
  `[`(y)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_read() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  `[let `[y]` = `[&x]`;]`
  `[let `[z]` = `[`[*y]` + 1]`;]`
  `[`(z)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_increment() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[&mut x]`;]`
  `[`[*y += 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_ignore_reads() {
  // n should be ignored
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  let y = &mut x;
  let n = *y;
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_aliasing() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[&mut x]`;]`
  `[let `[z]` = `[y]`;]`
  `[`[*z = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_multiple_locations() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[2]`;]`
  `[let `[z]` = if `[true]` {
    `[&mut x]`
  } else {
    `[&mut y]`
  };]`
  `[`[*z += 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_nested() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[&mut x]`;]`
  `[let `[z]` = `[&mut y]`;]`
  `[`[**z = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_reborrow() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[&mut x]`;]`
  `[let `[z]` = `[&mut *y]`;]`
  `[`[*z = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_reborrow_nested() {
  let src = r#"
fn main() {
  `[let `[mut x]`: i32 = `[1]`;]`
  `[let `[mut y]` = `[&mut x]`;]`
  `[let `[z]` = `[&mut y]`;]`
  `[let `[w]` = `[&mut **z]`;]`
  `[`[*w = 2]`;]`
  `[`(*y)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_mutate_field() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[(1,)]`;]`
  `[let `[mut y]` = `[&mut x]`;]`
  `[`[(*y).0 = 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_copy_and_read() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[&mut x]`;]`
  `[`[*y = 2]`;]`
  `[let `[z]` = `[y]`;]`
  `[`(*z)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn pointer_mutate_pointer() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[1]`;]`
  `[let `[mut a]` = `[&mut x]`;]`
  `[let `[b]` = `[&mut a]`;]`
  `[`[*b = `[&mut y]`]`;]`
  `[`[**b = 2]`;]`
  `[`(y)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_output() {
  let src = r#"
fn foo() -> i32 { 1 }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[x += `[foo()]`]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_input() {
  let src = r#"
fn foo(x: i32) -> i32 { x }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[`[x += `[foo(`[y]`)]`]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_input() {
  // y should be relevant b/c it could be involved in computation of x
  let src = r#"
fn foo(x: &mut i32, y: i32) {}

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[`[foo(`[&mut x]`, `[y]`)]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_ref_input() {
  // call should be irrelevant b/c x can only be read
  let src = r#"
fn foo(x: &i32) {}

fn main() {
  `[let `[mut x]` = `[1]`;]`
  foo(&x);
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_input_irrelevant() {
  let src = r#"
fn foo(x: &mut i32) {}

fn main() {
  `[let `[mut x]` = `[1]`;]`
  let mut y = 2;
  foo(&mut y);
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_input_field() {
  let src = r#"
fn foo(x: (&mut i32,)) {}

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[foo(`[(`[&mut x]`,)]`)]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_input_whole() {
  let src = r#"
fn write(t: &mut (i32, i32)) {}

fn main() {
  `[let `[mut x]` = `[(1, 2)]`;]`
  `[`[write(`[&mut x]`)]`;]`
  `[`(x.0)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_output() {
  let src = r#"
fn foo(x: &mut i32) -> &mut i32 { x }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[foo(`[&mut x]`)]`;]`
  `[`[*y += 2]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_output_lifetimes() {
  let src = r#"
fn foo<'a, 'b>(x: &'a mut i32, y: &'b mut i32) -> &'b mut i32 { y }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[2]`;]`
  `[let `[z]` = `[foo(`[&mut x]`, `[&mut y]`)]`;]`
  *z += 1;
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_output_lifetimes_outlives() {
  let src = r#"
fn foo<'a, 'b: 'a>(x: &'a mut i32, y: &'b mut i32) -> &'a mut i32 { x }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[2]`;]`
  `[let `[z]` = `[foo(`[&mut x]`, `[&mut y]`)]`;]`
  `[`[*z += 1]`;]`
  `[`(y)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_output_field_read_whole() {
  let src = r#"
fn foo(x: &mut (i32, i32)) -> &mut i32 { &mut x.0 }

fn main() {
  `[let `[mut x]` = `[(0, 1)]`;]`
  `[let `[y]` = `[foo(`[&mut x]`)]`;]`
  `[`[*y += 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interprocedural_mut_output_field_read_field() {
  // Should conservatively assume returned value could be any field
  let src = r#"
fn foo(x: &mut (i32, i32)) -> &mut i32 { &mut x.0 }

fn main() {
  `[let `[mut x]` = `[(0, 1)]`;]`
  `[let `[y]` = `[foo(`[&mut x]`)]`;]`
  `[`[*y += 1]`;]`
  `[`(x.1)`;]`
}
"#;

  backward_slice(src);
}

// #[test]
// fn interprocedural_field_independence() {
//   let src = r#"
// use std::ops::AddAssign;
// struct Foo(i32, i32);
// impl Foo {
//   fn bar(`[&mut self]`) {
//     self.0.add_assign(0);
//     `[`(self.1)`;]`
//   }
// }

// fn main() {}
// "#;

//   backward_slice(src);

//   let src = r#"
//   use std::ops::AddAssign;
//   struct Foo(i32, i32);
//   impl Foo {
//     fn bar(`[&mut self]`) {
//       `[let `[a]` = `[&mut *self]`;]`
//       `[`[a.0.add_assign(0)]`;]`
//       `[`(a.1)`;]`
//     }
//   }

//   fn main() {}
//   "#;

//     backward_slice(src);
// }

#[test]
fn interprocedural_ref_output() {
  let src = r#"
fn foo(x: &i32) -> &i32 { x }

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[foo(`[&x]`)]`;]`
  `[let `[z]` = `[*y]`;]`
  `[`(z)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn function_param() {
  let src = r#"
fn foo(`[x]`: i32) {
  `[let `[y]` = `[`[x]` + 1]`;]`
  `[`(y)`;]`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn function_mut_ptr_param() {
  let src = r#"
fn foo(`[x]`: &mut i32) {
  `[`[*x = 2]`;]`
  `[let `[y]` = `[*x]`;]`
  `[`(y)`;]`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn function_mut_ptr_param_field() {
  let src = r#"
fn foo(`[x]`: (&mut i32,)) {
  `[`[*x.0 = 2]`;]`
  `[let `[y]` = `[*x.0]`;]`
  `[`(y)`;]`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn function_ret() {
  let src = r#"
fn foo(`[x]`: i32) -> i32 {
  `(x)`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn function_ret_not_sliced() {
  let src = r#"
fn foo(`[x]`: i32) -> i32 {
  `[print!("{}", `(x)`);]`
  let y = 1;
  y
}

fn main() {}
"#;

  backward_slice(src);
}

// #[test]
// fn function_lifetime_alias_equal() {
//   let src = r#"
// fn foo<'a>(x: &'a mut i32, `[y]`: &'a mut i32) {
//   let z = x;
//   *z = 1;
//   `[`(*y)`;]`
// }

// fn main() {}
// "#;

//   backward_slice(src);
// }

// #[test]
// fn function_lifetime_alias_outlives() {
//   let src = r#"
// fn foo<'a, 'b: 'a>(x: &'a mut i32, `[y]`: &'b mut i32) -> &'a mut i32 {
//   let z = x;
//   *z = 1;
//   `[`(y)`]`
// }

// fn main() {}
// "#;

//   backward_slice(src);

//   let src = r#"
// fn foo<'a, 'b>(x: &'a mut i32, `[y]`: &'b mut i32) -> &'b mut i32 {
//   let z = x;
//   *z = 1;
//   `[`(y)`]`
// }

// fn main() {}
//   "#;

//   backward_slice(src);
// }

#[test]
fn function_lifetime_alias_mut() {
  let src = r#"
fn foo<'a>(x: &'a mut i32, `[y]`: &'a i32) {
  let z = x;
  *z = 1;
  `[`(*y)`;]`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn function_param_field_independence() {
  let src = r#"
fn foo(`[x]`: &mut (i32, i32)) {
  `[`[x.0 += 1]`;]`
  x.1 += 1;
  `[`(x.0)`;]`
}

fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn closure_write_upvar() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut f]` = `[|| { x += 1; }]`;]`
  `[`[`[f]`()]`;]`
  `[`(x)`;]`
}
"#;

  // NOTE
  // Seems like when a variable is captured as an upvar than explicitly passed as &mut,
  // the MIR source map links the &mut back to the closure definition source range.
  // Hence this example has the closure defn as part of the slice, but not the others.
  backward_slice(src);
}

#[test]
fn closure_read_upvar() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  let f = || { x + 1; };
  f();
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

// TODO: should this include the closure call? that's kind of an artifact
//   of the fact that the span is contained inside `main`, so the analysis for main
//   associates the span with e.g. the closure's return value and environment
#[test]
fn closure_slice_inner_write_inner() {
  let src = r#"
fn main() {
  let x = 1;
  `[`[`[(|| {
    `[let `[y]` = `[1]`;]`
    `[`(y)`;]`
  })]`()]`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn closure_slice_inner_write_outer() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[`[(|| {
    `[`[x += 1]`;]`
    `[`(x)`;]`
  })]`()]`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn macro_read() {
  let src = r#"
fn main() {
  `[let `[x]` = `[vec![1, 2, 3]]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn macro_print() {
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  println!("{}", x);
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn str_static_lifetime() {
  // if we're not careful "c" will be considered relevant:
  //   "b" and "c" both are &'static str, and lifetime-based alias analysis
  //   says when x = (some &'static str), then it could be either "b" or "c"
  let src = r#"
fn main() {
  let `[mut x]` = "a";
  `[`[x = `["b"]`]`;]`
  print!("c");
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn macro_slice() {
  // TODO: y shouldn't be included, seems to be an artifact of macro spans
  let src = r#"
fn main() {
  `[let `[x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[println!("{} {}", `[`(x)`]`, `[y]`);]`
}
"#;

  backward_slice(src);
}

#[test]
fn generic_param() {
  let src = r#"
fn main() {}

fn foo<T>(`[t]`: T) {
  `[let `[x]` = `[t]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn string_drop_and_replace() {
  let src = r#"
fn main() {
  let `[mut x]` = String::new();
  `[`[x]` = `[String::new()]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn match_test() {
  let src = r#"
  fn main() {
    `[let `[x]` = `[1]`;]`
    `[let `[y]` = `[match `[Some(`[x]`)]` {
      `[None]` => `[1]`,
      Some(`[x]`) => `[x]`,
    }]`;]`
    `[`(y)`;]`
  }
  "#;

  backward_slice(src);
}

#[test]
fn strong_update_conditional() {
  let src = r#"
fn main() {
  let `[mut x]` = 1;
  `[`[x = 1]`;]`
  `[let `[mut y]` = `[2]`;]`
  `[let `[z]` = if `[true]` { `[&mut x]` } else { `[&mut y]` };]`
  `[`[*z += 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn tuple_copy() {
  // TODO: y.1 shoudn't be part of the slice
  //   the issue is that when we do z = y, then the collected influence of y
  //   is broadcast into every conflicting place in z. So y.1 -> z -> z.0.
  //   we need something to "match up" influences so y.1 only flows to z.1
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[(0, 0)]`;]`
  `[`[y.1 += 1]`;]`
  `[let `[z]` = `[y]`;]`
  `[`[x += `[z.0]`]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn method_simple() {
  let src = r#"
struct Foo(i32, i32);
impl Foo { fn bar(&self) -> i32 { self.0 } }

fn main() {
  `[let `[x]` = `[1]`;]`
  `[let `[y]` = `[2]`;]`
  `[let `[z]` = `[Foo(`[x]`, `[y]`)]`;]`
  `[let `[w]` = `[z.bar()]`;]`
  `[`(w)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn async_simple() {
  let src = r#"
async fn foobar(x: &mut i32) -> i32 { 0 }
async fn test() `[{
  `[let `[mut x]` = `[1]`;]`
  `[`[foobar(`[&mut x]`)]`.await]`;
  `[`(x)`;]`
}]`
fn main() {}
"#;

  backward_slice(src);
}

#[test]
fn interior_mutability_observable() {
  let src = r#"
use std::cell::RefCell;
fn main() {
  `[let `[x]` = `[RefCell::new(0)]`;]`
  `[`[`[*`[x.borrow_mut()]`]` = 1]`;]`
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn interior_mutability_not_observable() {
  let src = r#"
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
  `[let `[x]` = `[Rc::new(`[RefCell::new(0)]`)]`;]`
  let y = x.clone();
  *y.borrow_mut() = 1;
  `[`(x)`;]`
}
"#;

  backward_slice(src);
}

#[test]
fn function_ref_in_ret() {
  let src = r#"
fn foo(`[x]`: (&mut i32,)) -> `[(&mut i32,)]` {
  `[`[*x.0 = 1]`;]`
  `(x)`
}
fn main() {}
"#;

  backward_slice(src);
}
