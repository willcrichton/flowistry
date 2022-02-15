use std::time::Instant;

use log::info;

pub fn elapsed(name: &str, start: Instant) {
  info!("{name} took {:.04}s", start.elapsed().as_secs_f64());
}

pub struct BlockTimer<'a> {
  pub name: &'a str,
  pub start: Instant,
}

impl Drop for BlockTimer<'_> {
  fn drop(&mut self) {
    elapsed(self.name, self.start);
  }
}

#[macro_export]
macro_rules! block_timer {
  ($name:expr) => {
    let name = $name;
    let start = std::time::Instant::now();
    let _timer = $crate::timer::BlockTimer { name, start };
    log::info!("Starting {name}...");
  };
}
