use flowistry::{
  indexed::IndexMatrix, infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases,
};
use rustc_middle::mir::{visit::Visitor, Body, Location, Place};

pub struct DirectInfluence<'a, 'tcx> {
  aliases: &'a Aliases<'a, 'tcx>,
  influence: IndexMatrix<Place<'tcx>, Location>,
}

impl DirectInfluence<'a, 'tcx> {
  pub fn build(body: &Body<'tcx>, aliases: &'a Aliases<'a, 'tcx>) -> Self {
    let mut influence = IndexMatrix::new(aliases.location_domain());

    ModularMutationVisitor::new(aliases, |mutated, inputs, location, _| {
      let mut add = |place: Place<'tcx>| {
        for alias in aliases.aliases(place) {
          influence.insert(*alias, location);
        }
      };

      for (input, _) in inputs {
        add(*input);
      }

      add(mutated);
    })
    .visit_body(body);

    DirectInfluence { aliases, influence }
  }

  pub fn lookup(&self, target: Place<'tcx>) -> Vec<Location> {
    let aliases = self.aliases.aliases(target);
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
