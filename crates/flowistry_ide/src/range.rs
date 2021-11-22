use anyhow::{bail, Context, Result};
use rustc_data_structures::sync::{Lrc, MappedReadGuard};
use rustc_macros::Encodable;
use rustc_span::{
  source_map::{monotonic::MonotonicVec, SourceMap},
  BytePos, FileName, RealFileName, SourceFile, Span,
};
use std::{default::Default, path::Path};

#[derive(Encodable, Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct Range {
  pub start: usize,
  pub end: usize,
  pub filename: String,
}

impl Range {
  pub fn substr(&self, s: &str) -> String {
    String::from_utf8(
      s.bytes()
        .skip(self.start)
        .take(self.end - self.start)
        .collect::<Vec<_>>(),
    )
    .unwrap()
  }
}

pub fn ranges_from_spans(
  spans: impl Iterator<Item = Span>,
  source_map: &SourceMap,
) -> Result<Vec<Range>> {
  spans
    .map(|span| Range::from_span(span, source_map))
    .collect()
}

impl Range {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Result<Self> {
    let file = source_map.lookup_source_file(span.lo());
    let filename = match &file.name {
      FileName::Real(RealFileName::LocalPath(filename)) => filename.to_string_lossy().into_owned(),
      filename => bail!("Range::from_span doesn't support {:?}", filename),
    };

    let offset = file.start_pos;
    Ok(Range {
      start: (span.lo() - offset).0 as usize,
      end: (span.hi() - offset).0 as usize,
      filename,
    })
  }

  pub fn source_file<'a>(
    &self,
    files: &'a MappedReadGuard<'_, MonotonicVec<Lrc<SourceFile>>>,
  ) -> Result<&'a SourceFile> {
    let filename = Path::new(&self.filename);
    files
      .iter()
      .find(|file| match &file.name {
        // rustc seems to store relative paths to files in the workspace, so if filename is absolute,
        // we can compare them using Path::ends_with
        FileName::Real(RealFileName::LocalPath(other)) => filename.ends_with(other),
        _ => false,
      })
      .map(|f| &**f)
      .with_context(|| {
        format!(
          "Could not find SourceFile for path: {}. Available SourceFiles were: [{}]",
          self.filename,
          files
            .iter()
            .filter_map(|file| match &file.name {
              FileName::Real(RealFileName::LocalPath(other)) =>
                Some(format!("{}", other.display())),
              _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
        )
      })
  }

  pub fn to_span(&self, source_map: &SourceMap) -> Result<Span> {
    let files = source_map.files();
    let source_file = self.source_file(&files)?;
    let offset = source_file.start_pos;

    Ok(Span::with_root_ctxt(
      offset + BytePos(self.start as u32),
      offset + BytePos(self.end as u32),
    ))
  }
}
