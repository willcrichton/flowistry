use rustc_data_structures::fx::FxIndexSet;
use rustc_macros::HashStable;
use rustc_middle::mir::{visit::{PlaceContext, Visitor}, *};
use rustc_index::bit_set::BitSet;

use std::collections::HashSet;

rustc_index::newtype_index! {
  pub struct PlaceIndex {
    derive [HashStable]
    DEBUG_FORMAT = "pl{}"
  }
}

pub type PlaceSet = BitSet<PlaceIndex>;

struct CollectPlaces<'tcx> {
  index_set: FxIndexSet<Place<'tcx>>,
}

impl<'tcx> Visitor<'tcx> for CollectPlaces<'tcx> {
  fn visit_place(&mut self, place: &Place<'tcx>, _context: PlaceContext, _location: Location) {
    self.index_set.insert_full(*place);
  }
}

pub struct PlaceIndices<'tcx> {
  index_set: FxIndexSet<Place<'tcx>>,
}

impl<'tcx> PlaceIndices<'tcx> {
  pub fn build(body: &Body<'tcx>) -> Self {
    let mut place_collector = CollectPlaces {
      index_set: FxIndexSet::default()
    };
    place_collector.visit_body(body);

    PlaceIndices {
      index_set: place_collector.index_set
    }
  }

  pub fn index(&self, place: &Place<'tcx>) -> PlaceIndex {
    PlaceIndex::from(self.index_set.get_index_of(place).unwrap())
  }

  pub fn lookup(&self, index: PlaceIndex) -> Place<'tcx> {
    self.index_set[index.as_usize()]
  }

  pub fn empty_set(&self) -> PlaceSet {
    BitSet::new_empty(self.index_set.len())
  }
}
