use std::time::Instant;
use log::info;

pub use mir::*;
pub use hir::*;

mod mir;
mod hir;

pub fn elapsed(name: &str, start: Instant) {
  info!("{} took {}s", name, start.elapsed().as_nanos() as f64 / 1e9)
}

