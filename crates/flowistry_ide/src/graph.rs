use rustc_utils::mir::borrowck_facts;
use serde::Serialize;

use crate::plugin::FlowistryResult;

#[derive(Serialize)]
pub struct GraphOutput {}

struct Callbacks {
  output: Option<FlowistryResult<GraphOutput>>,
}

impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    borrowck_facts::enable_mir_simplification();
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().enter(|tcx| {
      let (main_def_id, _) = tcx.entry_fn(()).unwrap();
      let main_def_id = main_def_id.expect_local();
      let graph = flowistry::pdg::compute_pdg(tcx, main_def_id);
      graph.generate_graphviz("target/graph.pdf").unwrap();

      self.output = Some(Ok(GraphOutput {}))
    });
    rustc_driver::Compilation::Stop
  }
}

pub fn graph(args: &[String]) -> FlowistryResult<GraphOutput> {
  let mut callbacks = Callbacks { output: None };
  crate::plugin::run_with_callbacks(args, &mut callbacks)?;
  callbacks.output.unwrap()
}
