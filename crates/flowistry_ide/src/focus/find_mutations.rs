use flowistry::{
  indexed::IndexMatrix, infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases,
};
use log::debug;
use rustc_hir::Mutability;
use rustc_middle::mir::{visit::Visitor, Body, Location, Place};

pub fn find_mutations(
  body: &Body<'tcx>,
  aliases: &Aliases<'_, 'tcx>,
) -> IndexMatrix<Place<'tcx>, Location> {
  let mut mutations = IndexMatrix::new(aliases.location_domain());

  ModularMutationVisitor::new(aliases, |mutated, inputs, location, _| {
    for (input, _) in inputs {
      for conflict in aliases.conflicts(input) {
        mutations.insert(*conflict, location);
      }
    }
    for conflict in aliases.conflicts(mutated) {
      mutations.insert(*conflict, location);
    }
  })
  .visit_body(body);

  mutations
}
