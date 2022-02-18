use flowistry::range::Range;
use rustc_ast::{visit::Visitor, Item, ItemKind};
use rustc_macros::Encodable;
use rustc_span::Span;

use crate::FlowistryResult;

#[derive(Encodable)]
pub struct SpansOutput {
  spans: Vec<Range>,
}

#[derive(Default)]
struct FindBodies {
  spans: Vec<Span>,
}

impl Visitor<'_> for FindBodies {
  fn visit_item(&mut self, i: &Item) {
    if matches!(i.kind, ItemKind::Fn(..)) {
      self.spans.push(i.span);
    }
  }
}

struct Callbacks {
  filename: String,
  output: Option<SpansOutput>,
}

// TODO: figure out if it's possible to get fn spans w/o typechecking

impl rustc_driver::Callbacks for Callbacks {
  fn after_parsing<'tcx>(
    &mut self,
    compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    let (krate, ..) = queries.expansion().unwrap().take();
    let mut finder = FindBodies::default();
    finder.visit_crate(&krate);

    let source_map = compiler.session().source_map();
    let files = source_map.files();
    let source_file = Range {
      start: 0,
      end: 0,
      filename: self.filename.clone(),
    }
    .source_file(&files)
    .unwrap();
    let spans = finder
      .spans
      .into_iter()
      .filter(|span| {
        println!("{:?}", source_map.span_to_snippet(*span).unwrap());
        source_map.lookup_source_file(span.lo()).name_hash == source_file.name_hash
      })
      .filter_map(|span| Range::from_span(span, source_map).ok())
      .collect::<Vec<_>>();
    self.output = Some(SpansOutput { spans });

    rustc_driver::Compilation::Stop
  }
}

pub fn spans(args: &[String], filename: String) -> FlowistryResult<SpansOutput> {
  let mut callbacks = Callbacks {
    filename,
    output: None,
  };
  crate::run_with_callbacks(args, &mut callbacks)?;
  Ok(callbacks.output.unwrap())
}
