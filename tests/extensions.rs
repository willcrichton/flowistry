use flowistry::config::{EvalMode, PointerMode, EVAL_MODE};
use fluid_let::fluid_set;
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
fn variable_read() {
  let src = r#"
fn main() {
  let mut x = 1;
  `[let `[mut y]` = `[2]`;]`
  let a = &mut x;
  let b = &mut y;
  *a += 1;
  `[`(y)`;]`
}
"#;

  mode! {
    pointer_mode: PointerMode::Conservative
  };
  backward_slice(src);
}
