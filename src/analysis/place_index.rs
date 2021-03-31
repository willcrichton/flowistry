use rustc_data_structures::fx::FxIndexSet;
use rustc_index::bit_set::BitSet;
use rustc_macros::HashStable;
use rustc_middle::mir::{
  visit::{PlaceContext, Visitor},
  *,
};
use rustc_mir::dataflow::fmt::DebugWithContext;
use std::borrow::Cow;
use std::fmt;

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
      index_set: FxIndexSet::default(),
    };
    place_collector.visit_body(body);

    PlaceIndices {
      index_set: place_collector.index_set,
    }
  }

  pub fn indices<'a>(&'a self) -> impl Iterator<Item=PlaceIndex> + 'a {
    self.index_set.iter().map(move |place| self.index(place))
  }

  pub fn insert(&mut self, place: &Place<'tcx>) -> PlaceIndex {
    self.index_set.insert(*place);
    self.index(place)
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

  pub fn vec_to_set(&self, places: &Vec<Place>) -> PlaceSet {
    let mut set = self.empty_set();
    for p in places {
      set.insert(self.index(p));
    }
    set
  }

  pub fn count(&self) -> usize {
    self.index_set.len()
  }
}

impl DebugWithContext<PlaceIndices<'_>> for PlaceSet {
  fn fmt_with(&self, ctxt: &PlaceIndices<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;
    let n = self.count();
    for (i, index) in self.iter().enumerate() {
      let place = format!("{:?}", ctxt.lookup(index));
      let place_sanitized = rustc_graphviz::LabelText::LabelStr(Cow::from(place)).to_dot_string();
      write!(f, "{}", place_sanitized)?;
      if i < n - 1 {
        write!(f, ", ")?;
      }
    }
    write!(f, "}}")
  }
}

impl DebugWithContext<PlaceIndices<'_>> for Vec<PlaceIndex> {
  fn fmt_with(&self, ctxt: &PlaceIndices<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;
    let n = self.len();
    for (i, index) in self.iter().enumerate() {
      let place = format!("{:?}", ctxt.lookup(*index));
      let place_sanitized = rustc_graphviz::LabelText::LabelStr(Cow::from(place)).to_dot_string();
      write!(f, "{}", place_sanitized)?;
      if i < n - 1 {
        write!(f, ", ")?;
      }
    }
    write!(f, "}}")
  }
}
