use flowistry::{
  indexed::IndexMatrix, infoflow::mutation::ModularMutationVisitor, mir::aliases::Aliases,
};
use rustc_middle::mir::{
  visit::Visitor, Body, HasLocalDecls, Location, Mutability, Place,
};

pub struct DirectInfluence<'a, 'tcx> {
  aliases: &'a Aliases<'a, 'tcx>,
  influence: IndexMatrix<Place<'tcx>, Location>,
}

impl<'a, 'tcx> DirectInfluence<'a, 'tcx> {
  pub fn build(body: &Body<'tcx>, aliases: &'a Aliases<'a, 'tcx>) -> Self {
    let mut influence = IndexMatrix::new(aliases.location_domain());
    let tcx = aliases.tcx;

    ModularMutationVisitor::new(aliases, |mutated, inputs, location, _| {
      let mut add = |place: Place<'tcx>| {
        let mut root_place = place;
        let mut steps = 0;
        let mut ty = place.ty(body.local_decls(), tcx).ty;
        loop {
          match ty.builtin_deref(false) {
            Some(deref) => {
              ty = deref.ty;
              root_place = tcx.mk_place_deref(root_place);

              steps += 1;
              if steps > 10 {
                break;
              }
            }
            None => {
              break;
            }
          }
        }

        for alias in aliases.aliases(root_place) {
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
