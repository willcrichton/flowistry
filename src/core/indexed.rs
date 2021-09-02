#![allow(dead_code)]

use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_index::{
  bit_set::{HybridBitSet, HybridIter, SparseBitMatrix},
  vec::{Enumerated, Idx, IndexVec},
};

use rustc_mir::dataflow::{fmt::DebugWithContext, JoinSemiLattice};
use std::{
  fmt,
  hash::Hash,
  ops::{Deref, DerefMut},
  rc::Rc,
  slice::Iter,
};

pub trait IndexedValue: Eq + Hash + Clone + fmt::Debug {
  type Index: Idx;
  type Domain: IndexedDomain<Index = Self::Index, Value = Self> = DefaultDomain<Self::Index, Self>;
}

pub trait ToIndex<T: IndexedValue> {
  fn to_index(&self, domain: &T::Domain) -> T::Index;
}

impl<T: IndexedValue> ToIndex<T> for T {
  fn to_index(&self, domain: &T::Domain) -> T::Index {
    domain.index(self)
  }
}

// Can't make this a blanket impl b/c it conflicts with the blanket impl above :(
#[macro_export]
macro_rules! to_index_impl {
  ($t:ty) => {
    impl ToIndex<$t> for <$t as IndexedValue>::Index {
      fn to_index(&self, _domain: &<$t as IndexedValue>::Domain) -> <$t as IndexedValue>::Index {
        *self
      }
    }
  };
}

pub trait IndexedDomain {
  type Value: IndexedValue;
  type Index: Idx = <Self::Value as IndexedValue>::Index;
  fn value(&self, index: Self::Index) -> &Self::Value;
  fn index(&self, value: &Self::Value) -> Self::Index;
  fn len(&self) -> usize;
  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>>;
}

#[derive(Clone)]
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
    *self
      .value_to_index
      .get(value)
      .unwrap_or_else(|| panic!("No index for value: {:?}", value))
  }

  fn len(&self) -> usize {
    self.index_to_value.len()
  }

  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>> {
    self.index_to_value.iter_enumerated()
  }
}

#[derive(Clone)]
pub struct OwnedSet<T: IndexedValue>(HybridBitSet<T::Index>);
#[derive(Clone, Copy)]
pub struct RefSet<'a, T: IndexedValue>(&'a HybridBitSet<T::Index>);
pub struct MutSet<'a, T: IndexedValue>(&'a mut HybridBitSet<T::Index>);

impl<T: IndexedValue> Deref for OwnedSet<T> {
  type Target = HybridBitSet<T::Index>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: IndexedValue> DerefMut for OwnedSet<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<T: IndexedValue> Deref for RefSet<'_, T> {
  type Target = HybridBitSet<T::Index>;

  fn deref(&self) -> &Self::Target {
    self.0
  }
}

pub trait ToSet<T: IndexedValue>: Deref<Target = HybridBitSet<T::Index>> {}
pub trait ToSetMut<T: IndexedValue>: DerefMut<Target = HybridBitSet<T::Index>> {}

impl<S: Deref<Target = HybridBitSet<T::Index>>, T: IndexedValue> ToSet<T> for S {}
impl<S: DerefMut<Target = HybridBitSet<T::Index>>, T: IndexedValue> ToSetMut<T> for S {}

pub struct IndexSet<T: IndexedValue, S = OwnedSet<T>> {
  set: S,
  domain: Rc<T::Domain>,
}

impl<T: IndexedValue> IndexSet<T, OwnedSet<T>> {
  pub fn new(domain: Rc<T::Domain>) -> Self {
    IndexSet {
      set: OwnedSet(HybridBitSet::new_empty(domain.len())),
      domain,
    }
  }
}

impl<T, S> IndexSet<T, S>
where
  T: IndexedValue,
  S: ToSet<T>,
{
  pub fn to_owned(&self) -> IndexSet<T, OwnedSet<T>> {
    IndexSet {
      set: OwnedSet(self.set.clone()),
      domain: self.domain.clone(),
    }
  }

  pub fn as_ref(&self) -> IndexSet<T, RefSet<T>> {
    IndexSet {
      set: RefSet(&*self.set),
      domain: self.domain.clone(),
    }
  }

  pub fn indices(&self) -> HybridIter<'_, T::Index> {
    self.set.iter()
  }

  pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
    self.set.iter().map(move |index| self.domain.value(index))
  }

  pub fn iter_enumerated(&self) -> impl Iterator<Item = (T::Index, &T)> + '_ {
    self
      .set
      .iter()
      .map(move |index| (index, self.domain.value(index)))
  }

  pub fn contains(&self, index: impl ToIndex<T>) -> bool {
    let elem = index.to_index(&self.domain);
    self.set.contains(elem)
  }

  pub fn len(&self) -> usize {
    match &*self.set {
      HybridBitSet::Dense(this) => this.count(),
      HybridBitSet::Sparse(_) => self.set.iter().count(),
    }
  }

  pub fn is_superset<S2: ToSet<T>>(&self, other: &IndexSet<T, S2>) -> bool {
    self.set.superset(&*other.set)
  }
}

impl<T: IndexedValue, S: ToSetMut<T>> IndexSet<T, S> {
  pub fn insert(&mut self, elt: impl ToIndex<T>) {
    let elt = elt.to_index(&self.domain);
    self.set.insert(elt);
  }

  pub fn union<S2: ToSet<T>>(&mut self, other: &IndexSet<T, S2>) -> bool {
    self.set.union(&other.set)
  }

  pub fn subtract<S2: ToSet<T>>(&mut self, other: &IndexSet<T, S2>) -> bool {
    match (&mut *self.set, &*other.set) {
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

  pub fn intersect<S2: ToSet<T>>(&mut self, other: &IndexSet<T, S2>) -> bool {
    match (&mut *self.set, &*other.set) {
      (HybridBitSet::Dense(this), HybridBitSet::Dense(other)) => this.intersect(other),
      (this, other) => {
        let mut changes = Vec::new();
        for elem in this.iter() {
          if !other.contains(elem) {
            changes.push(elem);
          }
        }

        let changed = !changes.is_empty();
        for elem in changes {
          this.remove(elem);
        }
        changed
      }
    }
  }
}

impl<T: IndexedValue, S: ToSet<T>> PartialEq for IndexSet<T, S> {
  fn eq(&self, other: &Self) -> bool {
    self.is_superset(other) && other.is_superset(self)
  }
}

impl<T: IndexedValue, S: ToSet<T>> Eq for IndexSet<T, S> {}

impl<T: IndexedValue, S: ToSetMut<T>> JoinSemiLattice for IndexSet<T, S> {
  fn join(&mut self, other: &Self) -> bool {
    self.union(other)
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

impl<T: IndexedValue + fmt::Debug, S: ToSet<T>> fmt::Debug for IndexSet<T, S> {
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

impl<T: IndexedValue + fmt::Debug, S: ToSet<T>, C> DebugWithContext<C> for IndexSet<T, S>
where
  T::Index: ToIndex<T>,
{
  fn fmt_diff_with(&self, old: &Self, _ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self == old {
      return Ok(());
    }

    let added = self
      .indices()
      .filter(|idx| !old.contains(*idx))
      .collect_indices(self.domain.clone());
    let removed = old
      .indices()
      .filter(|idx| !self.contains(*idx))
      .collect_indices(self.domain.clone());

    if added.len() > 0 {
      write!(f, "\u{001f}+{:?}", added)?;
    }

    if removed.len() > 0 {
      write!(f, "\u{001f}-{:?}", removed)?;
    }

    Ok(())
  }
}
pub trait IndexSetIteratorExt<T: IndexedValue> {
  fn collect_indices(self, domain: Rc<T::Domain>) -> IndexSet<T>;
}

impl<T, S, Iter> IndexSetIteratorExt<T> for Iter
where
  T: IndexedValue,
  Iter: Iterator<Item = S>,
  S: ToIndex<T>,
{
  fn collect_indices(self, domain: Rc<T::Domain>) -> IndexSet<T> {
    let mut set = IndexSet::new(domain);
    for s in self {
      set.insert(s);
    }
    set
  }
}

#[derive(Clone)]
pub struct IndexMatrix<R: IndexedValue, C: IndexedValue> {
  matrix: SparseBitMatrix<R::Index, C::Index>,
  row_domain: Rc<R::Domain>,
  col_domain: Rc<C::Domain>,
}

impl<R: IndexedValue, C: IndexedValue> IndexMatrix<R, C> {
  pub fn new(row_domain: Rc<R::Domain>, col_domain: Rc<C::Domain>) -> Self {
    IndexMatrix {
      matrix: SparseBitMatrix::new(col_domain.len()),
      row_domain,
      col_domain,
    }
  }

  pub fn insert(&mut self, row: impl ToIndex<R>, col: impl ToIndex<C>) -> bool {
    let row = row.to_index(&self.row_domain);
    let col = col.to_index(&self.col_domain);
    self.matrix.insert(row, col)
  }

  pub fn union_into_row<S2>(&mut self, into: impl ToIndex<R>, from: &IndexSet<C, S2>) -> bool
  where
    S2: Deref<Target = HybridBitSet<C::Index>>,
  {
    let into = into.to_index(&self.row_domain);
    self.matrix.union_into_row(into, &from.set)
  }

  pub fn row_indices(&self, row: impl ToIndex<R>) -> impl Iterator<Item = C::Index> + '_ {
    let row = row.to_index(&self.row_domain);
    self
      .matrix
      .row(row)
      .into_iter()
      .map(|set| set.iter())
      .flatten()
  }

  pub fn row<'a>(&'a self, row: impl ToIndex<R> + 'a) -> impl Iterator<Item = &'a C> + 'a {
    self
      .row_indices(row)
      .map(move |idx| self.col_domain.value(idx))
  }

  pub fn row_set<'a>(&'a self, row: impl ToIndex<R>) -> Option<IndexSet<C, RefSet<'a, C>>> {
    let row = row.to_index(&self.row_domain);
    self.matrix.row(row).map(|set| IndexSet {
      set: RefSet(set),
      domain: self.col_domain.clone(),
    })
  }

  pub fn rows(&self) -> impl Iterator<Item = R::Index> {
    self.matrix.rows()
  }

  pub fn clear_row(&mut self, row: impl ToIndex<R>) {
    let row = row.to_index(&self.row_domain);
    if let Some(set) = self.matrix.row(row) {
      // FIXME: unsafe hack, update this once my SparseBitMatrix PR is merged
      let set =
        unsafe { &mut *(set as *const HybridBitSet<C::Index> as *mut HybridBitSet<C::Index>) };
      set.clear();
    }
  }
}

impl<R: IndexedValue, C: IndexedValue> PartialEq for IndexMatrix<R, C> {
  fn eq(&self, other: &Self) -> bool {
    self.matrix.rows().count() == other.matrix.rows().count()
      && self.matrix.rows().all(|row| match self.matrix.row(row) {
        Some(set) => match other.matrix.row(row) {
          Some(other_set) => set.superset(other_set) && other_set.superset(set),
          None => false,
        },
        None => true,
      })
  }
}

impl<R: IndexedValue, C: IndexedValue> Eq for IndexMatrix<R, C> {}

impl<R: IndexedValue, C: IndexedValue> JoinSemiLattice for IndexMatrix<R, C> {
  fn join(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for row in other.matrix.rows() {
      if let Some(set) = other.matrix.row(row) {
        changed |= self.matrix.union_into_row(row, set);
      }
    }
    return changed;
  }
}

impl<R: IndexedValue + fmt::Debug, C: IndexedValue + fmt::Debug> fmt::Debug for IndexMatrix<R, C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;

    for row in self.matrix.rows() {
      let n = self.matrix.iter(row).count();
      if n == 0 {
        continue;
      }

      write!(f, "  {:?}: [", self.row_domain.value(row))?;
      for (i, col) in self.matrix.iter(row).enumerate() {
        write!(f, "{:?}", self.col_domain.value(col))?;
        if i < n - 1 {
          write!(f, ", ")?;
        }
      }
      write!(f, "]<br align=\"left\" />")?;
    }

    write!(f, "}}<br align=\"left\" />")
  }
}

impl<R: IndexedValue + fmt::Debug, C: IndexedValue + fmt::Debug, Ctx> DebugWithContext<Ctx>
  for IndexMatrix<R, C>
where
  R::Index: ToIndex<R>,
  C::Index: ToIndex<C>,
{
  fn fmt_diff_with(&self, old: &Self, ctxt: &Ctx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self == old {
      return Ok(());
    }

    let empty = IndexSet::new(self.col_domain.clone());
    let empty = empty.as_ref();
    for (row, set) in self
      .rows()
      .filter_map(|row| self.row_set(row).map(|set| (row, set)))
    {
      let row_value = self.row_domain.value(row);
      let old_set = old.row_set(row);
      let old_set = old_set.as_ref().unwrap_or(&empty);

      if old_set == &set {
        continue;
      }

      write!(f, "{:?}: ", row_value)?;
      set.fmt_diff_with(old_set, ctxt, f)?;
      writeln!(f)?;
    }

    Ok(())
  }
}
