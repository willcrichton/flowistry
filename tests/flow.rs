mod utils;

#[test]
fn foobar() {
  let src = r#"
fn main() {
  let mut x = 1;
  let y = 2;
  x += y;  
}
"#;

  utils::flow(src, "main");
}
