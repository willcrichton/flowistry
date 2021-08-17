use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::{
  bit_set::{HybridBitSet, SparseBitMatrix},
  vec::{Enumerated, Idx, IndexVec},
};
use rustc_middle::{
  mir::{Local, Place, ProjectionElem},
  ty::TyCtxt,
};
use rustc_mir::dataflow::{fmt::DebugWithContext, JoinSemiLattice};
use std::{fmt, hash::Hash, ops::Deref, rc::Rc, slice::Iter};

pub trait IndexedValue: Eq + Hash + Clone {
  type Index: Idx;
  type Domain: IndexedDomain<Index = Self::Index, Value = Self> = DefaultDomain<Self::Index, Self>;
}

pub trait IndexedDomain {
  type Value: IndexedValue;
  type Index: Idx = <Self::Value as IndexedValue>::Index;
  fn value(&self, index: Self::Index) -> &Self::Value;
  fn index(&self, value: &Self::Value) -> Self::Index;
  fn len(&self) -> usize;
  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>>;
}

pub struct DefaultDomain<I: Idx, T> {
  index_to_value: IndexVec<I, T>,
  value_to_index: HashMap<T, I>,
}

impl<I: Idx, T: IndexedValue> DefaultDomain<I, T> {
  pub fn new(domain: Vec<T>) -> Self {
    let index_to_value = IndexVec::from_raw(domain);
    let value_to_index = index_to_value
      .iter_enumerated()
      .map(|(idx, t)| (t.clone(), idx))
      .collect();
    DefaultDomain {
      index_to_value,
      value_to_index,
    }
  }
}

impl<I: Idx, T: IndexedValue> IndexedDomain for DefaultDomain<I, T> {
  type Index = I;
  type Value = T;

  fn value(&self, index: I) -> &T {
    self.index_to_value.get(index).unwrap()
  }

  fn index(&self, value: &T) -> I {
    *self.value_to_index.get(value).unwrap()
  }

  fn len(&self) -> usize {
    self.index_to_value.len()
  }

  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>> {
    self.index_to_value.iter_enumerated()
  }
}

pub struct IndexSet<T: IndexedValue> {
  set: HybridBitSet<T::Index>,
  domain: Rc<T::Domain>,
}

impl<T: IndexedValue> IndexSet<T> {
  pub fn new(domain: Rc<T::Domain>) -> Self {
    IndexSet {
      set: HybridBitSet::new_empty(domain.len()),
      domain,
    }
  }

  pub fn indices<'a>(&'a self) -> impl Iterator<Item = T::Index> + 'a {
    self.set.iter()
  }

  pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a T> + 'a {
    self.set.iter().map(move |index| self.domain.value(index))
  }

  pub fn iter_enumerated<'a, 'tcx>(&'a self) -> impl Iterator<Item = (T::Index, &'a T)> + 'a {
    self
      .set
      .iter()
      .map(move |index| (index, self.domain.value(index)))
  }

  pub fn insert(&mut self, index: T::Index) {
    self.set.insert(index);
  }

  pub fn union(&mut self, other: &Self) -> bool {
    self.set.union(&other.set)
  }

  pub fn subtract(&mut self, other: &Self) -> bool {
    match (&mut self.set, &other.set) {
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

  pub fn contains(&self, index: T::Index) -> bool {
    self.set.contains(index)
  }

  pub fn intersect(&mut self, other: &Self) -> bool {
    match (&mut self.set, &other.set) {
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
    match &self.set {
      HybridBitSet::Dense(this) => this.count(),
      HybridBitSet::Sparse(_) => self.set.iter().count(),
    }
  }

  pub fn to_hybrid(&self) -> HybridBitSet<T::Index> {
    match &self.set {
      HybridBitSet::Dense(this) => this.to_hybrid(),
      HybridBitSet::Sparse(_) => self.set.clone(),
    }
  }
}

impl<T: IndexedValue> PartialEq for IndexSet<T> {
  fn eq(&self, other: &Self) -> bool {
    self.set.superset(&other.set) && other.set.superset(&self.set)
  }
}

impl<T: IndexedValue> Eq for IndexSet<T> {}

impl<T: IndexedValue> JoinSemiLattice for IndexSet<T> {
  fn join(&mut self, other: &Self) -> bool {
    self.union(&other)
  }
}

impl<T: IndexedValue> Clone for IndexSet<T> {
  fn clone(&self) -> Self {
    IndexSet {
      set: self.set.clone(),
      domain: self.domain.clone(),
    }
  }

  fn clone_from(&mut self, source: &Self) {
    self.set.clone_from(&source.set);
    self.domain = source.domain.clone();
  }
}

impl<T: IndexedValue + fmt::Debug> fmt::Debug for IndexSet<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;
    let n = self.len();
    for (i, elt) in self.iter().enumerate() {
      write!(f, "{:?}", elt)?;
      if i < n - 1 {
        write!(f, ", ")?
      }
    }

    write!(f, "}}")
  }
}

impl<T: IndexedValue + fmt::Debug, C> DebugWithContext<C> for IndexSet<T> {}

pub trait IndexSetIteratorExt: Iterator {
  fn collect_indices<T: IndexedValue<Index = Self::Item>>(
    self,
    domain: Rc<T::Domain>,
  ) -> IndexSet<T>;
}

impl<Iter: Iterator> IndexSetIteratorExt for Iter {
  fn collect_indices<T: IndexedValue<Index = Self::Item>>(
    self,
    domain: Rc<T::Domain>,
  ) -> IndexSet<T> {
    let mut set = IndexSet::new(domain);
    for idx in self {
      set.insert(idx);
    }
    set
  }
}
