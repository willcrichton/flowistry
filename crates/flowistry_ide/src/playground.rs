use anyhow::Result;
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_utils::{mir::borrowck_facts::get_body_with_borrowck_facts, BodyExt};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct PlaygroundOutput {
  outlives: HashSet<(String, String)>,
}

pub fn playground(tcx: TyCtxt, body_id: BodyId) -> Result<PlaygroundOutput> {
  let def_id = tcx.hir().body_owner_def_id(body_id);
  let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
  let body = &body_with_facts.body;
  debug!("{}", body.to_string(tcx).unwrap());

  let subset_base = &body_with_facts.input_facts.as_ref().unwrap().subset_base;
  let outlives = subset_base
    .iter()
    .map(|(sup, sub, _)| (format!("{sup:?}"), format!("{sub:?}")))
    .collect::<HashSet<_>>();

  Ok(PlaygroundOutput { outlives })
}
