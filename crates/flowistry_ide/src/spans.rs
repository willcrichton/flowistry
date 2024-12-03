use rustc_middle::ty::TyCtxt;
use rustc_utils::source_map::{
  filename::Filename, find_bodies::find_bodies, range::CharRange,
};
use serde::Serialize;

use crate::plugin::{FlowistryError, FlowistryResult};

#[derive(Serialize)]
pub struct SpansOutput {
  spans: Vec<CharRange>,
}

unsafe impl Send for SpansOutput {}

struct Callbacks {
  filename: String,
  output: Option<FlowistryResult<SpansOutput>>,
}

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    tcx: TyCtxt<'tcx>,
  ) -> rustc_driver::Compilation {
    let spans = find_bodies(tcx).into_iter().map(|(span, _)| span);

    self.output = Some((|| {
      let source_map = tcx.sess.source_map();
      let source_file = Filename::intern(&self.filename)
        .find_source_file(source_map)
        .map_err(|_| FlowistryError::FileNotFound)?;

      let spans = spans
        .into_iter()
        .filter(|span| source_map.lookup_source_file(span.lo()).name == source_file.name)
        .filter_map(|span| CharRange::from_span(span, source_map).ok())
        .collect::<Vec<_>>();
      Ok(SpansOutput { spans })
    })());

    rustc_driver::Compilation::Stop
  }
}

pub fn spans(args: &[String], filename: String) -> FlowistryResult<SpansOutput> {
  let mut callbacks = Callbacks {
    filename,
    output: None,
  };
  crate::plugin::run_with_callbacks(args, &mut callbacks)?;
  callbacks.output.unwrap()
}
