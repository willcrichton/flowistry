use rustc_middle::mir::{
  visit::{PlaceContext, Visitor},
  Body, Location, Place,
};

struct FindMutations<'tcx> {
  place: Place<'tcx>,
  locations: Vec<Location>,
}

impl Visitor<'tcx> for FindMutations<'tcx> {
  fn visit_place(
    &mut self,
    place: &Place<'tcx>,
    _context: PlaceContext,
    _location: Location,
  ) {
    if _context.is_mutating_use() && *place == self.place {
      self.locations.push(_location);
    }
  }
}

pub fn find_mutations(body: &Body<'tcx>, place: Place<'tcx>) -> Vec<Location> {
  let mut finder = FindMutations {
    place,
    locations: Vec::new(),
  };
  finder.visit_body(body);

  finder.locations
}
