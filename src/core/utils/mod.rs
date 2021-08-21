use log::info;
use std::time::Instant;

pub use hir::*;
pub use mir::*;

mod hir;
mod mir;

pub fn elapsed(name: &str, start: Instant) {
  info!("{} took {}s", name, start.elapsed().as_nanos() as f64 / 1e9)
}
