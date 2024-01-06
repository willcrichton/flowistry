use flowistry::pdg::PdgParams;
use rustc_utils::{mir::borrowck_facts, source_map::find_bodies::find_bodies};
use serde::Serialize;

use crate::plugin::FlowistryResult;

#[derive(Serialize)]
pub struct GraphOutput {}

struct Callbacks {
  item_name: String,
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
      let defs = find_bodies(tcx)
        .into_iter()
        .filter_map(|(_, body_id)| {
          let def_id = tcx.hir().body_owner_def_id(body_id);
          tcx
            .def_path_str(def_id)
            .ends_with(&self.item_name)
            .then_some(def_id)
        })
        .collect::<Vec<_>>();
      if defs.len() == 0 {
        panic!("Could not find definition for: {}", self.item_name);
      } else if defs.len() > 1 {
        panic!("Ambiguous name. Found multiple definitions: {:?}", defs);
      }

      let def = *defs.first().unwrap();
      let params = PdgParams::new(tcx, def);
      let graph = flowistry::pdg::compute_pdg(params);
      println!("PDG generated. Creating graphviz diagram at target/graph.pdf");
      graph.generate_graphviz("target/graph.pdf").unwrap();

      self.output = Some(Ok(GraphOutput {}))
    });
    rustc_driver::Compilation::Stop
  }
}

pub fn graph(args: &[String], item_name: String) -> FlowistryResult<GraphOutput> {
  let mut callbacks = Callbacks {
    item_name,
    output: None,
  };
  crate::plugin::run_with_callbacks(args, &mut callbacks)?;
  callbacks.output.unwrap()
}
