//! Data structure for sharing [spans][Span] outside rustc.

use std::{default::Default, path::Path};

use anyhow::{bail, Context, Result};
use rustc_data_structures::sync::{Lrc, MappedReadGuard};
use rustc_hir::{
  intravisit::{self, Visitor},
  itemlikevisit::ItemLikeVisitor,
  BodyId,
};
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;
use rustc_span::{
  source_map::{monotonic::MonotonicVec, SourceMap},
  BytePos, FileName, RealFileName, SourceFile, Span,
};
use unicode_segmentation::UnicodeSegmentation;

pub fn qpath_to_span(tcx: TyCtxt, qpath: String) -> Result<Span> {
  struct Finder<'tcx> {
    tcx: TyCtxt<'tcx>,
    qpath: String,
    span: Option<Span>,
  }

  impl Visitor<'tcx> for Finder<'tcx> {
    fn visit_nested_body(&mut self, id: BodyId) {
      intravisit::walk_body(self, self.tcx.hir().body(id));

      let local_def_id = self.tcx.hir().body_owner_def_id(id);
      let function_path = self
        .tcx
        .def_path(local_def_id.to_def_id())
        .to_string_no_crate_verbose();
      if function_path[2 ..] == self.qpath {
        self.span = Some(self.tcx.hir().span(id.hir_id));
      }
    }
  }

  impl ItemLikeVisitor<'hir> for Finder<'tcx>
  where
    'hir: 'tcx,
  {
    fn visit_item(&mut self, item: &'hir rustc_hir::Item<'hir>) {
      <Self as Visitor<'tcx>>::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, impl_item: &'hir rustc_hir::ImplItem<'hir>) {
      <Self as Visitor<'tcx>>::visit_impl_item(self, impl_item);
    }

    fn visit_trait_item(&mut self, _trait_item: &'hir rustc_hir::TraitItem<'hir>) {}
    fn visit_foreign_item(&mut self, _foreign_item: &'hir rustc_hir::ForeignItem<'hir>) {}
  }

  let mut finder = Finder {
    tcx,
    qpath,
    span: None,
  };
  tcx.hir().visit_all_item_likes(&mut finder);
  finder
    .span
    .with_context(|| format!("No function with qpath {}", finder.qpath))
}

#[derive(Encodable, Debug, Clone, Hash, PartialEq, Eq, Default)]
pub struct Range {
  pub char_start: usize,
  pub char_end: usize,
  pub byte_start: usize,
  pub byte_end: usize,
  pub filename: String,
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
  pub fn substr(&self, s: &str) -> String {
    s[self.byte_start .. self.byte_end].to_string()
  }

  pub fn from_char_range(
    char_start: usize,
    char_end: usize,
    filename: String,
  ) -> Result<Self> {
    let src = String::from_utf8(std::fs::read(&filename)?)?;
    let mut iter = src.graphemes(true);
    let byte_start = (&mut iter).take(char_start).map(|s| s.len()).sum::<usize>();
    let byte_end = byte_start
      + iter
        .take(char_end - char_start)
        .map(|s| s.len())
        .sum::<usize>();
    Ok(Range {
      char_start,
      char_end,
      byte_start,
      byte_end,
      filename,
    })
  }

  pub fn from_byte_range(
    byte_start: usize,
    byte_end: usize,
    src: &str,
    filename: String,
  ) -> Self {
    let char_start = src[.. byte_start].graphemes(true).count();
    let char_end = char_start + src[byte_start .. byte_end].graphemes(true).count();
    Range {
      byte_start,
      byte_end,
      char_start,
      char_end,
      filename,
    }
  }

  pub fn from_span(span: Span, source_map: &SourceMap) -> Result<Self> {
    let file = source_map.lookup_source_file(span.lo());
    let filename = match &file.name {
      FileName::Real(RealFileName::LocalPath(filename)) => {
        filename.to_string_lossy().into_owned()
      }
      filename => bail!("Range::from_span doesn't support {filename:?}"),
    };

    source_map.ensure_source_file_source_present(file.clone());
    let src = file.src.as_ref().unwrap();

    let byte_start = source_map.lookup_byte_offset(span.lo()).pos.0 as usize;
    let byte_end = source_map.lookup_byte_offset(span.hi()).pos.0 as usize;

    Ok(Self::from_byte_range(byte_start, byte_end, src, filename))
  }

  pub fn source_file<'a>(
    &self,
    files: &'a MappedReadGuard<'_, MonotonicVec<Lrc<SourceFile>>>,
  ) -> Result<&'a SourceFile> {
    let filename = Path::new(&self.filename).canonicalize()?;
    files
      .iter()
      .find(|file| match &file.name {
        // rustc seems to store relative paths to files in the workspace, so if filename is absolute,
        // we can compare them using Path::ends_with
        FileName::Real(RealFileName::LocalPath(other)) => {
          filename.ends_with(other.canonicalize().unwrap())
        }
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
}

pub trait ToSpan: Send + Sync {
  fn to_span(&self, tcx: TyCtxt<'tcx>) -> Result<Span>;
}

impl ToSpan for Range {
  fn to_span(&self, tcx: TyCtxt<'tcx>) -> Result<Span> {
    let files = tcx.sess.source_map().files();
    let source_file = self.source_file(&files)?;
    let offset = source_file.start_pos;

    Ok(Span::with_root_ctxt(
      offset + BytePos(self.byte_start as u32),
      offset + BytePos(self.byte_end as u32),
    ))
  }
}

pub enum FunctionIdentifier {
  Qpath(String),
  Range(Range),
}

impl ToSpan for FunctionIdentifier {
  fn to_span(&self, tcx: TyCtxt<'tcx>) -> Result<Span> {
    match self {
      FunctionIdentifier::Qpath(qpath) => qpath_to_span(tcx, qpath.clone()),
      FunctionIdentifier::Range(range) => range.to_span(tcx),
    }
  }
}
