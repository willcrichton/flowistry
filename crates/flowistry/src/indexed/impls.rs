use std::{mem, rc::Rc};

use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::vec::IndexVec;
use rustc_middle::{
  mir::{BasicBlock, Body, Location, Place},
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, OwnedSet, ToIndex};
use crate::{
  mir::utils::{BodyExt, PlaceExt},
  to_index_impl,
};

rustc_index::newtype_index! {
  pub struct LocationIndex {
      DEBUG_FORMAT = "l{}"
  }
}

to_index_impl!(Location);

impl IndexedValue for Location {
  type Index = LocationIndex;
  type Domain = LocationDomain;
}

pub type LocationSet<S = OwnedSet<Location>> = IndexSet<Location, S>;
pub struct LocationDomain {
  domain: DefaultDomain<LocationIndex, Location>,
  arg_to_location: HashMap<Place<'static>, LocationIndex>,
  location_to_arg: HashMap<LocationIndex, Place<'static>>,
}

impl LocationDomain {
  pub fn new(body: &Body<'tcx>, tcx: TyCtxt<'tcx>, def_id: DefId) -> Rc<Self> {
    let mut locations = body.all_locations().collect::<Vec<_>>();

    let arg_block = BasicBlock::from_usize(body.basic_blocks().len());

    // TODO: the shallow interior_pointers was designed to avoid blowing up
    // the size of the location domain if there's a ton of reachable pointers
    // from the arguments, e.g. see
    //   rust/compiler/rustc_typeck/src/check/op.rs 21361
    // for a stress test. But not sure if this is sound yet.
    let mut arg_places = body
      .args_iter()
      .flat_map(|local| {
        let place = Place::from_local(local, tcx);
        let ptrs = place
          .interior_pointers(tcx, body, def_id, true)
          .into_values()
          .flat_map(|ptrs| ptrs.into_iter().map(|(ptr, _)| tcx.mk_place_deref(ptr)));
        ptrs
          .chain([place])
          .flat_map(|place| place.interior_places(tcx, body, def_id))
      })
      .map(|place| unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(place) })
      .collect::<Vec<_>>();
    arg_places.dedup();

    let arg_locations = (0 .. arg_places.len())
      .map(|i| Location {
        block: arg_block,
        statement_index: i,
      })
      .collect::<Vec<_>>();

    log::debug!("Arg places: {arg_places:?}");
    log::info!(
      "Location domain size: {} real, {} args",
      locations.len(),
      arg_places.len()
    );

    locations.extend(&arg_locations);

    let domain = DefaultDomain::new(locations);

    let arg_to_location = arg_places
      .into_iter()
      .zip(arg_locations.iter().map(|loc| domain.index(loc)))
      .collect::<HashMap<_, _>>();

    let location_to_arg = arg_to_location
      .iter()
      .map(|(k, v)| (*v, *k))
      .collect::<HashMap<_, _>>();

    Rc::new(LocationDomain {
      domain,
      arg_to_location,
      location_to_arg,
    })
  }

  pub fn num_real_locations(&self) -> usize {
    self.domain.size() - self.arg_to_location.len()
  }

  pub fn all_args(&'a self) -> impl Iterator<Item = (Place<'tcx>, LocationIndex)> + 'a {
    self.arg_to_location.iter().map(|(arg, loc)| {
      let arg = unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(*arg) };
      (arg, *loc)
    })
  }

  pub fn arg_to_location(&self, arg: Place<'tcx>) -> LocationIndex {
    let arg = unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(arg) };
    *self.arg_to_location.get(&arg).unwrap()
  }

  pub fn location_to_arg(&self, location: impl ToIndex<Location>) -> Option<Place<'tcx>> {
    let arg = self
      .location_to_arg
      .get(&location.to_index(self))
      .copied()?;
    Some(unsafe { mem::transmute::<Place<'static>, Place<'tcx>>(arg) })
  }
}

impl IndexedDomain for LocationDomain {
  type Index = LocationIndex;
  type Value = Location;

  fn value(&self, index: Self::Index) -> &Self::Value {
    self.domain.value(index)
  }

  fn index(&self, value: &Self::Value) -> Self::Index {
    self.domain.index(value)
  }

  fn contains(&self, value: &Self::Value) -> bool {
    self.domain.contains(value)
  }

  fn as_vec(&self) -> &IndexVec<Self::Index, Self::Value> {
    self.domain.as_vec()
  }
}

pub type PlaceSet<'tcx> = HashSet<Place<'tcx>>;
