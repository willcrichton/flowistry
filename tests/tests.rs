use rust_slicer::Range;
use utils::run;

mod utils;

#[test]
fn variable_read() {
  let src = r#"
fn main() {
  let mut x = 1;
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
  let mut x = 1;
  x = 2;
  x;
}
"#;

  run(src, Range::line(4, 3, 4), vec![3, 4]);
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
  let _src = r#"
fn main() {
  let x = 1;
  let y = if true { 1 } else { 2 };
  x;
}
"#;

  /* TODO!!
   * path_relevant doesn't seem to be working correctly here
   */

  //run(src, Range::line(4, 3, 4), vec![2, 4]);
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
  let _src = r#"
fn main() {
  let mut x = 1;
  let mut y = 2;
  while x < y {
    y -= 1;
  }
  x;
}
"#;

  /* TODO! 
   * probably same issue as if_all_paths_irrelevant
   */

  //run(src, Range::line(7, 3, 4), vec![2, 7]);
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
  let mut x = (0, 1);
  x.0 = 1;
  x.0;
}
"#;

  run(src, Range::line(4, 3, 6), vec![3, 4]);
}

#[test]
fn tuple_write_whole_read_whole() {
  let _src = r#"
fn main() {
  let mut x = (0, 1);
  x = (2, 3);
  x;
}
"#;

  /*
   * TODO!!!
   * This test currently fails b/c x = (2, 3) gets expanded to x.0 = 2, x.1 = 3
   * in the MIR. Slicer doesn't accumulate field-level assignments to eventually
   * kill the whole structure when each field has been set. (Possible feature?)
   */

  // run(src, Range::line(4, 3, 4), vec![3, 4]);
}


#[test]
fn tuple_write_whole_read_field() {
  let src = r#"
fn main() {
  let mut x = (0, 1);
  x = (2, 3);
  x.0;
}
"#;

  run(src, Range::line(4, 3, 6), vec![3, 4]);
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
   * TODO!!
   * apparently haven't implemented field-discrimination based on variant?
   * should be done in placeprim... double check this
   */
  
  //run(src, Range::line(11, 5, 6), vec![3, 4, 5, 10, 11]);
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

  run(src, Range::line(5, 3, 4), vec![4, 5]);
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

  run(src, Range::line(5, 3, 4), vec![2, 4, 5]);
}

#[test]
fn pointer_ignore_reads() {
  // y and n should be ignored
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

  run(src, Range::line(6, 3, 4), vec![5, 6]);
}

#[test]
fn function_output() {
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
fn function_input() {
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
fn function_mut_input() {
  let src = r#"
fn foo(x: &mut i32) {}  

fn main() {
  let mut x = 1;
  foo(&mut x);
  x;
}
"#;

  run(src, Range::line(6, 3, 4), vec![4, 5, 6]);
}


#[test]
fn function_ref_input() {
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
fn function_mut_output() {
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
fn function_mut_output_lifetimes() {
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



