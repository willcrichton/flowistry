//! Identifies the mutated places in a MIR instruction via modular approximation based on types.

use log::debug;
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyKind,
};
use rustc_target::abi::FieldIdx;
use rustc_utils::OperandExt;

use crate::mir::{
  aliases::Aliases,
  utils::{self, AsyncHack, PlaceCollector},
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

  /// The set of inputs to the mutating operation.
  pub inputs: &'a [Place<'tcx>],

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
    mutated: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
    debug!("Checking {location:?}: {mutated:?} = {rvalue:?}");
    let body = self.aliases.body;
    let tcx = self.aliases.tcx;

    match rvalue {
      // In the case of _1 = aggregate { field1: op1, field2: op2, ... },
      // then destructure this into a series of mutations like
      // _1.field1 = op1, _1.field2 = op2, and so on.
      Rvalue::Aggregate(box AggregateKind::Adt(def_id, idx, substs, _, _), ops) => {
        let adt_def = tcx.adt_def(*def_id);
        let variant = adt_def.variant(*idx);
        if variant.fields.len() > 0 {
          let fields = variant.fields.iter().enumerate().zip(ops.iter()).map(
            |((i, field), input_op)| {
              let input_place = input_op.as_place();
              let field =
                PlaceElem::Field(FieldIdx::from_usize(i), field.ty(tcx, substs));
              (mutated.project_deeper(&[field], tcx), input_place)
            },
          );
          for (mutated, input) in fields {
            (self.f)(Mutation {
              mutated,
              inputs: input.as_ref().map(std::slice::from_ref).unwrap_or_default(),
              location,
              status: MutationStatus::Definitely,
            });
          }
          return;
        }
      }

      // In the case of _1 = _2 where _2 : struct Foo { x: T, y: S, .. },
      // then destructure this into a series of mutations like
      // _1.x = _2.x, _1.y = _2.y, and so on.
      Rvalue::Use(Operand::Move(place) | Operand::Copy(place)) => {
        let place_ty = place.ty(&body.local_decls, tcx).ty;
        if let TyKind::Adt(adt_def, substs) = place_ty.kind() {
          if adt_def.is_struct() {
            let fields = adt_def.all_fields().enumerate().map(|(i, field_def)| {
              PlaceElem::Field(FieldIdx::from_usize(i), field_def.ty(tcx, substs))
            });
            for field in fields {
              let mutated_field = mutated.project_deeper(&[field], tcx);
              let input_field = place.project_deeper(&[field], tcx);
              (self.f)(Mutation {
                mutated: mutated_field,
                inputs: &[input_field],
                location,
                status: MutationStatus::Definitely,
              });
            }
            return;
          }
        }
      }

      _ => {}
    }

    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    (self.f)(Mutation {
      mutated: *mutated,
      inputs: &collector.0,
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
        let async_hack =
          AsyncHack::new(self.aliases.tcx, self.aliases.body, self.aliases.def_id);
        let arg_places = utils::arg_places(args)
          .into_iter()
          .map(|(_, place)| place)
          .filter(|place| !async_hack.ignore_place(*place))
          .collect::<Vec<_>>();
        let arg_inputs = arg_places.clone();

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

      _ => {}
    }
  }
}
