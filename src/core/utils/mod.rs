use log::info;
use rustc_span::{source_map::SourceMap, Pos, Span};
use std::time::Instant;

pub use hir::*;
pub use mir::*;

mod hir;
mod mir;

pub fn elapsed(name: &str, start: Instant) {
  info!("{} took {}s", name, start.elapsed().as_nanos() as f64 / 1e9)
}

pub fn span_to_string(span: Span, source_map: &SourceMap) -> String {
  let lo = source_map.lookup_char_pos(span.lo());
  let hi = source_map.lookup_char_pos(span.hi());
  let snippet = source_map.span_to_snippet(span).unwrap();
  format!(
    "{} ({}:{}-{}:{})",
    snippet,
    lo.line,
    lo.col.to_usize() + 1,
    hi.line,
    hi.col.to_usize() + 1
  )
}
