use rustc_hir::{def_id::DefId, Mutability};
use rustc_middle::{
  mir::{
    visit::{PlaceContext, Visitor},
    Body, HasLocalDecls, Location, Place
  },
  ty::TyCtxt,
};

use super::aliases::Aliases;
use crate::{infoflow::mutation::ModularMutationVisitor};

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
  def_id: DefId,
  place: Place<'tcx>,
  aliases: Aliases<'tcx>,
) -> Vec<Location> {
  let mut places = vec![];

  ModularMutationVisitor::new(tcx, body, def_id, |visitor_place, inputs, _, _| {
    if place == visitor_place {
      places = if inputs.is_empty() {
        vec![place]
      } else {
        inputs
      }
    }
  })
  .visit_body(body);

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
