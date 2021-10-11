use flowistry::extensions::{ContextMode, EvalMode, MutabilityMode, PointerMode, EVAL_MODE};
use fluid_let::fluid_set;
use test_env_log::test;
use utils::backward_slice;

mod utils;

macro_rules! mode {
  ($key:ident : $val:expr) => {
    fluid_set!(
      EVAL_MODE,
      &EvalMode {
        $key: $val,
        ..Default::default()
      }
    );
  };
}

#[test]
fn conservative_i32_mut_ptr() {
  let src = r#"
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[2]`;]`
  `[let `[a]` = `[&mut x]`;]`
  let b = &mut y;
  `[`[*a += 1]`;]`
  `[`(y)`;]`
}
"#;

  mode! { pointer_mode: PointerMode::Conservative };
  backward_slice(src);
}

#[test]
fn ignoremut_simple() {
  let src = r#"
fn other(x: &i32) {}
fn main() {
  `[let `[x]` = `[1]`;]`
  `[`[other(`[&x]`)]`;]`
  `[`(x)`;]`
}
"#;

  mode! { mutability_mode: MutabilityMode::IgnoreMut };
  backward_slice(src);
}

#[test]
fn recurse_simple() {
  let src = r#"
fn other(x: &mut i32) -> i32 { *x }
fn main() {
  `[let `[mut x]` = `[1]`;]`
  let y = other(&mut x);
  `[`(x)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_not_all_args() {
  let src = r#"
fn other(x: &mut i32, y: i32, z: i32) { *x += y; }
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[y]` = `[1]`;]`
  let z = 1;
  `[`[other(`[&mut x]`, `[y]`, z)]`;]`
  `[`(x)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_project_src() {
  // TODO: y.1 += 1 shouldn't be part of the slice
  //  see tuple_copy test
  let src = r#"
fn other(x: &mut i32, y: (i32, i32)) { *x += y.0; }
fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[let `[mut y]` = `[(0, 0)]`;]`
  `[`[y.0 += 1]`;]`
  `[`[y.1 += 1]`;]`
  `[`[other(`[&mut x]`, `[y]`)]`;]`
  `[`(x)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_project_dst() {
  let src = r#"
fn other(x: &mut (i32, i32)) { (*x).0 = 1; }
fn main() {
  `[let `[mut x]` = `[(0, 0)]`;]`
  other(&mut x);
  `[`(x.1)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_no_definition() {
  let src = r#"
fn main() {
  `[let `[mut v]` = `[vec![0]]`;]`
  `[`[v.get_mut(0)]`;]`
  `[`(v)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_recursive() {
  let src = r#"
fn foobar(x: &mut i32) -> i32 {
  foobar(x) - 1
}

fn main() {
  `[let `[mut x]` = `[1]`;]`
  `[`[foobar(`[&mut x]`)]`;]`
  `[`(x)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_cache() {
  // TODO: ideally we could actually verify that the flow for ok is only computed once?
  let src = r#"
fn ok(x: &i32) {}

fn main() {
  `[let `[x]` = `[1]`;]`
  ok(&x);
  ok(&x);
  `[`(x)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_return() {
  let src = r#"
fn ok(x: i32, y: i32) -> i32 { x }

fn main() {
  `[let `[x]` = `[1]`;]`
  let y = 1;
  `[let `[z]` = `[ok(`[x]`, y)]`;]`
  `[`(z)`;]`  
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_child_privacy() {
  let src = r#"
mod foo {
  pub struct Foo(i32);
  pub fn new() -> Foo { Foo(0) }
  pub fn ok(f: &mut Foo) { f.0 = 1; }
}  

fn main() {
  `[let `[mut f]` = `[foo::new()]`;]`
  `[`[foo::ok(`[&mut f]`)]`;]`
  `[`(f)`;]`
}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}

#[test]
fn recurse_parent_privacy() {
  let src = r#"
mod bar {
  pub fn whee(f: &mut super::foo::Foo) {
    super::foo::ok(f);
  }
} 

mod foo {
  pub struct Foo(i32);
  pub fn new() -> Foo { Foo(0) }
  pub fn ok(f: &mut Foo) { f.0 = 1; }
  pub fn test() {
    `[let `[mut f]` = `[Foo(0)]`;]`
    `[`[super::bar::whee(`[&mut f]`)]`;]`
    `[`(f)`;]`
  }
}  

fn main() {}
"#;

  mode! { context_mode: ContextMode::Recurse };
  backward_slice(src);
}
