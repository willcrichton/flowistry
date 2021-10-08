use utils::backward_slice;

mod utils;

#[test]
fn loop_counter() {
  // TODO: why isn't `loop` part of the slice?

  let src = r#"
use std::io;

fn main() {
  `[let `[mut buffer]` = `[String::new()]`;]`
  `[let `[stdin]` = `[io::stdin()]`;]`

  `[let `[mut total]` = `[0]`;]`
  loop {
    `[let `[input]` = {
      `[`[buffer.clear()]`;]`
      `[`[stdin.read_line(`[&mut buffer]`)]`.unwrap();]`
      `[buffer.trim()]`
    };]`

    `[if `[`[input]` == `["exit"]`]` {
      break;
    }]`

    `[let `[n]` = `[`[input.parse::<i32>()]`.unwrap()]`;]`
    println!("Read: {}", n);
    `[`[total += `[n]`]`;]`
  }

  `[println!("{:?}", `(total)`);]`
}
"#;

  backward_slice(src);
}
