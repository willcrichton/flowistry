//! Identifies the mutated places in a MIR instruction via modular approximation based on types.

use log::debug;
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::{AdtKind, TyKind},
};
use rustc_target::abi::FieldIdx;
use rustc_utils::{mir::place::PlaceCollector, AdtDefExt, OperandExt};

use crate::mir::{
  placeinfo::PlaceInfo,
  utils::{self, AsyncHack},
};

/// Indicator of certainty about whether a place is being mutated.
/// Used to determine whether an update should be strong or weak.
#[derive(Debug)]
pub enum MutationStatus {
  /// A place is definitely mutated, e.g. `x = y` definitely mutates `x`.
  Definitely,

  /// A place is possibly mutated, e.g. `f(&mut x)` possibly mutates `x`.
  Possibly,
}

/// Information about a particular mutation.
#[derive(Debug)]
pub struct Mutation<'tcx> {
  /// The place that is being mutated.
  pub mutated: Place<'tcx>,

  /// The set of inputs to the mutating operation.
  pub inputs: Vec<Place<'tcx>>,

  /// The certainty of whether the mutation is happening.
  pub status: MutationStatus,
}

/// MIR visitor that invokes a callback for every [`Mutation`] in the visited object.
///
/// Construct the visitor with [`ModularMutationVisitor::new`], then call one of the
/// MIR [`Visitor`] methods.
pub struct ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(Location, Vec<Mutation<'tcx>>),
{
  f: F,
  place_info: &'a PlaceInfo<'a, 'tcx>,
}

impl<'a, 'tcx, F> ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(Location, Vec<Mutation<'tcx>>),
{
  /// Constructs a new visitor.
  pub fn new(place_info: &'a PlaceInfo<'a, 'tcx>, f: F) -> Self {
    ModularMutationVisitor { place_info, f }
  }
}

impl<'tcx, F> Visitor<'tcx> for ModularMutationVisitor<'_, 'tcx, F>
where
  F: FnMut(Location, Vec<Mutation<'tcx>>),
{
  fn visit_assign(
    &mut self,
    mutated: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
    debug!("Checking {location:?}: {mutated:?} = {rvalue:?}");
    let body = self.place_info.body;
    let tcx = self.place_info.tcx;

    match rvalue {
      // In the case of _1 = aggregate { field1: op1, field2: op2, ... },
      // then destructure this into a series of mutations like
      // _1.field1 = op1, _1.field2 = op2, and so on.
      Rvalue::Aggregate(agg_kind, ops) => {
        let info = match &**agg_kind {
          AggregateKind::Adt(def_id, idx, substs, _, _) => {
            let adt_def = tcx.adt_def(*def_id);
            let variant = adt_def.variant(*idx);
            let mutated = match adt_def.adt_kind() {
              AdtKind::Enum => mutated.project_deeper(
                &[ProjectionElem::Downcast(Some(variant.name), *idx)],
                tcx,
              ),
              AdtKind::Struct | AdtKind::Union => *mutated,
            };
            let fields = variant.fields.iter();
            let tys = fields
              .map(|field| field.ty(tcx, substs))
              .collect::<Vec<_>>();
            Some((mutated, tys))
          }
          AggregateKind::Tuple => {
            let ty = rvalue.ty(body.local_decls(), tcx);
            Some((*mutated, ty.tuple_fields().to_vec()))
          }
          _ => None,
        };

        if let Some((mutated, tys)) = info {
          if tys.len() > 0 {
            let fields =
              tys
                .into_iter()
                .enumerate()
                .zip(ops.iter())
                .map(|((i, ty), input_op)| {
                  let field = PlaceElem::Field(FieldIdx::from_usize(i), ty);
                  let input_place = input_op.as_place();
                  (mutated.project_deeper(&[field], tcx), input_place)
                });

            let mutations = fields
              .map(|(mutated, input)| Mutation {
                mutated,
                inputs: input.into_iter().collect::<Vec<_>>(),
                status: MutationStatus::Definitely,
              })
              .collect::<Vec<_>>();
            (self.f)(location, mutations);
            return;
          }
        }
      }

      // In the case of _1 = _2 where _2 : struct Foo { x: T, y: S, .. },
      // then destructure this into a series of mutations like
      // _1.x = _2.x, _1.y = _2.y, and so on.
      Rvalue::Use(Operand::Move(place) | Operand::Copy(place)) => {
        let place_ty = place.ty(&body.local_decls, tcx).ty;
        if let TyKind::Adt(adt_def, substs) = place_ty.kind() {
          if adt_def.is_struct() {
            let fields = adt_def
              .all_visible_fields(self.place_info.def_id, self.place_info.tcx)
              .enumerate()
              .map(|(i, field_def)| {
                PlaceElem::Field(FieldIdx::from_usize(i), field_def.ty(tcx, substs))
              });
            let mut mutations = fields
              .map(|field| {
                let mutated_field = mutated.project_deeper(&[field], tcx);
                let input_field = place.project_deeper(&[field], tcx);
                Mutation {
                  mutated: mutated_field,
                  inputs: vec![input_field],
                  status: MutationStatus::Definitely,
                }
              })
              .collect::<Vec<_>>();

            if mutations.is_empty() {
              mutations.push(Mutation {
                mutated: *mutated,
                inputs: vec![*place],
                status: MutationStatus::Definitely,
              });
            }
            (self.f)(location, mutations);
            return;
          }
        }
      }

      _ => {}
    }

    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    (self.f)(location, vec![Mutation {
      mutated: *mutated,
      inputs: collector.0,
      status: MutationStatus::Definitely,
    }]);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    debug!("Checking {location:?}: {:?}", terminator.kind);
    let tcx = self.place_info.tcx;

    match &terminator.kind {
      TerminatorKind::Call {
        /*func,*/ // TODO: deal with func
        args,
        destination,
        ..
      } => {
        let async_hack = AsyncHack::new(
          self.place_info.tcx,
          self.place_info.body,
          self.place_info.def_id,
        );
        let arg_places = utils::arg_places(args)
          .into_iter()
          .map(|(_, place)| place)
          .filter(|place| !async_hack.ignore_place(*place))
          .collect::<Vec<_>>();
        let arg_inputs = arg_places.clone();

        let ret_is_unit = destination
          .ty(self.place_info.body.local_decls(), tcx)
          .ty
          .is_unit();
        let inputs = if ret_is_unit {
          Vec::new()
        } else {
          arg_inputs.clone()
        };

        let mut mutations = vec![Mutation {
          mutated: *destination,
          inputs,
          status: MutationStatus::Definitely,
        }];

        for arg in arg_places {
          for arg_mut in self.place_info.reachable_values(arg, Mutability::Mut) {
            mutations.push(Mutation {
              mutated: *arg_mut,
              inputs: arg_inputs.clone(),
              status: MutationStatus::Possibly,
            });
          }
        }

        (self.f)(location, mutations);
      }

      _ => {}
    }
  }
}
