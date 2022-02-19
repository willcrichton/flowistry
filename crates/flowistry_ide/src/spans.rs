use flowistry::{range::Range, source_map::find_bodies};
use rustc_macros::Encodable;

use crate::FlowistryResult;

#[derive(Encodable)]
pub struct SpansOutput {
  spans: Vec<Range>,
}

struct Callbacks {
  filename: String,
  output: Option<SpansOutput>,
}

impl rustc_driver::Callbacks for Callbacks {
  fn after_parsing<'tcx>(
    &mut self,
    compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let spans = find_bodies(tcx).into_iter().map(|(span, _)| span);

      let source_map = compiler.session().source_map();
      let files = source_map.files();
      let source_file = Range {
        start: 0,
        end: 0,
        filename: self.filename.clone(),
      }
      .source_file(&files)
      .unwrap();

      let spans = spans
        .into_iter()
        .filter(|span| {
          source_map.lookup_source_file(span.lo()).name_hash == source_file.name_hash
        })
        .filter_map(|span| Range::from_span(span, source_map).ok())
        .collect::<Vec<_>>();
      self.output = Some(SpansOutput { spans });
    });
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
