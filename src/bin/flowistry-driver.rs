#![feature(rustc_private)]

extern crate rustc_driver;

use std::env;
use std::process::exit;
use std::fmt::Debug;
use std::str::FromStr;

fn arg<T>(s: &str) -> T where T: FromStr, T::Err: Debug {
  env::var(format!("FLOWISTRY_{}", s)).unwrap().parse().unwrap()
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
  
  let exit_code = rustc_driver::catch_with_exit_code(|| {
    match arg::<String>("COMMAND").as_str() {
      "backward_slice" => {
        let range = flowistry::Range {
          start_line: arg::<usize>("START_LINE"),                  
          start_col: arg::<usize>("START_COL"),
          end_line: arg::<usize>("END_LINE"),
          end_col: arg::<usize>("END_COL"),
          filename: arg::<String>("FILE"),
        };
        let config = flowistry::Config {
          range,
          ..Default::default()
        };
        let slice = flowistry::slice(config, &args).unwrap();
        println!("{}", serde_json::to_string(&slice).unwrap());
        Ok(())
      }
      _ => unimplemented!()
    }
    
  });

  exit(exit_code);
}
