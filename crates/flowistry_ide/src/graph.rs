use anyhow::Result;
use flowistry::{
  indexed::{
    impls::{LocationSet, PlaceIndex},
    IndexedDomain,
  },
  infoflow::{self, mutation::ModularMutationVisitor},
  mir::{
    borrowck_facts::get_body_with_borrowck_facts,
    utils::{BodyExt, PlaceExt},
  },
};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_hir::BodyId;
use rustc_macros::Encodable;
use rustc_middle::{
  mir::{
    traversal, visit::Visitor, Location, Place, ProjectionElem, VarDebugInfoContents,
    RETURN_PLACE,
  },
  ty::{AdtKind, TyCtxt},
};
use rustc_span::Span;

use crate::{
  analysis::{FlowistryAnalysis, FlowistryOutput, FlowistryResult},
  range::FunctionIdentifier,
};

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

  fn analyze_function(
    &mut self,
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
  ) -> Result<Self::Output> {
    let def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = get_body_with_borrowck_facts(tcx, def_id);
    let body = &body_with_facts.body;
    let results = &infoflow::compute_flow(tcx, body_id, body_with_facts);

    let _location_domain = results.analysis.location_domain();
    let place_domain = results.analysis.place_domain();

    // let (loops, outer) = find_loops(body, location_domain);

    let direct_places = place_domain
      .as_vec()
      .iter_enumerated()
      .filter(|(_, place)| {
        place.is_direct(body) && {
          let local_is_ref = Place::from_local(place.local, tcx)
            .ty(&body.local_decls, tcx)
            .ty
            .is_ref();
          !(place.is_arg(body) && local_is_ref && !place.is_indirect())
        }
      })
      .collect::<Vec<_>>();

    let source_map = tcx.sess.source_map();
    let mut local_to_name = body
      .var_debug_info
      .iter()
      .filter_map(|info| match info.value {
        VarDebugInfoContents::Place(place) => {
          let from_expansion = info.source_info.span.from_expansion();
          (!from_expansion).then(move || (place.local, info.name.to_string()))
        }
        _ => None,
      })
      .collect::<HashMap<_, _>>();
    local_to_name.insert(RETURN_PLACE, "RETURN".into());

    let place_to_string = |place: Place<'tcx>| -> Option<String> {
      let local_name = local_to_name.get(&place.local)?;
      Some(
        place
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
            ProjectionElem::Downcast(sym, _) => format!(
              "{} as {}",
              s,
              sym.map(|s| s.to_string()).unwrap_or_else(|| "??".into())
            ),
            ProjectionElem::Index(_) => format!("{}[]", s),
            _ => unimplemented!(),
          }),
      )
    };

    let place_names = direct_places
      .iter()
      .filter_map(|(index, place)| {
        Some((index.as_usize(), PlaceDescriptor {
          place: format!("{:?}", place),
          name: place_to_string(**place)?,
          local: place.local.as_usize(),
          projection: place
            .projection
            .iter()
            .map(|elem| format!("{:?}", elem))
            .collect(),
        }))
      })
      .collect::<HashMap<_, _>>();

    let location_domain = results.analysis.location_domain();
    let location_names = location_domain
      .as_vec()
      .iter_enumerated()
      .filter(|(_, loc)| loc.block.as_usize() != body.basic_blocks().len())
      .map(|(index, loc)| {
        let span = body.source_info(*loc).span.source_callsite();
        let lines = source_map.span_to_lines(span).unwrap().lines;
        let s = match &lines[..] {
          [] => "???".into(),
          [l] => format!("{}:{}-{}", l.line_index + 1, l.start_col.0, l.end_col.0),
          [l1, .., l2] => format!(
            "{}:{}-{}:{}",
            l1.line_index + 1,
            l1.start_col.0,
            l2.line_index + 1,
            l2.end_col.0
          ),
        };
        (index.as_usize(), s /*format!("{} ({:?})", s, loc)*/)
      })
      .collect::<HashMap<_, _>>();

    let mut all_deps: HashMap<
      PlaceIndex,
      HashMap<Location, (LocationSet, HashSet<(Location, PlaceIndex)>)>,
    > = HashMap::default();

    for (place, deps) in results
      .state_at(Location::START)
      .rows()
      .filter(|(place, _)| {
        place_domain.value(*place).is_arg(body)
          && place_names.contains_key(&place.as_usize())
      })
    {
      all_deps
        .entry(place)
        .or_default()
        .insert(Location::START, (deps.to_owned(), HashSet::default()));
    }

    for location in traversal::reverse_postorder(body)
      .map(|(block, _)| body.locations_in_block(block))
      .flatten()
    {
      let mut all_mutated = HashSet::default();

      let data = &body.basic_blocks()[location.block];
      let mut visitor =
        ModularMutationVisitor::new(tcx, body, def_id.to_def_id(), |mutated: Place<'tcx>, _, _, _| {
          all_mutated.insert(mutated);
        });
      if location.statement_index == data.statements.len() {
        visitor.visit_terminator(data.terminator(), location);
      } else {
        visitor.visit_statement(&data.statements[location.statement_index], location);
      }

      let all_mutated_conflicts = all_mutated
        .into_iter()
        .map(|place| {
          results
            .analysis
            .aliases
            .conflicts(place)
            .indices()
            .collect::<Vec<_>>()
        })
        .flatten()
        .filter(|place| place_names.contains_key(&place.as_usize()))
        .collect::<HashSet<_>>();

      let state = results.state_at(location);
      for place in all_mutated_conflicts.iter().copied() {
        let place_loc_deps = state.row_set(place).unwrap();
        let mut place_place_deps = all_deps
          .iter()
          .filter_map(|(other, other_deps)| {
            let (loc, _) = other_deps
              .iter()
              .filter(|(_, (other_loc_deps, _))| {
                place_loc_deps.is_superset(other_loc_deps)
              })
              .max_by_key(|(_, (other_loc_deps, _))| other_loc_deps.len())?;

            Some((*loc, *other))
          })
          .collect::<HashSet<_>>();

        let place_place_deps2 = place_place_deps.clone();
        place_place_deps.retain(|(loc1, place1)| {
          !place_place_deps2.iter().any(|(loc2, place2)| {
            let (deps1, _) = &all_deps[place1][loc1];
            let (deps2, _) = &all_deps[place2][loc2];
            deps2.len() > deps1.len() && deps2.is_superset(deps1)
          })
        });

        place_place_deps.extend(all_mutated_conflicts.iter().copied().filter_map(
          |other| {
            state.row_set(other).and_then(|other_loc_deps| {
              (place != other && place_loc_deps.is_superset(&other_loc_deps))
                .then(move || (location, other))
            })
          },
        ));

        all_deps
          .entry(place)
          .or_default()
          .insert(location, (place_loc_deps.to_owned(), place_place_deps));
      }
    }

    // for location in traversal::reverse_postorder(body)
    //   .map(|(block, _)| body.locations_in_block(block))
    //   .flatten()
    // {
    //   let state = results.state_at(location);

    //   let rows = state
    //     .rows()
    //     .filter(|(place, deps)| {
    //       place_names.contains_key(&place.as_usize()) && deps.indices().next().is_some()
    //     })
    //     .collect::<HashMap<_, _>>();

    //   let changed_places = rows
    //     .into_iter()
    //     .filter(|(place, place_loc_deps)| {
    //       let place_entry = all_deps.entry(*place).or_default();
    //       place_entry.values().all(|(prev_place_loc_deps, _)| {
    //         prev_place_loc_deps.as_ref() != *place_loc_deps
    //       })
    //     })
    //     .collect::<HashMap<_, _>>();

    //   for (place, place_loc_deps) in &changed_places {
    //     let mut place_place_deps = all_deps
    //       .iter()
    //       .filter_map(|(other, other_deps)| {
    //         let (loc, _) = other_deps
    //           .iter()
    //           .filter(|(_, (other_loc_deps, _))| {
    //             place_loc_deps.is_superset(other_loc_deps)
    //           })
    //           .max_by_key(|(_, (other_loc_deps, _))| other_loc_deps.len())?;

    //         Some((*loc, *other))
    //       })
    //       .collect::<HashSet<_>>();

    //     debug!(
    //       "Adding {} @ {:?} -- {:?}",
    //       place_names[&place.as_usize()].name,
    //       location,
    //       place_place_deps
    //     );
    //     all_deps
    //       .entry(*place)
    //       .or_default()
    //       .insert(location, (place_loc_deps.to_owned(), place_place_deps));
    //   }
    // }

    // from: Place -> Location -> {(Location, Place)}
    // to: int -> int -> {(int, int)}
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
                  .map(|(loc, index)| {
                    (location_domain.index(&loc).as_usize(), index.as_usize())
                  })
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

pub fn graph(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<GraphOutput> {
  GraphAnalysis { id }.run(compiler_args)
}
