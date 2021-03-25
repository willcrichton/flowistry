use rust_slicer::Range;
use utils::run;

mod utils;

#[test]
fn loop_counter() {
  let src = r#"
use std::io;

fn main() {
  let mut buffer = String::new();
  let stdin = io::stdin();

  let mut total = 0;
  loop {
    let input = {
      buffer.clear();
      stdin.read_line(&mut buffer).unwrap();
      buffer.trim()
    };

    if input == "exit" {
      break;
    }

    let n = input.parse::<i32>().unwrap();
    println!("Read: {}", n);
    total += n;
  }

  println!("{:?}", total);
}
"#;

  run(
    src,
    Range::line(24, 20, 25),
    vec![4, 5, 7, 9, 10, 11, 12, 15, 19, 21, 24],
  );
}
