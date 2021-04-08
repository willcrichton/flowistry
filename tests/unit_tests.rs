use rust_slicer::Range;
use utils::run;

mod utils;

#[test]
fn variable_read() {
  let src = r#"
fn main() {
  let x = 1;
  x;
}
"#;

  run(src, Range::line(3, 3, 4), vec![2, 3]);
}

#[test]
fn variable_assign() {
  // should not include line 1 b/c of overriding assignment
  let src = r#"
fn main() {  
  let mut x = {
    1
  };
  x = 2;
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![2, 5, 6]);
}

#[test]
fn variable_reassign() {
  // should include line 1 b/c of reassign
  let src = r#"
fn main() {
  let mut x = 1;
  x += 2;
  x;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 3, 4]);
}

#[test]
fn variable_read_multiple() {
  let src = r#"
fn main() {
  let x = 1;
  let y = 2;
  x + y;
}
"#;

  run(src, Range::line(4, 3, 8), vec![2, 3, 4]);
}

#[test]
fn variable_slice_from_middle() {
  // should not include x += 3
  let src = r#"
fn main() {
  let mut x = 1;
  x += 2;
  x;
  x += 3;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 3, 4]);
}

#[test]
fn variable_select_lhs() {
  let _src = r#"
fn main() {
  let x = 1;
  let y = x;
}
"#;

  // TODO: need to figure out how to turn selections from LHS of assign
  // into corresponding SliceSet span

  //run(src, Range::line(3, 7, 8), vec![2, 3]);
}

#[test]
fn variable_slice_includes_lhs() {
  let src = r#"
fn main() {
  let x = {
    1
  };
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 5]);
}

#[test]
fn if_both_paths_relevant() {
  let src = r#"
fn main() {
  let x = if true {
    1
  } else {
    2
  };
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![2, 3, 5, 7]);
}

#[test]
fn if_all_paths_irrelevant() {
  let src = r#"
fn main() {
  let x = 1;
  let y = if true { 1 } else { 2 };
  x;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 4]);
}

#[test]
fn if_one_path_relevant() {
  let src = r#"
fn main() {
  let mut x = 1;
  let mut y = 2;
  if true {
    x = 3;
  } else {
    y = x;
  }
  x;
}
"#;

  run(src, Range::line(9, 3, 4), vec![2, 4, 5, 9]);
}

#[test]
fn while_cond_relevant() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = 2;
  while x < y {
    x += 1;
  }
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![2, 3, 4, 5, 7]);
}

#[test]
fn while_cond_irrelevant() {
  let src = r#"
fn main() {
  let mut x = 1;
  let mut y = 2;
  while x < y {
    y -= 1;
  }
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![2, 7]);
}

#[test]
fn loop_break() {
  let _src = r#"
fn main() {
  let mut x = 0;
  loop {
    if x == 10 {
      break;
    }
    x += 1;
  }
  x;
}
"#;

  // TODO
  // need to include control flow keywords like loop/break

  //run(src, Range::line(9, 3, 4), vec![2, 3, 4, 5, 7, 9]);
}

#[test]
fn tuple_write_field_read_whole() {
  // should include line 1 because x.1 is relevant
  let src = r#"
fn main() {
  let mut x = (0, 1);
  x.0 = 1;
  x;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 3, 4]);
}

#[test]
fn tuple_write_field_read_field() {
  // shouldn't include line 1 b/c x.1 isn't relevant
  let src = r#"
fn main() {
  let mut x = {
    (0, 1)
  };
  x.0 = 1;
  x.0;
}
"#;

  run(src, Range::line(6, 3, 6), vec![2, 5, 6]);
}

#[test]
fn tuple_write_whole_read_whole() {
  let src = r#"
fn main() {
  let mut x = {
    (0, 1)
  };
  x = (2, 3);
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![2, 5, 6]);
}

#[test]
fn tuple_write_whole_read_field() {
  let src = r#"
fn main() {
  let mut x = {
    (0, 1)
  };
  x = (2, 3);
  x.0;
}
"#;

  run(src, Range::line(6, 3, 6), vec![2, 5, 6]);
}

#[test]
fn struct_write() {
  let src = r#"
fn main() {
  struct Foo { x: i32, y: i32 }
  let mut x = Foo { x: 1, y: 2 };
  x.y = 3;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![3, 4, 5]);
}

#[test]
fn struct_mut_ptr() {
  let src = r#"
fn main() {
  struct Foo<'a>(&'a mut i32);
  let mut x = 1;
  let f = Foo(&mut x);
  *f.0 += 1;
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![3, 4, 5, 6]);
}

#[test]
fn struct_mut_ptr_function() {
  let src = r#"
struct Foo<'a>(&'a mut i32);
fn foo(f: Foo) {}

fn main() {
  let mut x = 0;
  let f = Foo(&mut x);  
  foo(f);
  x;
}
"#;

  run(src, Range::line(8, 3, 4), vec![5, 6, 7, 8]);
}

#[test]
fn enum_write_branch_read_whole() {
  // Both if-lets should be relevant
  let src = r#"
fn main() {
  enum Foo { X(i32), Y(i32) }
  let mut x = Foo::X(1);
  if let Foo::X(z) = &mut x {
    *z += 1;
  }
  if let Foo::Y(z) = &mut x {
    *z += 1;
  }  
  x;
}
"#;

  run(src, Range::line(10, 3, 4), vec![3, 4, 5, 7, 8, 10]);
}

#[test]
fn enum_write_branch_read_branch() {
  // Foo::Y code should be irrelevant
  let _src = r#"
fn main() {
  enum Foo { X(i32), Y(i32) }
  let mut x = Foo::X(1);
  if let Foo::X(z) = &mut x {
    *z += 1;
  }
  if let Foo::Y(z) = &mut x {
    *z += 1;
  }  
  if let Foo::X(z) = x {
    z;
  }
}
"#;

  /*
   * TODO!
   * Issue is that switch on discriminant(x) adds x to relevant set,
   * and then any mutations to subfields of x are relevant.
   * Not sure what the solution is beyond some kind of fancy flow-sensitivity,
   * or maybe including discriminant(x) as a first-class PlacePrim
   */

  //run(src, Range::line(11, 5, 6), vec![3, 4, 5, 10, 11]);
}

#[test]
fn array_write() {
  let src = r#"
fn main() {
  let mut x = [0; 1];
  x[0] = 2;
  x;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 3, 4]);
}

#[test]
fn array_read() {
  let src = r#"
fn main() {
  let x = [0; 1];
  let y = x[0];
  y;
}
"#;

  run(src, Range::line(4, 3, 4), vec![2, 3, 4]);
}

#[test]
fn slice_write() {
  let src = r#"
fn main() {
  let mut x = [0u8; 2];
  let y = &mut x[..1];
  y[0] = 0;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}
#[test]
fn slice_ptr_elem_write() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = [&mut x];
  *y[0] = 0;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}

#[test]
fn pointer_write() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = &mut x;
  *y = 1;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}


#[test]
fn pointer_read() {
  let src = r#"
fn main() {
  let x = 1;
  let y = &x;
  let z = *y + 1;
  z;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}

#[test]
fn pointer_increment() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = &mut x;
  *y += 1;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}

#[test]
fn pointer_ignore_reads() {
  // n should be ignored
  let src = r#"
fn main() {
  let mut x = 1;
  let y = &mut x;
  let n = *y;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 5]);
}

#[test]
fn pointer_aliasing() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = &mut x;
  let z = y;
  *z = 2;
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![2, 3, 4, 5, 6]);
}

#[test]
fn pointer_multiple_locations() {
  let src = r#"
fn main() {
  let mut x = 1;
  let mut y = 2;
  let z = if true {
    &mut x
  } else {
    &mut y
  };
  *z += 1;
  *z;
}
"#;

  run(src, Range::line(10, 3, 5), vec![2, 3, 4, 5, 7, 9, 10]);
}

#[test]
fn pointer_transitive() {
  let src = r#"
fn main() {
  let mut x = 1;
  let mut y = &mut x;
  let z = &mut y;
  **z = 2;
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![2, 3, 4, 5, 6]);
}

#[test]
fn interprocedural_output() {
  let src = r#"
fn foo() -> i32 { 1 }  

fn main() {
  let mut x = 1;
  x += foo();
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![4, 5, 6]);
}


#[test]
fn interprocedural_input() {
  let src = r#"
fn foo(x: i32) -> i32 { x }  

fn main() {
  let mut x = 1;
  let y = 2;
  x += foo(y);
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 5, 6, 7]);
}

#[test]
fn interprocedural_mut_input() {
  // y should be relevant b/c it could be involved in computation of x
  let src = r#"
fn foo(x: &mut i32, y: i32) {}  

fn main() {
  let mut x = 1;
  let y = 2;
  foo(&mut x, y);
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 5, 6, 7]);
}

#[test]
fn interprocedural_ref_input() {
  // call should be irrelevant b/c x can only be read
  let src = r#"
fn foo(x: &i32) {}  

fn main() {
  let mut x = 1;
  foo(&x);
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![4, 6]);
}

#[test]
fn interprocedural_mut_input_irrelevant() {
  let src = r#"
fn foo(x: &mut i32) {}  

fn main() {
  let mut x = 1;
  let mut y = 2;
  foo(&mut y);
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 7]);
}

#[test]
fn interprocedural_mut_input_field() {
  let src = r#"
fn foo(x: (&mut i32,)) {}  

fn main() {
  let mut x = 1;
  foo((&mut x,));
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![4, 5, 6]);
}

#[test]
fn interprocedural_mut_input_whole() {
  let src = r#"
fn write(t: &mut (i32, i32)) {}

fn main() {
  let mut x = (1, 2);
  write(&mut x);
  x.0;
}
"#;

  run(src, Range::line(6, 3, 6), vec![4, 5, 6]);
}

#[test]
fn interprocedural_mut_output() {
  let src = r#"
fn foo(x: &mut i32) -> &mut i32 { x }  

fn main() {
  let mut x = 1;
  let y = foo(&mut x);
  *y += 2;
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 5, 6, 7]);
}

#[test]
fn interprocedural_mut_output_lifetimes() {
  let src = r#"
fn foo<'a, 'b>(x: &'a mut i32, y: &'b mut i32) -> &'b mut i32 { y }  

fn main() {
  let mut x = 1;
  let mut y = 2;
  let z = foo(&mut x, &mut y);
  *z += 1;
  x;
}
"#;

  run(src, Range::line(8, 3, 4), vec![4, 5, 6, 8]);
}

#[test]
fn interprocedural_mut_output_lifetimes_outlives() {
  let src = r#"
fn foo<'a, 'b: 'a>(x: &'a mut i32, y: &'b mut i32) -> &'a mut i32 { x }  

fn main() {
  let mut x = 1;
  let mut y = 2;
  let z = foo(&mut x, &mut y);
  *z += 1;
  y;
}
"#;

  run(src, Range::line(8, 3, 4), vec![4, 5, 6, 7, 8]);
}

#[test]
fn interprocedural_mut_output_field_read_whole() {
  let src = r#"
fn foo(x: &mut (i32, i32)) -> &mut i32 { &mut x.0 }

fn main() {
  let mut x = (0, 1);
  let y = foo(&mut x);
  *y += 1;
  x;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 5, 6, 7]);
}

#[test]
fn interprocedural_mut_output_field_read_field() {
  // Should conservatively assume returned value could be any field
  let src = r#"
fn foo(x: &mut (i32, i32)) -> &mut i32 { &mut x.0 }

fn main() {
  let mut x = (0, 1);
  let y = foo(&mut x);
  *y += 1;
  x.1;
}
"#;

  run(src, Range::line(7, 3, 6), vec![4, 5, 6, 7]);
}

#[test]
fn interprocedural_ref_output() {
  let src = r#"
fn foo(x: &i32) -> &i32 { x }  

fn main() {
  let mut x = 1;
  let y = foo(&x);
  let z = *y;
  z;
}
"#;

  run(src, Range::line(7, 3, 4), vec![4, 5, 6, 7]);
}

#[test]
fn function_param() {
  let src = r#"
fn foo(x: i32) {
  let y = x + 1;
  y;
}

fn main() {}
"#;

  run(src, Range::line(3, 3, 4), vec![1, 2, 3]);
}

#[test]
fn function_mut_ptr_param() {
  let src = r#"
fn foo(x: &mut i32) {
  *x = 2;
  let y = *x;
  y;
}

fn main() {}
"#;

  run(src, Range::line(4, 3, 4), vec![1, 2, 3, 4]);
}

#[test]
fn function_mut_ptr_param_field() {
  let src = r#"
fn foo(x: (&mut i32,)) {
  *x.0 = 2;
  let y = *x.0;
  y;
}

fn main() {}
"#;

  run(src, Range::line(4, 3, 4), vec![1, 2, 3, 4]);
}

#[test]
fn function_lifetime_outlives_spurious_alias() {
  // given our algorithm of estimating aliases from lifetimes, `w` and `x` are considered
  // to aliases `y` and `z` given the constraint `'b: 'a`
  let src = r#"
fn foo<'a, 'b: 'a>(x: &'a mut i32, y: &'b mut i32) -> &'a mut i32 {
  let z = y;
  let w = x;
  *w = 1;
  z
}

fn main() {}
"#;

  run(src, Range::line(5, 3, 4), vec![1, 2, 3, 4, 5]);
}



#[test]
fn closure_write_upvar() {
  let src = r#"
fn main() {
  let mut x = 1;
  let mut f = || { x += 1; };
  f();
  x;
}
"#;

  // NOTE
  // Seems like when a variable is captured as an upvar than explicitly passed as &mut,
  // the MIR source map links the &mut back to the closure definition source range.
  // Hence this example has the closure defn as part of the slice, but not the others.
  run(src, Range::line(5, 3, 4), vec![2, 3, 4, 5]);
}

#[test]
fn closure_read_upvar() {
  let src = r#"
fn main() {
  let mut x = 1;
  let f = || { x + 1; };
  f();
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![2, 5]);
}

#[test]
fn closure_slice_inside() {
  let src = r#"
fn main() {
  let x = Some(1);
  x.and_then(
    |y| {
      let z = y + 1; 
      Some(z)
  });
}
"#;

  run(src, Range::line(6, 12, 13), vec![4, 5, 6]);
}

#[test]
fn macro_read() {
  let _src = r#"
fn main() {
  let x = vec![1, 2, 3];
  x;
}
"#;

  // TODO: need to figure out macro source-map

  // run(src, Range::line(3, 2, 3), vec![2, 3]);
}

#[test]
fn generic_param() {
  let src = r#"
fn main() {}

fn foo<T>(t: T) {
  let x = t;
  x;
}
"#;

  run(src, Range::line(5, 3, 4), vec![3, 4, 5]);
}

#[test]
fn string_print() {
  let src = r#"
fn main() {
  let x = "a";
  println!("{}", x);
}
"#;

  run(src, Range::line(3, 18, 19), vec![2, 3]);
}

#[test]
fn match_test() {
  let src = r#"
fn main() {
  let x = 1;
  let y = match Some(x) {
    None => 
      1,
    Some(x) => 
      x,
  };
  y;
}
"#;

  run(src, Range::line(9, 3, 4), vec![2, 3, 5, 6, 7, 9]);
}
