use once_cell::sync::OnceCell;
use serde::Serialize;
use rustc_span::{source_map::SourceMap, Span, BytePos};

#[derive(Serialize, Debug)]
pub struct Range {
  pub start_line: usize,
  pub start_col: usize,
  pub end_line: usize,
  pub end_col: usize,
}

impl Range {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Self {
    let lines = source_map.span_to_lines(span).unwrap();
    let start_line = lines.lines.first().unwrap();
    let end_line = lines.lines.last().unwrap();
    Range {
      start_line: start_line.line_index,
      start_col: start_line.start_col.0,
      end_line: end_line.line_index,
      end_col: end_line.end_col.0,
    }
  }

  pub fn to_span(&self, source_map: &SourceMap) -> Span {
    let source_file = source_map.lookup_source_file(BytePos(0));
    let start_pos = source_file.line_bounds(self.start_line).start + BytePos(self.start_col as u32);
    let end_pos = source_file.line_bounds(self.end_line).start + BytePos(self.end_col as u32);
    Span::with_root_ctxt(start_pos, end_pos)
  }
}


#[derive(Debug)]
pub struct Config {
  pub range: Range,
  pub debug: bool
}

pub static CONFIG: OnceCell<Config> = OnceCell::new();
  