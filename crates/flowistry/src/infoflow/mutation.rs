use std::iter;

use log::debug;
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{visit::Visitor, *},
  ty::TyCtxt,
};

use crate::mir::utils::{self, OperandExt, PlaceCollector, PlaceExt};

pub enum MutationStatus {
  Definitely,
  Possibly,
}

pub struct ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(Place<'tcx>, &[Place<'tcx>], Location, MutationStatus),
{
  f: F,
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  def_id: DefId,
}

impl<'a, 'tcx, F> ModularMutationVisitor<'a, 'tcx, F>
where
  F: FnMut(Place<'tcx>, &[Place<'tcx>], Location, MutationStatus),
{
  pub fn new(tcx: TyCtxt<'tcx>, body: &'a Body<'tcx>, def_id: DefId, f: F) -> Self {
    ModularMutationVisitor {
      tcx,
      body,
      def_id,
      f,
    }
  }
}

impl<'tcx, F> Visitor<'tcx> for ModularMutationVisitor<'_, 'tcx, F>
where
  F: FnMut(Place<'tcx>, &[Place<'tcx>], Location, MutationStatus),
{
  fn visit_assign(
    &mut self,
    place: &Place<'tcx>,
    rvalue: &Rvalue<'tcx>,
    location: Location,
  ) {
    debug!("Checking {:?}: {:?} = {:?}", location, place, rvalue);
    let mut collector = PlaceCollector::default();
    collector.visit_rvalue(rvalue, location);
    (self.f)(
      *place,
      &collector.places,
      location,
      MutationStatus::Definitely,
    );
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    debug!("Checking {:?}: {:?}", location, terminator.kind);
    let tcx = self.tcx;

    match &terminator.kind {
      TerminatorKind::Call {
        /*func,*/ // TODO: deal with func
        args,
        destination,
        ..
      } => {
        let inputs_for_arg = |arg: Place<'tcx>| {
          arg
            .interior_pointers(tcx, self.body, self.def_id)
            .into_values()
            .map(|places| {
              places
                .into_iter()
                .map(|(place, _)| tcx.mk_place_deref(place))
            })
            .flatten()
            .chain(iter::once(arg))
        };

        let arg_places = utils::arg_places(args);
        let arg_inputs = arg_places
          .iter()
          .map(|(_, arg)| inputs_for_arg(*arg))
          .flatten()
          .collect::<Vec<_>>();

        if let Some((dst_place, _)) = destination {
          let ret_is_unit = dst_place.ty(self.body.local_decls(), tcx).ty.is_unit();
          let empty = vec![];
          let inputs = if ret_is_unit { &empty } else { &arg_inputs };

          (self.f)(*dst_place, inputs, location, MutationStatus::Definitely);
        }

        for (_, mut_ptr) in utils::arg_mut_ptrs(&arg_places, tcx, self.body, self.def_id)
        {
          (self.f)(mut_ptr, &arg_inputs, location, MutationStatus::Possibly);
        }
      }

      TerminatorKind::DropAndReplace { place, value, .. } => {
        if let Some(src) = value.to_place() {
          (self.f)(*place, &[src], location, MutationStatus::Definitely);
        }
      }

      _ => {}
    }
  }
}
