use flowistry::{
  indexed::IndexMatrix, infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases,
};
use rustc_middle::mir::{visit::Visitor, Body, Location, Mutability, Place};

pub struct DirectInfluence<'a, 'tcx> {
  aliases: &'a Aliases<'a, 'tcx>,
  influence: IndexMatrix<Place<'tcx>, Location>,
}

impl<'a, 'tcx> DirectInfluence<'a, 'tcx> {
  pub fn build(body: &Body<'tcx>, aliases: &'a Aliases<'a, 'tcx>) -> Self {
    let mut influence = IndexMatrix::new(aliases.location_domain());

    ModularMutationVisitor::new(aliases, |mutated, inputs, location, _| {
      let mut add = |place: Place<'tcx>, mutability: Mutability| {
        for alias in aliases.reachable_values(place, mutability) {
          influence.insert(*alias, location);
        }
      };

      for (input, _) in inputs {
        add(*input, Mutability::Not);
      }

      add(mutated, Mutability::Mut);
    })
    .visit_body(body);

    DirectInfluence { aliases, influence }
  }

  pub fn lookup(&self, target: Place<'tcx>) -> Vec<Location> {
    let aliases = self.aliases.reachable_values(target, Mutability::Not);
    aliases
      .iter()
      .flat_map(|target_alias| {
        self
          .influence
          .row_set(*target_alias)
          .iter()
          .copied()
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>()
  }
}
