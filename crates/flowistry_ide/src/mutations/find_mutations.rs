use flowistry::{
  indexed::IndexSetIteratorExt,
  infoflow::mutation::ModularMutationVisitor,
  mir::{aliases::Aliases, utils::PlaceExt},
};
use log::debug;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{visit::Visitor, Body, Location, Place},
  ty::TyCtxt,
};

pub fn find_mutations(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  def_id: DefId,
  place: Place<'tcx>,
  aliases: Aliases<'tcx>,
) -> Vec<Location> {
  let mut locations = vec![];
  let pointer_aliases = place
    .interior_pointers(tcx, body, def_id)
    .into_values()
    .map(|v| v.into_iter().map(|(place, _)| place))
    .flatten()
    .map(|place| aliases.aliases.row(tcx.mk_place_deref(place)).copied())
    .flatten()
    .chain(vec![place])
    .collect_indices(aliases.place_domain.clone());

  debug!("pointer aliases: {:?}", pointer_aliases);

  ModularMutationVisitor::new(
    tcx,
    body,
    def_id,
    |mutated_place, _, mutated_location, _| {
      debug!("checking mutated location {:?}", mutated_location);

      let mut place_conflicts = aliases.conflicts(mutated_place);
      place_conflicts.intersect(&pointer_aliases);

      if place_conflicts.len() > 0 {
        debug!("  found conflicts: {:?}", place_conflicts);
        locations.push(mutated_location);
      }
    },
  )
  .visit_body(body);

  locations
}
