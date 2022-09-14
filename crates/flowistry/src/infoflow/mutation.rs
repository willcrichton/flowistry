use log::debug;
use rustc_middle::mir::{visit::Visitor, *};

use crate::mir::{
  aliases::Aliases,
  utils::{self, OperandExt, PlaceCollector},
};

pub enum MutationStatus {
  Definitely,
  Possibly,
}

// Note: wcrichto tried making FnMut(...) a trait alias, but this
// interacted poorly with type inference and required ModularMutationVisitor
// clients to explicitly write out the type parameter of every closure argument.
pub struct ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(
    Place<'tcx>,
    &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
    Location,
    MutationStatus,
  ),
{
  f: F,
  aliases: &'a Aliases<'a, 'tcx>,
}

impl<'a, 'tcx, F> ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(
    Place<'tcx>,
    &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
    Location,
    MutationStatus,
  ),
{
  pub fn new(aliases: &'a Aliases<'a, 'tcx>, f: F) -> Self {
    ModularMutationVisitor { aliases, f }
  }
}

impl<'tcx, F> Visitor<'tcx> for ModularMutationVisitor<'_, 'tcx, F>
where
  F: FnMut(
    Place<'tcx>,
    &[(Place<'tcx>, Option<PlaceElem<'tcx>>)],
    Location,
    MutationStatus,
  ),
{
  fn visit_assign(
    &mut self,
    place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
    debug!("Checking {location:?}: {place:?} = {rvalue:?}");
    let mut collector = PlaceCollector {
      places: Vec::new(),
      tcx: self.aliases.tcx,
    };
    collector.visit_rvalue(rvalue, location);
    (self.f)(
      *place,
      &collector.places,
      location,
      MutationStatus::Definitely,
    );
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    debug!("Checking {location:?}: {:?}", terminator.kind);
    let tcx = self.aliases.tcx;

    match &terminator.kind {
      TerminatorKind::Call {
        /*func,*/ // TODO: deal with func
        args,
        destination,
        ..
      } => {
        let arg_places = utils::arg_places(args)
          .into_iter()
          .map(|(_, place)| place)
          .collect::<Vec<_>>();
        let arg_inputs = arg_places
          .iter()
          .map(|place| (*place, None))
          .collect::<Vec<_>>();

        let ret_is_unit = destination
          .ty(self.aliases.body.local_decls(), tcx)
          .ty
          .is_unit();
        let empty = vec![];
        let inputs = if ret_is_unit { &empty } else { &arg_inputs };

        (self.f)(*destination, inputs, location, MutationStatus::Definitely);

        for arg in arg_places {
          for arg_mut in self.aliases.reachable_values(arg, Mutability::Mut) {
            // The argument itself can never be modified in a caller-visible way,
            // because it's either getting moved or copied.
            if arg == *arg_mut {
              continue;
            }

            (self.f)(*arg_mut, &arg_inputs, location, MutationStatus::Possibly);
          }
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(src) = value.to_place() {
          (self.f)(*place, &[(src, None)], location, MutationStatus::Definitely);
        }
      }

      _ => {}
    }
  }
}
