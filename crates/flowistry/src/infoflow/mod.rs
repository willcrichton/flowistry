use std::cell::RefCell;

use log::debug;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;

pub use self::{
  analysis::{FlowAnalysis, FlowDomain},
  dependencies::{compute_dependencies, compute_dependency_spans, Direction},
};
use crate::{
  block_timer,
  mir::{
    aliases::Aliases, control_dependencies::ControlDependencies, engine, utils::BodyExt,
  },
};

mod analysis;
mod dependencies;
pub mod mutation;
mod recursive;

pub type FlowResults<'a, 'tcx> = engine::AnalysisResults<'tcx, FlowAnalysis<'a, 'tcx>>;

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
      rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s
        .print_expr(&tcx.hir().body(body_id).value))
    );
    debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

    let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
    let aliases = Aliases::build(tcx, def_id, body_with_facts);
    let location_domain = aliases.location_domain().clone();

    let body = &body_with_facts.body;
    let control_dependencies = ControlDependencies::build(body.clone());
    debug!("Control dependencies: {control_dependencies:?}");

    let results = {
      block_timer!("Flow");

      let analysis = FlowAnalysis::new(tcx, def_id, body, aliases, control_dependencies);
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
