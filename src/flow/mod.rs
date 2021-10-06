use std::cell::RefCell;

use crate::{
  config::{EvalMode, EVAL_MODE},
  core::{aliases::Aliases, control_dependencies::ControlDependencies},
  utils,
};

use log::debug;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_mir_dataflow::{Analysis, Results};
use std::{mem::transmute, pin::Pin};

pub use dataflow::{FlowAnalysis, FlowDomain};
pub use dependencies::{compute_dependencies, compute_dependency_ranges, Direction};

mod dataflow;
mod dependencies;

type CacheKey = (BodyId, Option<EvalMode>);
type FlowResults<'a, 'b> = Results<'b, FlowAnalysis<'a, 'b>>;

thread_local! {
  pub static BODY_STACK: RefCell<Vec<BodyId>> =
    RefCell::new(Vec::new());
  static CACHE: RefCell<HashMap<CacheKey, Pin<Box<FlowResults<'static, 'static>>>>> =
    RefCell::new(HashMap::default());
}

pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> &'a FlowResults<'a, 'tcx> {
  let run = || {
    debug!(
      "{}",
      utils::mir_to_string(tcx, &body_with_facts.body).unwrap()
    );

    let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
    let aliases = Aliases::build(tcx, def_id, body_with_facts);

    let body = &body_with_facts.body;
    let control_dependencies = ControlDependencies::build(body.clone());
    debug!("Control dependencies: {:?}", control_dependencies);

    {
      let _timer = utils::block_timer("Flow");

      FlowAnalysis::new(tcx, def_id, body, aliases, control_dependencies)
        .into_engine(tcx, body)
        .iterate_to_fixpoint()
    }
  };

  CACHE.with(|cache| {
    let key = (body_id, EVAL_MODE.copied());
    if !cache.borrow().contains_key(&key) {
      let results = BODY_STACK.with(|body_stack| {
        body_stack.borrow_mut().push(body_id);
        let results = run();
        body_stack.borrow_mut().pop();
        results
      });

      let results =
        unsafe { transmute::<FlowResults<'a, 'tcx>, FlowResults<'static, 'static>>(results) };
      cache.borrow_mut().insert(key, Pin::new(Box::new(results)));
    }

    // TODO: SCARY UNSAFETY
    //    better way to implement this w/o transmute?
    let mut cache = cache.borrow_mut();
    let results = &mut **cache.get_mut(&key).unwrap();
    let results = unsafe {
      transmute::<&mut FlowResults<'static, 'static>, &'a mut FlowResults<'a, 'tcx>>(results)
    };
    results.analysis.body = &body_with_facts.body;

    results
  })
}
