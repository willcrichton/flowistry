#![allow(dead_code)]

use std::io::Write;

use anyhow::Result;
use flowistry::{infoflow::FlowResults, mir::utils::PlaceSet};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::{def::Res, def_id::DefId, BodyId};
use rustc_infer::traits::EvaluationResult;
use rustc_middle::{
  mir::*,
  ty::{ParamEnv, Ty, TyCtxt, TypingMode},
};
use rustc_mir_dataflow::JoinSemiLattice;
use rustc_span::FileName;
use rustc_trait_selection::infer::{InferCtxtExt, TyCtxtInferExt};
use rustc_utils::{BodyExt, PlaceExt, SpanExt};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

fn implements_trait<'tcx>(
  tcx: TyCtxt<'tcx>,
  param_env: ParamEnv<'tcx>,
  ty: Ty<'tcx>,
  trait_def_id: DefId,
) -> bool {
  let infcx = tcx.infer_ctxt().build(TypingMode::non_body_analysis());
  let ty = tcx.erase_regions(ty);
  let result = infcx.type_implements_trait(trait_def_id, [ty], param_env);
  matches!(
    result,
    EvaluationResult::EvaluatedToOk | EvaluationResult::EvaluatedToOkModuloRegions
  )
}

pub enum IssueFound {
  Yes,
  No,
}

pub fn analyze(body_id: &BodyId, results: &FlowResults) -> Result<IssueFound> {
  let tcx = results.analysis.tcx;
  let body = results.analysis.body;
  let def_id = tcx.hir().body_owner_def_id(*body_id).to_def_id();

  log::debug!(
    "Crates: {:?}",
    tcx
      .crates(())
      .iter()
      .map(|krate| tcx.crate_name(*krate))
      .collect::<Vec<_>>()
  );
  let ifc_crate = match tcx
    .crates(())
    .iter()
    .find(|krate| tcx.crate_name(**krate).as_str() == "flowistry_ifc_traits")
  {
    Some(c) => *c,
    None => {
      return Ok(IssueFound::No);
    }
  };

  let ifc_mod = DefId {
    krate: ifc_crate,
    index: rustc_hir::def_id::CRATE_DEF_INDEX,
  };
  let ifc_items = tcx
    .module_children(ifc_mod)
    .iter()
    .filter_map(|export| match export.res {
      Res::Def(_, id) => Some((export.ident.to_string(), id)),
      _ => None,
    })
    .collect::<HashMap<_, _>>();

  let all_places = body
    .local_decls()
    .indices()
    .flat_map(|local| {
      let place = Place::from_local(local, tcx);
      place.interior_places(tcx, body, def_id)
    })
    .collect::<PlaceSet>();

  let find_implements = |trait_def_id| -> PlaceSet {
    all_places
      .iter()
      .copied()
      .filter(|place| {
        let ty = place.ty(body.local_decls(), tcx).ty;
        implements_trait(tcx, tcx.param_env(def_id), ty, trait_def_id)
      })
      .collect()
  };
  let secure_places = find_implements(ifc_items["Secure"]);
  let insecure_places = find_implements(ifc_items["Insecure"]);

  let final_state = body
    .all_returns()
    .map(|location| results.state_at(location).clone())
    .reduce(|mut a, b| {
      a.join(&b);
      a
    })
    .unwrap();

  let mut errors = Vec::new();
  for secure in secure_places.iter() {
    let secure_deps = results.analysis.deps_for(&final_state, *secure);
    for insecure in insecure_places.iter() {
      let insecure_deps = results.analysis.deps_for(&final_state, *insecure);
      if insecure_deps.is_superset(&secure_deps) {
        errors.push((secure, insecure));
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
  let has_errors = !errors.is_empty();
  for (src, dst) in errors {
    let body_span = tcx.hir().span_with_body(body_id.hir_id);
    let src_span = decls[src.local].source_info.span.as_local(body_span);
    let dst_span = decls[dst.local].source_info.span.as_local(body_span);

    let span_range = |span| match span {
      Some(span) => {
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
      }
      None => "<in macro expansion>".to_owned(),
    };

    let span_contents = |span| match span {
      Some(span) => source_map.span_to_snippet(span).unwrap(),
      None => "<in macro expansion>".to_owned(),
    };

    stdout.set_color(&red_spec)?;
    writeln!(
      stdout,
      "ERROR: insecure flow in {filename} from data at {src_span}:",
      filename = filename
        .local_path_if_available()
        .file_name()
        .unwrap()
        .to_string_lossy(),
      src_span = span_range(src_span)
    )?;

    stdout.set_color(&black_spec)?;
    writeln!(
      stdout,
      "  {src_snippet}",
      src_snippet = span_contents(src_span)
    )?;

    stdout.set_color(&red_spec)?;
    writeln!(
      stdout,
      "to data at {dst_span}:",
      dst_span = span_range(dst_span)
    )?;

    stdout.set_color(&black_spec)?;
    writeln!(
      stdout,
      "  {dst_snippet}\n",
      dst_snippet = span_contents(dst_span)
    )?;
  }

  Ok(if has_errors {
    IssueFound::Yes
  } else {
    IssueFound::No
  })
}
