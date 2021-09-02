use crate::{
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    config::Range,
    indexed::IndexedDomain,
    utils,
  },
  flow::{self, Direction},
};
use anyhow::Result;
use log::debug;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_middle::{mir::ProjectionElem, ty::TyCtxt};

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
  args_effects: HashMap<String, Vec<Effect>>,
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
    let body_with_facts = utils::get_body_with_borrowck_facts(tcx, body_id);
    let body = &body_with_facts.body;
    debug!("{}", utils::mir_to_string(tcx, body)?);

    let flow_results = flow::compute_flow(tcx, body_id, body, &body_with_facts.input_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/effects.png", body, &flow_results)?;
    }

    let mut find_effects = visitor::FindEffects::new(&flow_results.analysis);
    flow_results.visit_reachable_with(body, &mut find_effects);
    debug!("effects: {:#?}", find_effects.effects);

    let spanner = utils::HirSpanner::new(tcx, body_id);

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

    let deps =
      flow::compute_dependency_ranges(&flow_results, targets, Direction::Backward, &spanner);

    let source_map = tcx.sess.source_map();
    let ranged_effects =
      effects
        .into_iter()
        .zip(deps.into_iter())
        .filter_map(|((kind, loc), slice)| {
          let spans = utils::location_to_spans(loc, body, &spanner);
          let range = spans
            .into_iter()
            .min_by_key(|span| span.hi() - span.lo())
            .and_then(|span| Range::from_span(span, source_map).ok())?;
          let effect = Effect {
            effect: range,
            slice,
          };
          Some((kind, effect))
        });

    let mut output = EffectsOutput::default();
    for (kind, effect) in ranged_effects {
      match kind {
        EffectKind::Return => {
          if !body.return_ty().is_unit() {
            output.returns.push(effect);
          }
        }
        EffectKind::MutArg(arg) => {
          let arg_place = flow_results.analysis.place_domain().value(arg);

          let effect_str = {
            let local_span = body.local_decls[arg_place.local].source_info.span;
            let local_str = source_map.span_to_snippet(local_span).unwrap();

            arg_place
              .iter_projections()
              .fold(local_str, |acc, (place, elem)| {
                let ty = place.ty(&body.local_decls, tcx).ty;
                match elem {
                  ProjectionElem::Field(field, _) => {
                    let adt_def = ty.ty_adt_def().unwrap();
                    let field_def = adt_def.all_fields().nth(field.as_usize()).unwrap();
                    format!("{}.{}", acc, field_def.ident)
                  }
                  ProjectionElem::Downcast(_, variant) => {
                    let adt_def = ty.ty_adt_def().unwrap();
                    let variant_def = &adt_def.variants[variant];
                    format!("{} as {}", acc, variant_def.ident)
                  }
                  ProjectionElem::Deref => format!("(*{})", acc),
                  ProjectionElem::Index(_) => format!("{}[]", acc),
                  ProjectionElem::ConstantIndex { .. } => {
                    format!("{}[TODO]", acc)
                  }
                  ProjectionElem::Subslice { from, to, .. } => {
                    format!("{}[{}..{}]", acc, from, to)
                  }
                }
              })
          };

          output
            .args_effects
            .entry(effect_str)
            .or_default()
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
