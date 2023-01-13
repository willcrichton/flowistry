use std::{
  collections::hash_map::Entry, default::Default, ffi::OsStr, path::PathBuf, sync::Mutex,
};

use anyhow::{bail, Context, Result};
use rustc_data_structures::{fx::FxHashMap as HashMap, sync::Lrc};
use rustc_hir::{
  intravisit::{self, Visitor},
  BodyId,
};
use rustc_middle::ty::TyCtxt;
use rustc_span::{source_map::SourceMap, FileName, RealFileName, SourceFile, Span};
use serde::Serialize;

use crate::{
  cached::Cache,
  indexed::{
    impls::{Filename, FilenameDomain, FilenameIndex},
    IndexedDomain,
  },
};

struct CharByteMapping {
  #[allow(unused)]
  byte_to_char: HashMap<BytePos, CharPos>,
  char_to_byte: HashMap<CharPos, BytePos>,
}

impl CharByteMapping {
  pub fn build(s: &str) -> Self {
    let mut byte_to_char = HashMap::default();
    let mut char_to_byte = HashMap::default();

    for (char_idx, (byte_idx, _)) in s.char_indices().enumerate() {
      byte_to_char.insert(BytePos(byte_idx), CharPos(char_idx));
      char_to_byte.insert(CharPos(char_idx), BytePos(byte_idx));
    }

    CharByteMapping {
      byte_to_char,
      char_to_byte,
    }
  }
}

#[derive(Default)]
pub struct RangeContext {
  filenames: FilenameDomain,
  path_mapping: HashMap<FilenameIndex, Lrc<SourceFile>>,
  char_byte_mapping: Cache<FilenameIndex, CharByteMapping>,
}

#[derive(Default)]
struct RangeContextCell(Mutex<RangeContext>);

lazy_static::lazy_static! {
  static ref CONTEXT: RangeContextCell = RangeContextCell::default();
}

// SAFETY: we only ever used RangeContext in two threads: the
// thread calling Rustc, and the Rustc driver thread. The
// former needs access to the Filename interner.
unsafe impl Sync for RangeContextCell {}

impl Filename {
  pub fn intern<T: ?Sized + AsRef<OsStr>>(t: &T) -> FilenameIndex {
    let filename = Filename(PathBuf::from(t));
    let mut ctx = CONTEXT.0.lock().unwrap();
    ctx.filenames.ensure(&filename)
  }
}

impl FilenameIndex {
  pub fn find_source_file(self, source_map: &SourceMap) -> Result<Lrc<SourceFile>> {
    let ctx = &mut *CONTEXT.0.lock().unwrap();
    match ctx.path_mapping.entry(self) {
      Entry::Occupied(entry) => Ok(Lrc::clone(entry.get())),
      Entry::Vacant(entry) => {
        let files = source_map.files();
        debug_assert!(
          ctx.filenames.as_vec().get(self).is_some(),
          "Missing file index!"
        );
        let filename = &ctx.filenames.value(self);
        let filename = filename
          .canonicalize()
          .unwrap_or_else(|_| filename.to_path_buf());
        let rustc_filename = files
          .iter()
          .map(|file| &file.name)
          .find(|name| match &name {
            // rustc seems to store relative paths to files in the workspace, so if filename is absolute,
            // we can compare them using Path::ends_with
            FileName::Real(RealFileName::LocalPath(other)) => {
              let canonical = other.canonicalize();
              let other = canonical.as_ref().unwrap_or(other);
              filename.ends_with(other)
            }
            _ => false,
          })
          .with_context(|| {
            format!(
              "Could not find SourceFile for path: {}. Available SourceFiles were: [{}]",
              filename.display(),
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
          })?;
        let file = source_map.get_source_file(rustc_filename).unwrap();
        entry.insert(Lrc::clone(&file));
        Ok(file)
      }
    }
  }
}

#[derive(Serialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct BytePos(pub usize);

#[derive(Serialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct CharPos(pub usize);

/// Data structure for sharing spans outside rustc.
///
/// Rustc uses byte indexes to describe ranges of source code, whereas
/// most Javascript-based editors I've encountered (e.g. VSCode) use
/// character-based (really grapheme-based) indexes. This data structure
/// helps convert between the two representations.
#[derive(Serialize, Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Range<T> {
  pub start: T,
  pub end: T,
  pub filename: FilenameIndex,
}

pub type ByteRange = Range<BytePos>;
pub type CharRange = Range<CharPos>;

impl ByteRange {
  pub fn as_char_range(&self, source_map: &SourceMap) -> CharRange {
    let file = self.filename.find_source_file(source_map).unwrap();
    let get_char_pos = |rel_byte: BytePos| {
      let bpos = file.start_pos + rustc_span::BytePos(rel_byte.0 as u32);
      let cpos = file.bytepos_to_file_charpos(bpos);
      CharPos(cpos.0)
    };

    let char_start = get_char_pos(self.start);
    let char_end = get_char_pos(self.end);

    CharRange {
      start: char_start,
      end: char_end,
      filename: self.filename,
    }
  }

  pub fn from_char_range(
    char_start: CharPos,
    char_end: CharPos,
    filename: FilenameIndex,
    source_map: &SourceMap,
  ) -> Result<ByteRange> {
    let file = filename.find_source_file(source_map)?;

    let ctx = CONTEXT.0.lock().unwrap();
    let mapping = ctx.char_byte_mapping.get(filename, |_| {
      CharByteMapping::build(file.src.as_ref().unwrap().as_str())
    });
    let byte_start = mapping.char_to_byte[&char_start];
    let byte_end = mapping.char_to_byte[&char_end];
    Ok(ByteRange {
      start: byte_start,
      end: byte_end,
      filename,
    })
  }

  pub fn from_span(span: Span, source_map: &SourceMap) -> Result<Self> {
    let mut ctx = CONTEXT.0.lock().unwrap();

    log::trace!("Converting to range: {span:?}");
    let file = source_map.lookup_source_file(span.lo());
    let filename = match &file.name {
      FileName::Real(RealFileName::LocalPath(filename)) => {
        ctx.filenames.ensure(&Filename(filename.clone()))
      }
      filename => bail!("Range::from_span doesn't support {filename:?}"),
    };

    assert!(
      source_map.ensure_source_file_source_present(file.clone()),
      "Could not load source for file: {:?}",
      file.name
    );
    let external = file.external_src.borrow();
    let _src = file
      .src
      .as_ref()
      .unwrap_or_else(|| external.get_source().as_ref().unwrap());

    let byte_start = BytePos(source_map.lookup_byte_offset(span.lo()).pos.0 as usize);
    let byte_end = BytePos(source_map.lookup_byte_offset(span.hi()).pos.0 as usize);

    Ok(ByteRange {
      start: byte_start,
      end: byte_end,
      filename,
    })
  }

  pub fn substr(&self, s: &str) -> String {
    s[self.start.0 .. self.end.0].to_string()
  }
}

impl CharRange {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Result<Self> {
    let byte_range = ByteRange::from_span(span, source_map)?;
    Ok(byte_range.as_char_range(source_map))
  }
}

/// Used to convert objects into a [`Span`] with access to [`TyCtxt`]
pub trait ToSpan: Send + Sync {
  fn to_span(&self, tcx: TyCtxt) -> Result<Span>;
}

impl ToSpan for ByteRange {
  fn to_span(&self, tcx: TyCtxt) -> Result<Span> {
    let source_map = tcx.sess.source_map();
    let source_file = self.filename.find_source_file(source_map)?;
    let offset = source_file.start_pos;

    Ok(Span::with_root_ctxt(
      offset + rustc_span::BytePos(self.start.0 as u32),
      offset + rustc_span::BytePos(self.end.0 as u32),
    ))
  }
}

impl ToSpan for CharRange {
  fn to_span(&self, tcx: TyCtxt) -> Result<Span> {
    let range = ByteRange::from_char_range(
      self.start,
      self.end,
      self.filename,
      tcx.sess.source_map(),
    )?;
    range.to_span(tcx)
  }
}

fn qpath_to_span(tcx: TyCtxt, qpath: String) -> Result<Span> {
  struct Finder<'tcx> {
    tcx: TyCtxt<'tcx>,
    qpath: String,
    span: Option<Span>,
  }

  impl<'tcx> Visitor<'tcx> for Finder<'tcx> {
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

  let mut finder = Finder {
    tcx,
    qpath,
    span: None,
  };
  tcx.hir().visit_all_item_likes_in_crate(&mut finder);
  finder
    .span
    .with_context(|| format!("No function with qpath {}", finder.qpath))
}

/// An externally-provided identifier of a function
pub enum FunctionIdentifier {
  /// Name of a function
  Qpath(String),

  /// Range of code possibly inside a function
  Range(CharRange),
}

impl ToSpan for FunctionIdentifier {
  fn to_span(&self, tcx: TyCtxt) -> Result<Span> {
    match self {
      FunctionIdentifier::Qpath(qpath) => qpath_to_span(tcx, qpath.clone()),
      FunctionIdentifier::Range(range) => range.to_span(tcx),
    }
  }
}
