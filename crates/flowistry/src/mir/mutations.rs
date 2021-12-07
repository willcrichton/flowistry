use rustc_hir::Mutability;
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    Body, HasLocalDecls, Location, Place,
  },
  ty::TyCtxt,
};

use super::aliases::Aliases;

struct FindMutations<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  place: Place<'tcx>,
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
      }) && *place == self.place
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
  let mut finder = FindMutations {
    tcx,
    body,
    place,
    aliases,
    locations: Vec::new(),
  };
  finder.visit_body(body);

  finder.locations
}
