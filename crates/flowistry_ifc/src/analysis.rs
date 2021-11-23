#![allow(dead_code)]
#![allow(dead_code)]

use anyhow::{Context, Result};
use flowistry::{
  indexed::{IndexSetIteratorExt, IndexedDomain},
  infoflow::FlowResults,
  mir::utils,
};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::{def::Res, def_id::DefId, BodyId};
use rustc_infer::traits::EvaluationResult;
use rustc_middle::{
  mir::*,
  ty::{ParamEnv, Ty, TyCtxt},
};
use rustc_mir_dataflow::JoinSemiLattice;
use rustc_span::FileName;
use rustc_trait_selection::infer::{InferCtxtExt, TyCtxtInferExt};
use std::io::Write;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

fn implements_trait(
  tcx: TyCtxt<'tcx>,
  param_env: ParamEnv<'tcx>,
  ty: Ty<'tcx>,
  trait_def_id: DefId,
) -> bool {
  tcx.infer_ctxt().enter(|infcx| {
    let ty = tcx.erase_regions(ty);
    let result =
      infcx.type_implements_trait(trait_def_id, ty, tcx.mk_substs_trait(ty, &[]), param_env);
    matches!(
      result,
      EvaluationResult::EvaluatedToOk | EvaluationResult::EvaluatedToOkModuloRegions
    )
  })
}

pub fn analyze(body_id: &BodyId, results: &FlowResults) -> Result<()> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;
  let def_id = tcx.hir().body_owner_def_id(*body_id).to_def_id();

  let ifc_crate = tcx
    .crates(())
    .iter()
    .copied()
    .find(|krate| tcx.crate_name(*krate).as_str() == "flowistry_ifc_traits")
    .context("Could not find flowistry_ifc_traits crate")?;
  let ifc_mod = DefId {
    krate: ifc_crate,
    index: rustc_hir::def_id::CRATE_DEF_INDEX,
  };
  let ifc_items = tcx
    .item_children(ifc_mod)
    .iter()
    .filter_map(|export| match export.res {
      Res::Def(_, id) => Some((export.ident.to_string(), id)),
      _ => None,
    })
    .collect::<HashMap<_, _>>();

  let place_domain = results.analysis.place_domain();
  let find_implements = |trait_def_id| {
    place_domain
      .as_vec()
      .iter()
      .filter(|place| {
        let ty = place.ty(body.local_decls(), tcx).ty;
        implements_trait(tcx, tcx.param_env(def_id), ty, trait_def_id)
      })
      .collect_indices(place_domain.clone())
  };
  let secure_places = find_implements(ifc_items["Secure"]);
  let insecure_places = find_implements(ifc_items["Insecure"]);

  let final_state = utils::all_returns(body)
    .into_iter()
    .map(|location| results.state_at(location).clone())
    .reduce(|mut a, b| {
      a.join(&b);
      a
    })
    .unwrap();

  let mut errors = Vec::new();
  for secure in secure_places.indices() {
    if let Some(secure_deps) = final_state.row_set(secure) {
      for insecure in insecure_places.indices() {
        if let Some(insecure_deps) = final_state.row_set(insecure) {
          if insecure_deps.is_superset(&secure_deps) {
            errors.push((secure, insecure));
          }
        }
      }
    }
  }

  let mut stdout = StandardStream::stderr(ColorChoice::Auto);
  let mut black_spec = ColorSpec::new();
  black_spec.set_fg(Some(Color::Yellow));
  let mut red_spec = ColorSpec::new();
  red_spec.set_fg(Some(Color::Red));

  let decls = body.local_decls();
  let source_map = tcx.sess.source_map();
  let filename = match source_map.span_to_filename(body.span) {
    FileName::Real(f) => f,
    _ => unimplemented!(),
  };
  for (src, dst) in errors {
    let src = place_domain.value(src);
    let dst = place_domain.value(dst);
    let src_span = decls[src.local].source_info.span;
    let dst_span = decls[dst.local].source_info.span;

    let fmt_span = |span| {
      let lines = source_map.span_to_lines(span).unwrap();
      let first = lines.lines.first().unwrap();
      let last = lines.lines.last().unwrap();
      format!(
        "{}:{}-{}:{}",
        first.line_index + 1,
        first.start_col.0,
        last.line_index + 1,
        last.end_col.0
      )
    };

    stdout.set_color(&red_spec)?;
    writeln!(
      &mut stdout,
      "ERROR: insecure flow in {filename} from data at {src_span}:",
      filename = filename
        .local_path_if_available()
        .file_name()
        .unwrap()
        .to_string_lossy(),
      src_span = fmt_span(src_span.source_callsite())
    )?;

    stdout.set_color(&black_spec)?;
    writeln!(
      &mut stdout,
      "  {src_snippet}",
      src_snippet = source_map.span_to_snippet(src_span).unwrap()
    )?;

    stdout.set_color(&red_spec)?;
    writeln!(
      &mut stdout,
      "to data at {dst_span}:",
      dst_span = fmt_span(dst_span.source_callsite())
    )?;

    stdout.set_color(&black_spec)?;
    writeln!(
      &mut stdout,
      "  {dst_snippet}",
      dst_snippet = if dst_span.from_expansion() {
        "<in macro expansion>".to_string()
      } else {
        source_map.span_to_snippet(dst_span).unwrap()
      },
    )?;

    // stdout.set_color(&red_spec)?;
    // writeln!(
    //   &mut stdout,
    //   "at the instruction {instr_span}:",
    //   instr_span = fmt_span(instr_span.source_callsite())
    // )?;

    // stdout.set_color(&black_spec)?;
    // writeln!(
    //   &mut stdout,
    //   "  {instr_snippet}",
    //   instr_snippet = source_map.span_to_snippet(instr_span).unwrap(),
    // )?;
  }

  Ok(())
}
