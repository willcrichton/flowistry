use std::{mem, rc::Rc};

use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::vec::IndexVec;
use rustc_middle::{
  mir::{BasicBlock, Body, Location, Place},
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;

use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, ToIndex};
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

pub type LocationSet = IndexSet<Location>;
pub struct LocationDomain {
  domain: DefaultDomain<LocationIndex, Location>,
  arg_to_location: HashMap<Place<'static>, LocationIndex>,
  location_to_arg: HashMap<LocationIndex, Place<'static>>,
}

impl LocationDomain {
  pub fn new(body: &Body<'tcx>, tcx: TyCtxt<'tcx>, def_id: DefId) -> Rc<Self> {
    let mut locations = body.all_locations().collect::<Vec<_>>();

    let arg_block = BasicBlock::from_usize(body.basic_blocks().len());

    let (arg_places, arg_locations): (Vec<_>, Vec<_>) = body
      .args_iter()
      .flat_map(|local| {
        let place = Place::from_local(local, tcx);
        let ptrs = place
          .interior_pointers(tcx, body, def_id)
          .into_values()
          .flat_map(|ptrs| ptrs.into_iter().map(|(ptr, _)| ptr));
        ptrs
          .chain([place])
          .flat_map(|place| place.interior_places(tcx, body, def_id))
      })
      .enumerate()
      .map(|(i, place)| {
        (
          unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(place) },
          Location {
            block: arg_block,
            statement_index: i,
          },
        )
      })
      .unzip();

    log::info!(
      "Location domain size: {} real, {} args",
      locations.len(),
      arg_places.len()
    );

    locations.extend(&arg_locations);

    let domain = DefaultDomain::new(locations);

    let arg_to_location = arg_places
      .iter()
      .copied()
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
