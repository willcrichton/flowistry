use crate::{block_timer, mir::utils::SimplifyMir};

use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::{
  self,
  query::{query_values::mir_borrowck, Providers},
  TyCtxt,
};
use rustc_mir_transform::MirPass;

use std::{cell::RefCell, pin::Pin};

// For why we need to do override mir_borrowck, see:
// https://github.com/rust-lang/rust/blob/485ced56b8753ec86936903f2a8c95e9be8996a1/src/test/run-make-fulldeps/obtain-borrowck/driver.rs
pub fn override_queries(
  _session: &rustc_session::Session,
  local: &mut Providers,
  external: &mut Providers,
) {
  local.mir_borrowck = mir_borrowck;
  external.mir_borrowck = mir_borrowck;
}

thread_local! {
  static MIR_BODIES: RefCell<HashMap<LocalDefId, Pin<Box<BodyWithBorrowckFacts<'static>>>>> =
    RefCell::new(HashMap::default());
}

fn mir_borrowck<'tcx>(tcx: TyCtxt<'tcx>, def_id: LocalDefId) -> mir_borrowck<'tcx> {
  block_timer!(&format!(
    "get_body_with_borrowck_facts for {}",
    tcx.def_path_debug_str(def_id.to_def_id())
  ));

  let mut body_with_facts = rustc_borrowck::consumers::get_body_with_borrowck_facts(
    tcx,
    ty::WithOptConstParam::unknown(def_id),
  );

  let body = &mut body_with_facts.body;
  SimplifyMir.run_pass(tcx, body);

  // SAFETY: The reader casts the 'static lifetime to 'tcx before using it.
  let body_with_facts: BodyWithBorrowckFacts<'static> =
    unsafe { std::mem::transmute(body_with_facts) };
  MIR_BODIES.with(|state| {
    let mut map = state.borrow_mut();
    assert!(map
      .insert(def_id, Pin::new(Box::new(body_with_facts)))
      .is_none());
  });

  let mut providers = Providers::default();
  rustc_borrowck::provide(&mut providers);
  let original_mir_borrowck = providers.mir_borrowck;
  original_mir_borrowck(tcx, def_id)
}

pub fn get_body_with_borrowck_facts(
  tcx: TyCtxt<'tcx>,
  def_id: LocalDefId,
) -> &'tcx BodyWithBorrowckFacts<'tcx> {
  let _ = tcx.mir_borrowck(def_id);
  MIR_BODIES.with(|state| {
    let state = state.borrow();
    let body = &*state.get(&def_id).unwrap();
    unsafe {
      std::mem::transmute::<&BodyWithBorrowckFacts<'static>, &'tcx BodyWithBorrowckFacts<'tcx>>(
        body,
      )
    }
  })
}
