use super::place_set::{PlaceDomain, PlaceIndex, PlaceSet};
use indexmap::map::{Entry, IndexMap};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHasher};
use rustc_index::{
  bit_set::{HybridBitSet, SparseBitMatrix},
  vec::IndexVec,
};
use rustc_middle::mir::*;
use rustc_mir::dataflow::JoinSemiLattice;
use std::borrow::Cow;
use std::hash::BuildHasherDefault;

rustc_index::newtype_index! {
    pub struct LocationIndex {
        DEBUG_FORMAT = "l{}"
    }
}

pub struct LocationDomain {
  index_to_loc: IndexVec<LocationIndex, Location>,
  loc_to_index: HashMap<Location, LocationIndex>,
}

impl LocationDomain {
  pub fn new(body: &Body) -> Self {
    let locations = body
      .basic_blocks()
      .iter_enumerated()
      .map(|(block, data)| {
        (0..data.statements.len() + 1).map(move |statement_index| Location {
          block,
          statement_index,
        })
      })
      .flatten()
      .collect::<Vec<_>>();
    let index_to_loc = IndexVec::from_raw(locations);
    let loc_to_index = index_to_loc
      .iter_enumerated()
      .map(|(idx, loc)| (*loc, idx))
      .collect();
    LocationDomain {
      index_to_loc,
      loc_to_index,
    }
  }

  pub fn index(&self, location: Location) -> LocationIndex {
    *self.loc_to_index.get(&location).unwrap()
  }

  pub fn location(&self, index: LocationIndex) -> Location {
    *self.index_to_loc.get(index).unwrap()
  }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RelevantLocations(IndexMap<LocationIndex, PlaceSet, BuildHasherDefault<FxHasher>>);

impl RelevantLocations {
  pub fn new(domain: &PlaceDomain) -> Self {
    RelevantLocations(IndexMap::default())
  }

  pub fn union(&mut self, location: LocationIndex, places: Cow<'_, PlaceSet>) -> bool {
    match self.0.entry(location) {
      Entry::Occupied(mut entry) => entry.get_mut().union(&*places),
      Entry::Vacant(mut entry) => {
        entry.insert(places.into_owned());
        true
      }
    }
  }

  pub fn iter<'a>(&'a self, domain: &'a LocationDomain) -> impl Iterator<Item = Location> + 'a {
    self.0.keys().map(move |index| domain.location(*index))
  }

  pub fn contains(&self, location: LocationIndex) -> bool {
    self.0.contains_key(&location)
  }

  pub fn get(&self, location: LocationIndex) -> Option<&PlaceSet> {
    self.0.get(&location)
  }
}

impl Clone for RelevantLocations {
  fn clone(&self) -> Self {
    RelevantLocations(self.0.clone())
  }

  fn clone_from(&mut self, other: &Self) {
    self.0.clone_from(&other.0);
  }
}

impl JoinSemiLattice for RelevantLocations {
  fn join(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for (k, v) in other.0.iter() {
      changed |= self.union(*k, Cow::Borrowed(v));
    }
    changed
  }
}

#[derive(PartialEq, Eq, Debug)]
pub struct RelevanceDomain {
  pub places: PlaceSet,
  pub locations: RelevantLocations,
}

impl RelevanceDomain {
  pub fn new(place_domain: &PlaceDomain) -> Self {
    let places = PlaceSet::new(place_domain);
    let locations = RelevantLocations::new(place_domain);
    RelevanceDomain { places, locations }
  }
}

impl Clone for RelevanceDomain {
  fn clone(&self) -> Self {
    RelevanceDomain {
      places: self.places.clone(),
      locations: self.locations.clone(),
    }
  }

  fn clone_from(&mut self, other: &Self) {
    self.places.clone_from(&other.places);
    self.locations.clone_from(&other.locations);
  }
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let b1 = self.places.join(&other.places);
    let b2 = self.locations.join(&other.locations);
    b1 || b2
  }
}
