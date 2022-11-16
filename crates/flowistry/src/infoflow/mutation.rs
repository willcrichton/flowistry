//! Identifies the mutated places in a MIR instruction via modular approximation based on types.

use log::debug;
use rustc_middle::mir::{visit::Visitor, *};

use crate::mir::{
  aliases::Aliases,
  utils::{self, OperandExt, PlaceCollector},
};

/// Indicator of certainty about whether a place is being mutated.
pub enum MutationStatus {
  /// A place is definitely mutated, e.g. `x = y` definitely mutates `x`.
  Definitely,

  /// A place is possibly mutated, e.g. `f(&mut x)` possibly mutates `x`.
  Possibly,
}

/// Information about a particular mutation.
pub struct Mutation<'a, 'tcx> {
  /// The place that is being mutated.
  pub mutated: Place<'tcx>,

  /// The set of inputs to the mutating operation, each paired with
  /// an optional [`ProjectionElem::Field`] in the case of aggregate constructors.
  pub inputs: &'a [(Place<'tcx>, Option<PlaceElem<'tcx>>)],

  /// Where the mutation is occuring.
  pub location: Location,

  /// The certainty of whether the mutation is happening.
  pub status: MutationStatus,
}

/// MIR visitor that invokes a callback for every [`Mutation`] in the visited object.
///
/// Construct the visitor with [`ModularMutationVisitor::new`], then call one of the
/// MIR [`Visitor`] methods.
pub struct ModularMutationVisitor<'a, 'tcx, F>
where
  // API design note: wcrichto tried making FnMut(...) a trait alias, but this
  // interacted poorly with type inference and required ModularMutationVisitor
  // clients to explicitly write out the type parameter of every closure argument.
  F: FnMut(Mutation<'_, 'tcx>),
{
  f: F,
  aliases: &'a Aliases<'a, 'tcx>,
}

impl<'a, 'tcx, F> ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(Mutation<'_, 'tcx>),
{
  pub fn new(aliases: &'a Aliases<'a, 'tcx>, f: F) -> Self {
    ModularMutationVisitor { aliases, f }
  }
}

impl<'tcx, F> Visitor<'tcx> for ModularMutationVisitor<'_, 'tcx, F>
where
  F: FnMut(Mutation<'_, 'tcx>),
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
    (self.f)(Mutation {
      mutated: *place,
      inputs: &collector.places,
      location,
      status: MutationStatus::Definitely,
    });
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

        (self.f)(Mutation {
          mutated: *destination,
          inputs,
          location,
          status: MutationStatus::Definitely,
        });

        for arg in arg_places {
          for arg_mut in self.aliases.reachable_values(arg, Mutability::Mut) {
            // The argument itself can never be modified in a caller-visible way,
            // because it's either getting moved or copied.
            if arg == *arg_mut {
              continue;
            }

            (self.f)(Mutation {
              mutated: *arg_mut,
              inputs: &arg_inputs,
              location,
              status: MutationStatus::Possibly,
            });
          }
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(src) = value.to_place() {
          (self.f)(Mutation {
            mutated: *place,
            inputs: &[(src, None)],
            location,
            status: MutationStatus::Definitely,
          });
        }
      }

      _ => {}
    }
  }
}
