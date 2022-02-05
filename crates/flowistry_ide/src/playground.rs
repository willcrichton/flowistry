use anyhow::Result;
use flowistry::mir::{borrowck_facts::get_body_with_borrowck_facts, utils::BodyExt};
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::ty::TyCtxt;

#[derive(Debug, Clone, Encodable, Default)]
pub struct PlaygroundOutput {
  outlives: HashSet<(String, String)>,
}

pub fn playground(tcx: TyCtxt<'tcx>, body_id: BodyId) -> Result<PlaygroundOutput> {
  let def_id = tcx.hir().body_owner_def_id(body_id);
  let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
  let body = &body_with_facts.body;
  debug!("{}", body.to_string(tcx).unwrap());

  let outlives = body_with_facts
    .input_facts
    .subset_base
    .iter()
    .map(|(sup, sub, _)| (format!("{sup:?}"), format!("{sub:?}")))
    .collect::<HashSet<_>>();

  Ok(PlaygroundOutput { outlives })
}
