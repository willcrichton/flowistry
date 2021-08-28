#![feature(rustc_private)]

extern crate rustc_driver;

use std::{env, fmt::Debug, process::exit, str::FromStr};

fn arg<T>(s: &str) -> T
where
  T: FromStr,
  T::Err: Debug,
{
  env::var(format!("FLOWISTRY_{}", s))
    .expect(&format!("Missing argument: {}", s))
    .parse()
    .unwrap()
}

fn main() {
  let args = env::args().collect::<Vec<_>>();

  // TODO: how does prusti or clippy determine be_rustc? --print seems like a hack
  let be_rustc = args.iter().any(|arg| arg.starts_with("--print"));
  if be_rustc {
    rustc_driver::main();
  }

  rustc_driver::init_rustc_env_logger();
  env_logger::init();

  let exit_code = rustc_driver::catch_with_exit_code(|| match arg::<String>("COMMAND").as_str() {
    cmd @ ("backward_slice" | "forward_slice") => {
      let range = flowistry::Range {
        start: arg::<usize>("START"),
        end: arg::<usize>("END"),
        filename: arg::<String>("FILE"),
      };
      let config = flowistry::Config {
        range,
        ..Default::default()
      };
      let slice = if cmd == "backward_slice" {
        flowistry::backward_slice(config, &args).unwrap()
      } else {
        flowistry::forward_slice(config, &args).unwrap()
      };
      println!("{}", serde_json::to_string(&slice).unwrap());
      Ok(())
    }
    "effects" => {
      let effects = flowistry::effects(arg::<String>("QPATH"), &args).unwrap();
      println!("{}", serde_json::to_string(&effects).unwrap());
      Ok(())
    }
    _ => unimplemented!(),
  });

  exit(exit_code);
}
