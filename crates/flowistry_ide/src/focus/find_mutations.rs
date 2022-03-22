use flowistry::{
  indexed::IndexMatrix, infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases,
};
use rustc_middle::mir::{visit::Visitor, Body, Location, Mutability, Place};

// TODO: change the name, this isn't just mutations. More like "find_direct_flows"
pub fn find_mutations(
  body: &Body<'tcx>,
  aliases: &Aliases<'_, 'tcx>,
) -> IndexMatrix<Place<'tcx>, Location> {
  let mut mutations = IndexMatrix::new(aliases.location_domain());

  ModularMutationVisitor::new(aliases, |mutated, inputs, location, _| {
    let mut add = |place: Place<'tcx>, mutability: Mutability| {
      for reachable in aliases.reachable_values(place, mutability) {
        for conflict in aliases.conflicts(*reachable) {
          mutations.insert(*conflict, location);
        }
      }
    };

    for (input, _) in inputs {
      add(*input, Mutability::Not);
    }

    add(mutated, Mutability::Mut);
  })
  .visit_body(body);

  mutations
}
