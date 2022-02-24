use std::rc::Rc;

use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_index::vec::IndexVec;
use rustc_middle::mir::{BasicBlock, Body, Local, Location, Place};

use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, OwnedSet, ToIndex};
use crate::{mir::utils::BodyExt, to_index_impl};

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
  arg_block: BasicBlock,
  real_locations: usize,
  // arg_to_location: HashMap<Place<'static>, LocationIndex>,
  // location_to_arg: HashMap<LocationIndex, Place<'static>>,
}

impl LocationDomain {
  pub fn new(body: &Body<'tcx>) -> Rc<Self> {
    let mut locations = body.all_locations().collect::<Vec<_>>();

    let arg_block = BasicBlock::from_usize(body.basic_blocks().len());

    // let mut arg_places = body
    //   .args_iter()
    //   .flat_map(|local| {
    //     let place = Place::from_local(local, tcx);
    //     let ptrs = place
    //       .interior_pointers(tcx, body, def_id)
    //       .into_values()
    //       .flat_map(|ptrs| ptrs.into_iter().map(|(ptr, _)| tcx.mk_place_deref(ptr)));
    //     ptrs
    //       .chain([place])
    //       .flat_map(|place| place.interior_places(tcx, body, def_id))
    //   })
    //   .map(|place| unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(place) })
    //   .collect::<Vec<_>>();
    // arg_places.dedup();

    // let arg_locations = (0 .. arg_places.len())
    //   .map(|i| Location {
    //     block: arg_block,
    //     statement_index: i,
    //   })
    //   .collect::<Vec<_>>();

    let real_locations = locations.len();

    let arg_locations = (0 .. body.arg_count).map(|i| Location {
      block: arg_block,
      statement_index: i + 1,
    });
    locations.extend(arg_locations);

    let domain = DefaultDomain::new(locations);

    // let arg_to_location = arg_places
    //   .into_iter()
    //   .zip(arg_locations.iter().map(|loc| domain.index(loc)))
    //   .collect::<HashMap<_, _>>();

    // let location_to_arg = arg_to_location
    //   .iter()
    //   .map(|(k, v)| (*v, *k))
    //   .collect::<HashMap<_, _>>();

    Rc::new(LocationDomain {
      domain,
      arg_block,
      real_locations
      // arg_to_location,
      // location_to_arg,
    })
  }

  pub fn num_real_locations(&self) -> usize {
    self.real_locations
  }

  // pub fn all_args(&'a self) -> impl Iterator<Item = (Place<'tcx>, LocationIndex)> + 'a {
  //   self.arg_to_location.iter().map(|(arg, loc)| {
  //     let arg = unsafe { mem::transmute::<Place<'tcx>, Place<'static>>(*arg) };
  //     (arg, *loc)
  //   })
  // }

  pub fn arg_to_location(&self, local: Local) -> LocationIndex {
    let location = Location {
      block: self.arg_block,
      statement_index: local.as_usize(),
    };
    self.index(&location)
  }

  pub fn location_to_local(&self, location: impl ToIndex<Location>) -> Option<Local> {
    let location = self.value(location.to_index(self));
    (location.block == self.arg_block)
      .then(|| Local::from_usize(location.statement_index))
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
