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
use intervaltree::IntervalTree;
use log::debug;
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_hir::BodyId;
use rustc_middle::{
  mir::ProjectionElem,
  ty::{TyCtxt, TyKind},
};

use rustc_macros::Encodable;
use rustc_span::Span;
use visitor::EffectKind;

mod visitor;

#[derive(Debug, Encodable)]
pub struct Effect {
  effect: Range,
  slice: Vec<Range>,
  unique: Vec<Range>,
}

#[derive(Debug, Default, Encodable)]
pub struct EffectsOutput {
  args_effects: Vec<(String, Vec<Effect>)>,
  arg_spans: HashMap<usize, Range>,
  returns: Vec<Effect>,
  body_span: Range,
  fn_name: String
}

impl FlowistryOutput for EffectsOutput {
  fn merge(&mut self, other: Self) {
    self.args_effects.extend(other.args_effects.into_iter());
    self.arg_spans.extend(other.arg_spans.into_iter());
    self.returns.extend(other.returns.into_iter());
    self.body_span = other.body_span;
    self.fn_name = other.fn_name;
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

    let flow_results = flow::compute_flow(tcx, body_id, &body_with_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/effects.png", body, &flow_results)?;
    }

    let mut find_effects = visitor::FindEffects::new(&flow_results.analysis);
    flow_results.visit_reachable_with(body, &mut find_effects);
    debug!("effects: {:?}", find_effects.effects);

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
    let mut ranged_effects = effects
      .into_iter()
      .zip(deps.into_iter())
      .filter_map(|((kind, loc), slice)| {
        let spans = utils::location_to_spans(loc, body, &spanner, source_map);
        let range = spans
          .into_iter()
          .min_by_key(|span| span.hi() - span.lo())
          .and_then(|span| Range::from_span(span, source_map).ok())?;
        let effect = Effect {
          effect: range,
          slice,
          unique: Vec::new(),
        };
        Some((kind, effect))
      })
      .collect::<Vec<_>>();

    for i in 0..ranged_effects.len() {
      let other_ranges = ranged_effects
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, (_, effect))| effect.slice.clone().into_iter())
        .flatten()
        .map(|range| (range.start..range.end, ()))
        .collect::<IntervalTree<_, _>>();

      let unique = ranged_effects[i]
        .1
        .slice
        .iter()
        .filter(|range| other_ranges.query(range.start..range.end).next().is_none())
        .cloned()
        .collect::<Vec<_>>();

      debug!("{}: {:?}", i, unique);
      ranged_effects[i].1.unique = unique;
    }

    let body_span = Range::from_span(
      tcx.hir().body(body_id).value.span,
      source_map,
    )?;
    let fn_name = tcx.def_path_str(tcx.hir().body_owner_def_id(body_id).to_def_id());
    let mut output = EffectsOutput {
      body_span,
      fn_name,
      ..Default::default()
    };

    let fn_decl = tcx
      .hir()
      .fn_decl_by_hir_id(tcx.hir().body_owner(body_id))
      .unwrap();

    let mut args_effects: HashMap<_, Vec<_>> = HashMap::default();
    let mut effect_str_order = HashMap::default();

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
            let local_str =
              if arg_place.local.as_usize() == 1 && fn_decl.implicit_self.has_implicit_self() {
                "self".to_string()
              } else {
                let local_span = body.local_decls[arg_place.local].source_info.span;
                source_map.span_to_snippet(local_span).unwrap()
              };

            arg_place
              .iter_projections()
              .fold(local_str, |acc, (place, elem)| {
                let ty = place.ty(&body.local_decls, tcx).ty;
                match elem {
                  ProjectionElem::Field(field, _) => {
                    let field_str = match ty.kind() {
                      TyKind::Tuple(..) => format!("{}", field.as_usize()),
                      TyKind::Adt(..) => {
                        let adt_def = ty.ty_adt_def().unwrap();
                        let field_def = adt_def.all_fields().nth(field.as_usize()).unwrap();
                        format!("{}", field_def.ident)
                      }
                      _ => unimplemented!("{:?}", ty),
                    };
                    format!("{}.{}", acc, field_str)
                  }
                  ProjectionElem::Downcast(_, variant) => {
                    let adt_def = ty.ty_adt_def().unwrap();
                    let variant_def = &adt_def.variants[variant];
                    format!("{} as {}", acc, variant_def.ident)
                  }
                  ProjectionElem::Deref => acc,
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

          let mut order = vec![arg_place.local.as_usize()];
          order.extend(arg_place.projection.iter().map(|elem| match elem {
            ProjectionElem::Field(f, _) => f.as_usize(),
            _ => 0,
          }));
          effect_str_order.insert(effect_str.clone(), order);

          args_effects.entry(effect_str).or_default().push(effect);
        }
      }
    }


    output.args_effects = args_effects.into_iter().collect::<Vec<_>>();
    output
      .args_effects
      .sort_by_key(|(k, _)| &effect_str_order[k]);

    for (_, effects) in output.args_effects.iter_mut() {
      effects.sort_by_key(|e| e.effect.start);
    }

    Ok(output)
  }
}

pub fn effects(id: FunctionIdentifier, compiler_args: &[String]) -> Result<EffectsOutput> {
  EffectsHarness { id }.run(compiler_args)
}
