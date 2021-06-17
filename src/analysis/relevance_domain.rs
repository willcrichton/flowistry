use super::place_set::{PlaceDomain, PlaceSet};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::mir::*;
use rustc_mir::dataflow::JoinSemiLattice;

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

  pub fn len(&self) -> usize {
    self.loc_to_index.len()
  }
}

// #[derive(Debug, PartialEq, Eq)]
// pub struct RelevantLocations(IndexMap<LocationIndex, PlaceSet, BuildHasherDefault<FxHasher>>);

// impl RelevantLocations {
//   pub fn new(domain: &PlaceDomain) -> Self {
//     RelevantLocations(IndexMap::default())
//   }

//   pub fn union(&mut self, location: LocationIndex, places: Cow<'_, PlaceSet>) -> bool {
//     match self.0.entry(location) {
//       Entry::Occupied(mut entry) => entry.get_mut().union(&*places),
//       Entry::Vacant(mut entry) => {
//         entry.insert(places.into_owned());
//         true
//       }
//     }
//   }

//   pub fn iter<'a>(&'a self, domain: &'a LocationDomain) -> impl Iterator<Item = Location> + 'a {
//     self.0.keys().map(move |index| domain.location(*index))
//   }

//   pub fn contains(&self, location: LocationIndex) -> bool {
//     self.0.contains_key(&location)
//   }

//   pub fn get(&self, location: LocationIndex) -> Option<&PlaceSet> {
//     self.0.get(&location)
//   }
// }

// impl Clone for RelevantLocations {
//   fn clone(&self) -> Self {
//     RelevantLocations(self.0.clone())
//   }

//   fn clone_from(&mut self, other: &Self) {
//     self.0.clone_from(&other.0);
//   }
// }

// impl JoinSemiLattice for RelevantLocations {
//   fn join(&mut self, other: &Self) -> bool {
//     let mut changed = false;
//     for (k, v) in other.0.iter() {
//       changed |= self.union(*k, Cow::Borrowed(v));
//     }
//     changed
//   }
// }

pub type LocationSet = BitSet<LocationIndex>;

#[derive(PartialEq, Eq, Debug)]
pub struct RelevanceDomain {
  pub places: PlaceSet,
  pub mutated: PlaceSet,
  pub locations: LocationSet,
}

impl RelevanceDomain {
  pub fn new(place_domain: &PlaceDomain, location_domain: &LocationDomain) -> Self {
    let places = PlaceSet::new(place_domain);
    let mutated = PlaceSet::new(place_domain);
    let locations = BitSet::new_empty(location_domain.len());
    RelevanceDomain {
      places,
      mutated,
      locations,
    }
  }
}

impl Clone for RelevanceDomain {
  fn clone(&self) -> Self {
    RelevanceDomain {
      places: self.places.clone(),
      mutated: self.mutated.clone(),
      locations: self.locations.clone(),
    }
  }

  fn clone_from(&mut self, other: &Self) {
    self.places.clone_from(&other.places);
    self.mutated.clone_from(&other.mutated);
    self.locations.clone_from(&other.locations);
  }
}

impl JoinSemiLattice for RelevanceDomain {
  fn join(&mut self, other: &Self) -> bool {
    let b1 = self.places.join(&other.places);
    let b2 = self.mutated.join(&other.mutated);
    let b3 = self.locations.join(&other.locations);
    b1 || b2 || b3
  }
}
