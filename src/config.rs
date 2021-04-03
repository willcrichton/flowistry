use fluid_let::fluid_let;
use rustc_span::{
  source_map::{SourceFile, SourceMap},
  BytePos, FileName, Span,
};
use serde::Serialize;
use std::default::Default;

#[derive(Serialize, Debug, Clone)]
pub struct Range {
  pub start_line: usize,
  pub start_col: usize,
  pub end_line: usize,
  pub end_col: usize,
  pub filename: String,
}

impl Range {
  pub fn line(line: usize, start: usize, end: usize) -> Range {
    Range {
      start_line: line,
      start_col: start,
      end_line: line,
      end_col: end,
      filename: "".to_owned(),
    }
  }

  pub fn substr(&self, s: &str) -> String {
    let lines = s.split("\n").collect::<Vec<_>>();
    if self.start_line != self.end_line {
      unimplemented!()
    } else {
      lines[self.start_line][self.start_col..self.end_col].to_owned()
    }
  }
}

impl Range {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Self {
    let filename = source_map.span_to_filename(span);
    let filename = if let FileName::Real(filename) = filename {
      filename.local_path().to_string_lossy().into_owned()
    } else {
      unimplemented!("Range::from_span doesn't support {:?}", filename)
    };

    let lines = source_map.span_to_lines(span).unwrap();
    if lines.lines.len() == 0 {
      return Range {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 0,
        filename,
      };
    }

    let start_line = lines.lines.first().unwrap();
    let end_line = lines.lines.last().unwrap();
    
    Range {
      start_line: start_line.line_index,
      start_col: start_line.start_col.0,
      end_line: end_line.line_index,
      end_col: end_line.end_col.0,
      filename,
    }
  }

  pub fn to_span(&self, source_file: &SourceFile) -> Span {
    let start_pos = source_file.line_bounds(self.start_line).start + BytePos(self.start_col as u32);
    let end_pos = source_file.line_bounds(self.end_line).start + BytePos(self.end_col as u32);
    Span::with_root_ctxt(start_pos, end_pos)
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub enum BorrowMode {
  DistinguishMut,
  IgnoreMut
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub enum ContextMode {
  SigOnly,
  Recurse,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
pub struct EvalMode {
  pub borrow_mode: BorrowMode,
  pub context_mode: ContextMode,
}

#[derive(Debug, Clone)]
pub struct Config {
  pub range: Range,
  pub debug: bool,
  pub eval_mode: EvalMode,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      range: Range::line(0, 0, 0),
      debug: false,
      eval_mode: EvalMode {
        borrow_mode: BorrowMode::DistinguishMut,
        context_mode: ContextMode::SigOnly
      }
    }
  }
}

fluid_let!(pub static CONFIG: Config);
