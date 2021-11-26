use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::FunctionIdentifier,
};
use anyhow::Result;
use flowistry::{
  indexed::{
    impls::{LocationSet, PlaceIndex},
    IndexedDomain,
  },
  infoflow,
  mir::{borrowck_facts::get_body_with_borrowck_facts, utils},
};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::{
  mir::{traversal, Location, ProjectionElem, VarDebugInfoContents, RETURN_PLACE},
  ty::{AdtKind, TyCtxt},
};
use rustc_span::Span;

#[derive(Debug, Clone, Encodable, Default)]
pub struct PlaceDescriptor {
  place: String,
  local: usize,
  name: String,
  projection: Vec<String>,
}

#[derive(Debug, Clone, Encodable, Default)]
pub struct GraphOutput {
  place_names: HashMap<usize, PlaceDescriptor>,
  location_names: HashMap<usize, String>,
  place_deps: HashMap<usize, HashMap<usize, HashSet<(usize, usize)>>>,
}

impl FlowistryOutput for GraphOutput {
  fn merge(&mut self, other: Self) {
    self.location_names.extend(other.location_names);
    self.place_names.extend(other.place_names);
    self.place_deps.extend(other.place_deps);
  }
}

pub struct GraphAnalysis {
  id: FunctionIdentifier,
}

impl FlowistryAnalysis for GraphAnalysis {
  type Output = GraphOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.id.to_span(tcx)?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;
    let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

    let place_domain = results.analysis.place_domain();
    let direct_places = place_domain
      .as_vec()
      .iter_enumerated()
      .filter(|(_, place)| !place.is_indirect() || utils::is_arg(**place, body))
      .collect::<Vec<_>>();

    let mut local_to_name = body
      .var_debug_info
      .iter()
      .filter_map(|info| match info.value {
        VarDebugInfoContents::Place(place) => Some((place.local, info.name.to_string())),
        _ => None,
      })
      .collect::<HashMap<_, _>>();
    local_to_name.insert(RETURN_PLACE, "RETURN".into());

    let place_names = direct_places
      .iter()
      .filter_map(|(index, place)| {
        let local_name = local_to_name.get(&place.local)?;
        let name = place
          .iter_projections()
          .fold(local_name.to_string(), |s, (place, elem)| match elem {
            ProjectionElem::Deref => format!("*{}", s),
            ProjectionElem::Field(f, _) => {
              let ty = place.ty(&body.local_decls, tcx).ty;
              let default = || format!("{}.{}", s, f.as_usize());
              if let Some(def) = ty.ty_adt_def() {
                match def.adt_kind() {
                  AdtKind::Struct => {
                    let name = def.non_enum_variant().fields[f.as_usize()].ident;
                    format!("{}.{}", s, name)
                  }
                  _ => default(),
                }
              } else {
                default()
              }
            }
            _ => unimplemented!(),
          });

        Some((
          index.as_usize(),
          PlaceDescriptor {
            place: format!("{:?}", place),
            name,
            local: place.local.as_usize(),
            projection: place
              .projection
              .iter()
              .map(|elem| format!("{:?}", elem))
              .collect(),
          },
        ))
      })
      .collect::<HashMap<_, _>>();

    let source_map = tcx.sess.source_map();
    let location_domain = results.analysis.location_domain();
    let location_names = location_domain
      .as_vec()
      .iter_enumerated()
      .filter(|(_, loc)| loc.block.as_usize() != body.basic_blocks().len())
      .map(|(index, loc)| {
        let span = body.source_info(*loc).span;
        let lines = source_map.span_to_lines(span).unwrap().lines;
        let s = if lines.len() == 1 {
          format!("{}", lines[0].line_index + 1)
        } else {
          format!(
            "{}-{}",
            lines[0].line_index + 1,
            lines[lines.len() - 1].line_index + 1
          )
        };
        (index.as_usize(), s)
      })
      .collect::<HashMap<_, _>>();

    let mut all_deps: HashMap<
      PlaceIndex,
      HashMap<Location, (LocationSet, HashSet<(Location, PlaceIndex)>)>,
    > = HashMap::default();
    for (block, data) in traversal::reverse_postorder(body) {
      let locations = (0..=data.statements.len()).map(|i| Location {
        block,
        statement_index: i,
      });
      for location in locations {
        let state = results.state_at(location);

        let rows = state
          .rows()
          .filter(|(place, deps)| {
            place_names.contains_key(&place.as_usize()) && deps.indices().next().is_some()
          })
          .collect::<Vec<_>>();

        for (place, place_loc_deps) in &rows {
          let place_entry = all_deps.entry(*place).or_default();
          let unchanged = place_entry
            .values()
            .any(|(prev_place_loc_deps, _)| prev_place_loc_deps.as_ref() == *place_loc_deps);
          if !unchanged {
            let place_place_deps = all_deps
              .iter()
              .filter_map(|(other, other_deps)| {
                let (loc, _) = other_deps
                  .iter()
                  .filter(|(_, (other_loc_deps, _))| place_loc_deps.is_superset(other_loc_deps))
                  .max_by_key(|(_, (other_loc_deps, _))| other_loc_deps.len())?;
                Some((*loc, *other))
              })
              .collect();
            all_deps
              .entry(*place)
              .or_default()
              .insert(location, (place_loc_deps.to_owned(), place_place_deps));
          }
        }
      }
    }

    let place_deps = all_deps
      .into_iter()
      .map(|(index, deps)| {
        (
          index.as_usize(),
          deps
            .into_iter()
            .map(|(loc, (_, deps))| {
              (
                location_domain.index(&loc).as_usize(),
                deps
                  .into_iter()
                  .map(|(loc, index)| (location_domain.index(&loc).as_usize(), index.as_usize()))
                  .collect(),
              )
            })
            .collect(),
        )
      })
      .collect();

    Ok(GraphOutput {
      place_names,
      location_names,
      place_deps,
    })
  }
}

pub fn graph(id: FunctionIdentifier, compiler_args: &[String]) -> FlowistryResult<GraphOutput> {
  GraphAnalysis { id }.run(compiler_args)
}
