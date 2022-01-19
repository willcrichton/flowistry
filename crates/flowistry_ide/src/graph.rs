#![allow(warnings)]
use anyhow::Result;
use either::Either;
use flowistry::{
  indexed::{
    impls::{LocationSet, PlaceIndex, PlaceSet},
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
  mir::{traversal, visit::Visitor, Location, Place},
  ty::TyCtxt,
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

    let location_domain = results.analysis.location_domain();
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

    let place_names = direct_places
      .iter()
      .filter_map(|(index, place)| {
        Some((index.as_usize(), PlaceDescriptor {
          place: format!("{place:?}"),
          name: place.to_string(tcx, body)?,
          local: place.local.as_usize(),
          projection: place
            .projection
            .iter()
            .map(|elem| format!("{elem:?}"))
            .collect(),
        }))
      })
      .collect::<HashMap<_, _>>();

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
        (index.as_usize(), s)
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
      let mut inputs = PlaceSet::new(place_domain);

      let mut visitor = ModularMutationVisitor::new(
        tcx,
        body,
        def_id.to_def_id(),
        |mutated: Place<'tcx>, _, _, _| {
          // all_mutated.insert(mutated);
        },
      );
      match body.stmt_at(location) {
        Either::Left(stmt) => {
          visitor.visit_statement(stmt, location);
        }
        Either::Right(terminator) => {
          visitor.visit_terminator(terminator, location);
        }
      };

      // let aliases = &results.analysis.aliases;
      // let all_mutated_conflicts = all_mutated
      //   .into_iter()
      //   .map(|place| aliases.conflicts(place).indices().collect::<Vec<_>>())
      //   .flatten()
      //   .filter(|place| place_names.contains_key(&place.as_usize()))
      //   .collect::<HashSet<_>>();

      // let state = results.state_at(location);
      // for place in all_mutated_conflicts.iter().copied() {
      //   let place_loc_deps = state.row_set(place).unwrap();
      //   let mut place_place_deps = all_deps
      //     .iter()
      //     .filter_map(|(other, other_deps)| {
      //       let (loc, _) = other_deps
      //         .iter()
      //         .filter(|(_, (other_loc_deps, _))| {
      //           place_loc_deps.is_superset(other_loc_deps)
      //         })
      //         .max_by_key(|(_, (other_loc_deps, _))| other_loc_deps.len())?;

      //       Some((*loc, *other))
      //     })
      //     .collect::<HashSet<_>>();

      //   let place_place_deps2 = place_place_deps.clone();
      //   place_place_deps.retain(|(loc1, place1)| {
      //     !place_place_deps2.iter().any(|(loc2, place2)| {
      //       let (deps1, _) = &all_deps[place1][loc1];
      //       let (deps2, _) = &all_deps[place2][loc2];
      //       deps2.len() > deps1.len() && deps2.is_superset(deps1)
      //     })
      //   });

      //   place_place_deps.extend(all_mutated_conflicts.iter().copied().filter_map(
      //     |other| {
      //       state.row_set(other).and_then(|other_loc_deps| {
      //         (place != other && place_loc_deps.is_superset(&other_loc_deps))
      //           .then(move || (location, other))
      //       })
      //     },
      //   ));

      //   all_deps
      //     .entry(place)
      //     .or_default()
      //     .insert(location, (place_loc_deps.to_owned(), place_place_deps));
      // }
    }

    // // from: Place -> Location -> {(Location, Place)}
    // // to: int -> int -> {(int, int)}
    // let place_deps = all_deps
    //   .into_iter()
    //   .map(|(index, deps)| {
    //     (
    //       index.as_usize(),
    //       deps
    //         .into_iter()
    //         .map(|(loc, (_, deps))| {
    //           (
    //             location_domain.index(&loc).as_usize(),
    //             deps
    //               .into_iter()
    //               .map(|(loc, index)| {
    //                 (location_domain.index(&loc).as_usize(), index.as_usize())
    //               })
    //               .collect(),
    //           )
    //         })
    //         .collect(),
    //     )
    //   })
    //   .collect();

    Ok(GraphOutput {
      place_names,
      location_names,
      place_deps: todo!(),
    })
  }
}

pub fn graph(
  id: FunctionIdentifier,
  compiler_args: &[String],
) -> FlowistryResult<GraphOutput> {
  GraphAnalysis { id }.run(compiler_args)
}
