use crate::{
  backward_slicing::Range,
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    utils,
  },
  flow::{self, dependencies},
};
use anyhow::Result;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_middle::{
  mir::{Local, Location},
  ty::{TyCtxt, WithOptConstParam},
};
use rustc_mir::consumers::get_body_with_borrowck_facts;
use rustc_span::Span;
use serde::Serialize;
use visitor::EffectKind;

mod visitor;

#[derive(Debug, Serialize)]
pub struct Effect {
  effect: Range,
  slice: Vec<Range>,
}

#[derive(Debug, Default, Serialize)]
pub struct EffectsOutput {
  args_effects: HashMap<usize, Vec<Effect>>,
  arg_spans: HashMap<usize, Range>,
  returns: Vec<Effect>,
}

impl FlowistryOutput for EffectsOutput {
  fn empty() -> Self {
    EffectsOutput::default()
  }

  fn merge(&mut self, other: Self) {
    self.args_effects.extend(other.args_effects.into_iter());
    self.arg_spans.extend(other.arg_spans.into_iter());
    self.returns.extend(other.returns.into_iter());
  }
}

struct EffectsHarness {
  id: FunctionIdentifier,
}

pub enum FunctionIdentifier {
  Qpath(String),
  Range(Range),
}

impl FunctionIdentifier {
  pub fn to_span(&self, tcx: TyCtxt) -> Result<Span> {
    match self {
      FunctionIdentifier::Qpath(qpath) => utils::qpath_to_span(tcx, qpath.clone()),
      FunctionIdentifier::Range(range) => range.to_span(tcx.sess.source_map()),
    }
  }
}

impl FlowistryAnalysis for EffectsHarness {
  type Output = EffectsOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.id.to_span(tcx)?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts =
      get_body_with_borrowck_facts(tcx, WithOptConstParam::unknown(local_def_id));
    let body = &body_with_facts.body;
    let flow_results = flow::compute_flow(tcx, body, &body_with_facts.input_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/effects.png", body, &flow_results)?;
    }

    let mut find_effects = visitor::FindEffects::new(&flow_results.analysis);
    flow_results.visit_reachable_with(body, &mut find_effects);

    let hir_body = tcx.hir().body(body_id);
    let spanner = utils::HirSpanner::new(hir_body);

    let (effects, targets): (Vec<_>, Vec<_>) = find_effects
      .effects
      .into_iter()
      .map(|(kind, effects)| {
        effects
          .into_iter()
          .map(move |(place, loc)| ((kind, loc), (place, loc)))
      })
      .flatten()
      .unzip();

    let deps = dependencies::compute_dependency_ranges(
      &flow_results,
      targets,
      dependencies::Direction::Backward,
      &spanner,
    );

    let source_map = tcx.sess.source_map();
    let loc_to_range = |loc: Location| -> Option<Range> {
      let mir_span = body.source_info(loc).span;
      let hir_span = spanner.find_enclosing_hir_span(mir_span);
      hir_span
        .into_iter()
        .map(|hir_span| Range::from_span(hir_span, source_map).ok())
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .min_by_key(|range| range.end - range.end)
    };

    let ranged_effects =
      effects
        .into_iter()
        .zip(deps.into_iter())
        .filter_map(|((kind, loc), slice)| {
          let effect = Effect {
            effect: loc_to_range(loc)?,
            slice,
          };
          Some((kind, effect))
        });

    let mut output = EffectsOutput::default();
    for (kind, effect) in ranged_effects {
      match kind {
        EffectKind::Return => {
          output.returns.push(effect);
        }
        EffectKind::MutArg(arg) => {
          let arg_span = body.local_decls[Local::from_usize(arg + 1)]
            .source_info
            .span;
          output
            .arg_spans
            .insert(arg, Range::from_span(arg_span, source_map).unwrap());
          output
            .args_effects
            .entry(arg)
            .or_insert_with(Vec::new)
            .push(effect);
        }
      }
    }

    Ok(output)
  }
}

pub fn effects(id: FunctionIdentifier, compiler_args: &[String]) -> Result<EffectsOutput> {
  EffectsHarness { id }.run(compiler_args)
}
