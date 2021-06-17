use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::{bit_set::HybridBitSet, vec::IndexVec};
use rustc_middle::{
  mir::{Local, Place, ProjectionElem},
  ty::TyCtxt,
};
use rustc_mir::dataflow::{fmt::DebugWithContext, JoinSemiLattice};
use std::cell::RefCell;
use std::fmt;

rustc_index::newtype_index! {
    pub struct PlaceIndex {
        DEBUG_FORMAT = "p{}"
    }
}

struct NormalizedPlaces<'tcx> {
  tcx: TyCtxt<'tcx>,
  cache: HashMap<Place<'tcx>, Place<'tcx>>,
}

impl NormalizedPlaces<'tcx> {
  fn normalize(&mut self, place: Place<'tcx>) -> Place<'tcx> {
    let tcx = self.tcx;
    *self.cache.entry(place).or_insert_with(|| {
      let place = tcx.erase_regions(place);
      let projection = place
        .projection
        .into_iter()
        .map(|elem| match elem {
          ProjectionElem::Index(_) => ProjectionElem::Index(Local::from_usize(0)),
          _ => elem,
        })
        .collect::<Vec<_>>();

      Place {
        local: place.local,
        projection: tcx.intern_place_elems(&projection),
      }
    })
  }
}

pub struct PlaceDomain<'tcx> {
  index_to_place: IndexVec<PlaceIndex, Place<'tcx>>,
  place_to_index: HashMap<Place<'tcx>, PlaceIndex>,
  normalized_places: RefCell<NormalizedPlaces<'tcx>>,
}

impl PlaceDomain<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>, places: Vec<Place<'tcx>>) -> Self {
    let normalized_places = RefCell::new(NormalizedPlaces {
      tcx,
      cache: HashMap::default(),
    });
    let index_to_place = IndexVec::from_raw(
      places
        .into_iter()
        .map(|place| normalized_places.borrow_mut().normalize(place))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
    );
    let place_to_index = index_to_place
      .iter_enumerated()
      .map(|(idx, place)| (*place, idx))
      .collect();
    PlaceDomain {
      index_to_place,
      place_to_index,
      normalized_places,
    }
  }

  pub fn place(&self, index: PlaceIndex) -> Place<'tcx> {
    *self.index_to_place.get(index).unwrap()
  }

  pub fn index(&self, place: Place<'tcx>) -> PlaceIndex {
    *self
      .place_to_index
      .get(&self.normalized_places.borrow_mut().normalize(place))
      .unwrap()
  }

  pub fn len(&self) -> usize {
    self.index_to_place.len()
  }

  pub fn iter_enumerated<'a>(&'a self) -> impl Iterator<Item = (PlaceIndex, &'a Place<'tcx>)> + 'a {
    self.index_to_place.iter_enumerated()
  }
}

#[derive(Debug)]
pub struct PlaceSet(HybridBitSet<PlaceIndex>);

impl PlaceSet {
  pub fn new(domain: &PlaceDomain) -> Self {
    PlaceSet(HybridBitSet::new_empty(domain.len()))
  }

  pub fn indices<'a>(&'a self) -> impl Iterator<Item = PlaceIndex> + 'a {
    self.0.iter()
  }

  pub fn iter<'a, 'tcx>(
    &'a self,
    domain: &'a PlaceDomain<'tcx>,
  ) -> impl Iterator<Item = Place<'tcx>> + 'a {
    self.0.iter().map(move |index| domain.place(index))
  }

  pub fn iter_enumerated<'a, 'tcx>(
    &'a self,
    domain: &'a PlaceDomain<'tcx>,
  ) -> impl Iterator<Item = (PlaceIndex, Place<'tcx>)> + 'a {
    self.0.iter().map(move |index| (index, domain.place(index)))
  }

  pub fn insert(&mut self, index: PlaceIndex) {
    self.0.insert(index);
  }

  pub fn union(&mut self, other: &Self) -> bool {
    self.0.union(&other.0)
  }

  pub fn subtract(&mut self, other: &Self) -> bool {
    match (&mut self.0, &other.0) {
      (HybridBitSet::Dense(this), HybridBitSet::Dense(other)) => this.subtract(other),
      (this, other) => {
        let mut changed = false;
        for elem in other.iter() {
          changed |= this.remove(elem);
        }
        changed
      }
    }
  }

  pub fn contains(&self, index: PlaceIndex) -> bool {
    self.0.contains(index)
  }

  pub fn intersect(&mut self, other: &Self) -> bool {
    match (&mut self.0, &other.0) {
      (HybridBitSet::Dense(this), HybridBitSet::Dense(other)) => this.intersect(other),
      (this, other) => {
        let mut changes = Vec::new();
        for elem in this.iter() {
          if !other.contains(elem) {
            changes.push(elem);
          }
        }
        let changed = changes.len() > 0;
        for elem in changes {
          this.remove(elem);
        }
        changed
      }
    }
  }

  pub fn len(&self) -> usize {
    match &self.0 {
      HybridBitSet::Dense(this) => this.count(),
      HybridBitSet::Sparse(_) => self.0.iter().count(),
    }
  }

  pub fn to_hybrid(&self) -> HybridBitSet<PlaceIndex> {
    match &self.0 {
      HybridBitSet::Dense(this) => this.to_hybrid(),
      HybridBitSet::Sparse(_) => self.0.clone(),
    }
  }
}

impl PartialEq for PlaceSet {
  fn eq(&self, other: &Self) -> bool {
    self.0.superset(&other.0) && other.0.superset(&self.0)
  }
}
impl Eq for PlaceSet {}

pub trait PlaceSetIteratorExt {
  fn collect_indices(self, domain: &PlaceDomain<'tcx>) -> PlaceSet;
}

impl<T> PlaceSetIteratorExt for T
where
  T: Iterator<Item = PlaceIndex>,
{
  fn collect_indices(self, domain: &PlaceDomain<'tcx>) -> PlaceSet {
    let mut set = PlaceSet::new(domain);
    for idx in self {
      set.insert(idx);
    }
    set
  }
}

impl JoinSemiLattice for PlaceSet {
  fn join(&mut self, other: &Self) -> bool {
    self.union(&other)
  }
}

impl DebugWithContext<PlaceDomain<'tcx>> for PlaceSet {
  fn fmt_with(&self, ctxt: &PlaceDomain<'tcx>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let format_place = |place: Place| {
      let mut s = format!("{:?}", place.local);
      for elem in place.projection.iter() {
        s = match elem {
          ProjectionElem::Deref => format!("(*{})", s),
          ProjectionElem::Field(field, _) => format!("{}.{:?}", s, field.as_usize()),
          ProjectionElem::Index(_) => format!("{}[]", s),
          _ => format!("TODO({})", s),
        };
      }
      s
    };

    write!(
      f,
      "{{{}}}",
      self
        .iter(ctxt)
        .map(|place| format_place(place))
        .collect::<Vec<_>>()
        .join(", ")
    )
  }
}

impl Clone for PlaceSet {
  fn clone(&self) -> Self {
    PlaceSet(self.0.clone())
  }

  fn clone_from(&mut self, source: &Self) {
    self.0.clone_from(&source.0);
  }
}
