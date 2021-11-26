use utils::{backward_slice, forward_slice};

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

#[test]
fn time_calculation() {
  let src = r#"
use std::time::Instant;
fn run_expensive_calculation(){}
fn main() {
  `[let `(start)` = `[Instant::now()]`;]`
  run_expensive_calculation();
  `[let `[elapsed]` = `[start.elapsed()]`;]`
  `[println!("Elapsed: {}s", `[elapsed.as_secs()]`);]`
}
"#;

  forward_slice(src);
}

#[test]
fn hashset_union() {
  let src = r#"
use std::collections::HashSet;
fn union(`[set]`: &mut HashSet<i32>, `[other]`: &HashSet<i32>) -> bool {
  let orig_len = set.len();
  `[for `[el]` in `[other]` {
    `[`[set.insert(`[*el]`)]`;]`
  }]`
  `[let `(final_len)` = `[set.len()]`;]`
  return orig_len != final_len;
}
"#;

  backward_slice(src);
}
