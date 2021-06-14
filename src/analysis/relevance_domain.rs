use super::place_set::{PlaceDomain, PlaceIndex, PlaceSet};
use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_index::{
  bit_set::{HybridBitSet, SparseBitMatrix},
  vec::IndexVec,
};
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
}

#[derive(Clone, Debug)]
pub struct RelevantLocations(pub SparseBitMatrix<LocationIndex, PlaceIndex>);

impl RelevantLocations {
  pub fn new(domain: &PlaceDomain) -> Self {
    RelevantLocations(SparseBitMatrix::new(domain.len()))
  }

  pub fn insert(&mut self, location: LocationIndex, places: PlaceSet) {
    self.0.union_into_row(location, &places.to_hybrid());
  }

  pub fn iter<'a>(&'a self, domain: &'a LocationDomain) -> impl Iterator<Item = Location> + 'a {
    self
      .0
      .rows()
      .filter(move |location| self.0.row(*location).is_some())
      .map(move |index| domain.location(index))
  }

  pub fn contains(&self, location: LocationIndex) -> bool {
    self.0.row(location).is_some()
  }

  pub fn get(&self, location: LocationIndex) -> Option<&HybridBitSet<PlaceIndex>> {
    self.0.row(location)
  }
}

impl PartialEq for RelevantLocations {
  fn eq(&self, other: &Self) -> bool {
    (self.0.rows().count() == other.0.rows().count())
      && self
        .0
        .rows()
        .all(|row| match (self.0.row(row), other.0.row(row)) {
          (Some(s1), Some(s2)) => s1.superset(&s2) && s2.superset(&s1),
          _ => false,
        })
  }
}

impl JoinSemiLattice for RelevantLocations {
  fn join(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for row in other.0.rows() {
      if let Some(s) = other.0.row(row) {
        changed |= self.0.union_into_row(row, s);
      }
    }
    changed
  }
}

impl Eq for RelevantLocations {}

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
