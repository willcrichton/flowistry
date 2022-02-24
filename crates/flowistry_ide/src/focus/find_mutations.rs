use flowistry::{infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases};
use log::debug;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{visit::Visitor, Body, Location, Place},
  ty::TyCtxt,
};

#[allow(dead_code)]
pub fn find_mutations(
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
  def_id: DefId,
  place: Place<'tcx>,
  aliases: &Aliases<'_, 'tcx>,
) -> Vec<Location> {
  let mut locations = vec![];
  let reachable_values = aliases.reachable_values(place);
  debug!("reachable values: {reachable_values:?}");

  ModularMutationVisitor::new(
    tcx,
    body,
    def_id,
    |mutated_place, _, mutated_location, _| {
      debug!("checking mutated location {mutated_location:?}");

      let place_conflicts = aliases.conflicts(mutated_place).to_owned();
      if place_conflicts.iter().any(|v| reachable_values.contains(v)) {
        debug!("  found conflicts: {place_conflicts:?}");
        locations.push(mutated_location);
      }
    },
  )
  .visit_body(body);

  return locations;
}
