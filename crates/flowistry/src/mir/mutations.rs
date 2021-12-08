use std::collections::HashMap;

use rustc_hir::Mutability;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    Body, HasLocalDecls, Location, Place, Rvalue,
  },
  ty::TyCtxt,
};

use super::aliases::Aliases;
use crate::mir::utils::PlaceCollector;

struct GatherInputs<'tcx> {
  inputs: HashMap<Place<'tcx>, Vec<Place<'tcx>>>,
}

impl Visitor<'tcx> for GatherInputs<'tcx> {
  fn visit_assign(
    &mut self,
    place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    
    if collector.places.is_empty() {
      collector.places.push(*place);
    }

    self.inputs.insert(*place, collector.places);
  }
}

struct FindMutations<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  places: Vec<Place<'tcx>>,
  aliases: Aliases<'tcx>,
  locations: Vec<Location>,
}

impl Visitor<'tcx> for FindMutations<'a, 'tcx> {
  fn visit_place(
    &mut self,
    place: &Place<'tcx>,
    _context: PlaceContext,
    _location: Location,
  ) {
    let all_conflicts = self.aliases.conflicts(place);
    let place_has_conflicts = all_conflicts.iter().any(|place| {
      place.iter_projections().all(|(sub_place, _)| {
        let ty = sub_place.ty(self.body.local_decls(), self.tcx).ty;
        !matches!(ty.ref_mutability(), Some(Mutability::Not))
      }) && self.places.contains(place)
    });

    if _context.is_mutating_use() && place_has_conflicts {
      self.locations.push(_location);
    }
  }
}

pub fn find_mutations(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  place: Place<'tcx>,
  aliases: Aliases<'tcx>,
) -> Vec<Location> {
  let mut input_gatherer = GatherInputs {
    inputs: HashMap::new(),
  };
  input_gatherer.visit_body(body);

  let places = input_gatherer
    .inputs
    .get(&place)
    .cloned()
    .unwrap_or_default();

  let mut finder = FindMutations {
    tcx,
    body,
    places,
    aliases,
    locations: Vec::new(),
  };
  finder.visit_body(body);

  finder.locations
}
