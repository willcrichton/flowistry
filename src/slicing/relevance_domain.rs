use crate::core::indexed_impls::{LocationDomain, LocationSet, PlaceDomain, PlaceSet};

use rustc_mir::dataflow::JoinSemiLattice;
use std::rc::Rc;

#[derive(PartialEq, Eq, Debug)]
pub struct RelevanceDomain<'tcx> {
  pub places: PlaceSet<'tcx>,
  pub mutated: PlaceSet<'tcx>,
  pub locations: LocationSet,
}

impl RelevanceDomain<'tcx> {
  pub fn new(place_domain: Rc<PlaceDomain<'tcx>>, location_domain: Rc<LocationDomain>) -> Self {
    let places = PlaceSet::new(place_domain.clone());
    let mutated = PlaceSet::new(place_domain);
    let locations = LocationSet::new(location_domain);
    RelevanceDomain {
      places,
      mutated,
      locations,
    }
  }
}

impl Clone for RelevanceDomain<'_> {
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

impl JoinSemiLattice for RelevanceDomain<'_> {
  fn join(&mut self, other: &Self) -> bool {
    let b1 = self.places.join(&other.places);
    let b2 = self.mutated.join(&other.mutated);
    let b3 = self.locations.join(&other.locations);
    b1 || b2 || b3
  }
}
