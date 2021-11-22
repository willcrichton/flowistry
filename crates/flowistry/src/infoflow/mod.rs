use crate::{
  block_timer,
  indexed::impls::LocationDomain,
  mir::{aliases::Aliases, control_dependencies::ControlDependencies, engine, utils},
};
use log::debug;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use std::cell::RefCell;

pub use analysis::{FlowAnalysis, FlowDomain};
pub use dependencies::{compute_dependencies, compute_dependency_spans, Direction};

mod analysis;
mod dependencies;

pub type FlowResults<'a, 'b> = engine::AnalysisResults<'b, FlowAnalysis<'a, 'b>>;

thread_local! {
  pub static BODY_STACK: RefCell<Vec<BodyId>> =
    RefCell::new(Vec::new());
}

pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> FlowResults<'a, 'tcx> {
  BODY_STACK.with(|body_stack| {
    body_stack.borrow_mut().push(body_id);
    debug!(
      "{}",
      utils::mir_to_string(tcx, &body_with_facts.body).unwrap()
    );

    let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
    let aliases = Aliases::build(tcx, def_id, body_with_facts);

    let body = &body_with_facts.body;
    let control_dependencies = ControlDependencies::build(body.clone());
    debug!("Control dependencies: {:?}", control_dependencies);

    let location_domain = LocationDomain::new(body, &aliases.place_domain);

    let results = {
      block_timer!("Flow");

      let analysis = FlowAnalysis::new(
        tcx,
        def_id,
        body,
        aliases,
        control_dependencies,
        location_domain.clone(),
      );
      engine::iterate_to_fixpoint(tcx, body, location_domain, analysis)

      // analysis.into_engine(tcx, body).iterate_to_fixpoint()
    };

    if std::env::var("DUMP_MIR").is_ok()
      && BODY_STACK.with(|body_stack| body_stack.borrow().len() == 1)
    {
      todo!()
      // utils::dump_results(body, &results, def_id, tcx).unwrap();
    }

    body_stack.borrow_mut().pop();

    results
  })
}
