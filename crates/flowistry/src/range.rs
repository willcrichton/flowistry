//! Data structure for sharing [spans][Span] outside rustc.

use std::{cell::RefCell, default::Default, fs, path::Path};

use anyhow::{bail, Context, Result};
use rustc_data_structures::{
  fx::FxHashMap as HashMap,
  sync::{Lrc, MappedReadGuard},
};
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

pub struct GraphemeIndices {
  indices: Vec<usize>,
}

impl GraphemeIndices {
  pub fn new(s: &str) -> Self {
    let mut indices = Vec::new();
    let mut idx = 0;
    for g in s.graphemes(true) {
      indices.push(idx);
      idx += g.as_bytes().len();
    }
    GraphemeIndices { indices }
  }

  pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
    let bytes = fs::read(path)?;
    let s = std::str::from_utf8(&bytes)?;
    Ok(Self::new(s))
  }

  pub fn byte_to_char(&self, byte: usize) -> usize {
    self
      .indices
      .iter()
      .enumerate()
      .find(|(_, b)| byte == **b)
      .unwrap()
      .0
  }

  pub fn char_to_byte(&self, char: usize) -> usize {
    self.indices[char]
  }
}

thread_local! {
  static GRAPHEME_INDICES: RefCell<HashMap<String, GraphemeIndices>> = RefCell::new(HashMap::default());
}

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
    filename: &str,
    file: &GraphemeIndices,
  ) -> Self {
    let byte_start = file.char_to_byte(char_start);
    let byte_end = file.char_to_byte(char_end);
    Range {
      char_start,
      char_end,
      byte_start,
      byte_end,
      filename: filename.to_string(),
    }
  }

  pub fn from_byte_range(
    byte_start: usize,
    byte_end: usize,
    filename: &str,
    file: &GraphemeIndices,
  ) -> Self {
    let char_start = file.byte_to_char(byte_start);
    let char_end = file.byte_to_char(byte_end);
    Range {
      byte_start,
      byte_end,
      char_start,
      char_end,
      filename: filename.to_string(),
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

    GRAPHEME_INDICES.with(|grapheme_indices| {
      let mut grapheme_indices = grapheme_indices.borrow_mut();
      let indices = grapheme_indices
        .entry(filename.clone())
        .or_insert_with(|| GraphemeIndices::new(src));
      Ok(Self::from_byte_range(
        byte_start, byte_end, &filename, indices,
      ))
    })
  }

  pub fn source_file<'a>(
    &self,
    files: &'a MappedReadGuard<'_, MonotonicVec<Lrc<SourceFile>>>,
  ) -> Result<&'a SourceFile> {
    let filename = Path::new(&self.filename);
    let filename = filename
      .canonicalize()
      .unwrap_or_else(|_| filename.to_path_buf());

    files
      .iter()
      .find(|file| match &file.name {
        // rustc seems to store relative paths to files in the workspace, so if filename is absolute,
        // we can compare them using Path::ends_with
        FileName::Real(RealFileName::LocalPath(other)) => {
          let canonical = other.canonicalize();
          let other = canonical.as_ref().unwrap_or(other);
          filename.ends_with(other)
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
