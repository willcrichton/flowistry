use flowistry::{infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases};
use log::debug;
use rustc_hir::Mutability;
use rustc_middle::mir::{visit::Visitor, Body, Location, Place};

#[allow(dead_code)]
pub fn find_mutations(
  body: &Body<'tcx>,
  place: Place<'tcx>,
  aliases: &Aliases<'_, 'tcx>,
) -> Vec<Location> {
  let mut locations = vec![];
  let reachable_values = aliases.reachable_values(place, Mutability::Mut);
  debug!("reachable values: {reachable_values:?}");

  ModularMutationVisitor::new(aliases, |mutated_place, _, mutated_location, _| {
    debug!("checking mutated location {mutated_location:?}");

    let place_conflicts = aliases.conflicts(mutated_place).to_owned();
    if place_conflicts.iter().any(|v| reachable_values.contains(v)) {
      debug!("  found conflicts: {place_conflicts:?}");
      locations.push(mutated_location);
    }
  })
  .visit_body(body);

  return locations;
}
