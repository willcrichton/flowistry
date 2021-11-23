use test_env_log::test;

mod utils;

#[test]
fn foo() {
  let src = r#"
fn foo(x: &mut i32) -> i32 {
  *x += 1;
  *x + 1  
}

fn main() {}
"#;

  utils::effects(src, "foo");
}
