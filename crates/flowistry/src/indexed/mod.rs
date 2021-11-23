use rustc_data_structures::fx::FxHashMap as HashMap;
use rustc_index::{
  bit_set::BitSet,
  vec::{Idx, IndexVec},
};
use rustc_mir_dataflow::{fmt::DebugWithContext, JoinSemiLattice};
use std::{
  fmt,
  hash::Hash,
  ops::{Deref, DerefMut},
  rc::Rc,
};

pub mod impls;

pub trait IndexedValue: Eq + Hash + Clone + Ord + fmt::Debug {
  type Index: Idx + ToIndex<Self>;
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

impl<T: IndexedValue> ToIndex<T> for &T {
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
  fn contains(&self, value: &Self::Value) -> bool;
  fn as_vec(&self) -> &IndexVec<Self::Index, Self::Value>;
  fn size(&self) -> usize {
    self.as_vec().len()
  }
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
  fn contains(&self, value: &T) -> bool {
    self.value_to_index.contains_key(value)
  }

  fn as_vec(&self) -> &IndexVec<Self::Index, Self::Value> {
    &self.index_to_value
  }
}

type IndexSetImpl<T> = BitSet<T>;

#[derive(Clone)]
pub struct OwnedSet<T: IndexedValue>(IndexSetImpl<T::Index>);
#[derive(Clone, Copy)]
pub struct RefSet<'a, T: IndexedValue>(&'a IndexSetImpl<T::Index>);
pub struct MutSet<'a, T: IndexedValue>(&'a mut IndexSetImpl<T::Index>);

impl<T: IndexedValue> Deref for OwnedSet<T> {
  type Target = IndexSetImpl<T::Index>;

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
  type Target = IndexSetImpl<T::Index>;

  fn deref(&self) -> &Self::Target {
    self.0
  }
}

pub trait ToSet<T: IndexedValue>: Deref<Target = IndexSetImpl<T::Index>> {}
pub trait ToSetMut<T: IndexedValue>: DerefMut<Target = IndexSetImpl<T::Index>> {}

impl<S: Deref<Target = IndexSetImpl<T::Index>>, T: IndexedValue> ToSet<T> for S {}
impl<S: DerefMut<Target = IndexSetImpl<T::Index>>, T: IndexedValue> ToSetMut<T> for S {}

pub struct IndexSet<T: IndexedValue, S = OwnedSet<T>> {
  set: S,
  domain: Rc<T::Domain>,
}

impl<T: IndexedValue> IndexSet<T, OwnedSet<T>> {
  pub fn new(domain: Rc<T::Domain>) -> Self {
    IndexSet {
      set: OwnedSet(IndexSetImpl::new_empty(domain.as_vec().len())),
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

  pub fn indices(&self) -> impl Iterator<Item = T::Index> + '_ {
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
    self.set.count()
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
    self.set.union(&*other.set)
  }

  pub fn subtract<S2: ToSet<T>>(&mut self, other: &IndexSet<T, S2>) -> bool {
    self.set.subtract(&*other.set)
  }

  pub fn intersect<S2: ToSet<T>>(&mut self, other: &IndexSet<T, S2>) -> bool {
    self.set.intersect(&*other.set)
  }
}

impl<T: IndexedValue, S: ToSet<T>> PartialEq for IndexSet<T, S> {
  fn eq(&self, other: &Self) -> bool {
    *self.set == *other.set
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
        write!(f, ", ")?;
        if f.alternate() {
          write!(f, "\n  ")?;
        }
      }
    }

    write!(f, "}}")
  }
}

struct Escape<T>(T);
impl<T: fmt::Debug> fmt::Display for Escape<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", html_escape::encode_text(&format!("{:?}", self.0)))
  }
}

impl<T: IndexedValue + fmt::Debug, S: ToSet<T>, C> DebugWithContext<C> for IndexSet<T, S>
where
  T::Index: ToIndex<T>,
{
  fn fmt_with(&self, _ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut elts = self.iter().collect::<Vec<_>>();
    elts.sort();
    write!(f, "{}", Escape(elts))
  }

  fn fmt_diff_with(&self, old: &Self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
      write!(f, "\u{001f}+")?;
      added.fmt_with(ctxt, f)?;
    }

    if removed.len() > 0 {
      write!(f, "\u{001f}-")?;
      removed.fmt_with(ctxt, f)?;
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

pub struct IndexMatrix<R: IndexedValue, C: IndexedValue> {
  matrix: HashMap<R::Index, IndexSetImpl<C::Index>>,
  pub row_domain: Rc<R::Domain>,
  pub col_domain: Rc<C::Domain>,
}

impl<R: IndexedValue, C: IndexedValue> IndexMatrix<R, C> {
  pub fn new(row_domain: Rc<R::Domain>, col_domain: Rc<C::Domain>) -> Self {
    IndexMatrix {
      matrix: HashMap::default(),
      row_domain,
      col_domain,
    }
  }

  fn ensure_row(&mut self, row: impl ToIndex<R>) -> &mut IndexSetImpl<C::Index> {
    let row = row.to_index(&self.row_domain);
    let nc = self.col_domain.size();
    self
      .matrix
      .entry(row)
      .or_insert_with(|| IndexSetImpl::new_empty(nc))
  }

  pub fn insert(&mut self, row: impl ToIndex<R>, col: impl ToIndex<C>) -> bool {
    let col = col.to_index(&self.col_domain);
    self.ensure_row(row).insert(col)
  }

  pub fn union_into_row<S2>(&mut self, into: impl ToIndex<R>, from: &IndexSet<C, S2>) -> bool
  where
    S2: ToSet<C>,
  {
    self.ensure_row(into).union(&*from.set)
  }

  pub fn row<'a>(&'a self, row: impl ToIndex<R> + 'a) -> impl Iterator<Item = &'a C> + 'a {
    let row = row.to_index(&self.row_domain);
    self
      .matrix
      .get(&row)
      .into_iter()
      .map(move |set| set.iter().map(move |idx| self.col_domain.value(idx)))
      .flatten()
  }

  pub fn row_set<'a>(&'a self, row: impl ToIndex<R>) -> Option<IndexSet<C, RefSet<'a, C>>> {
    let row = row.to_index(&self.row_domain);
    self.matrix.get(&row).map(|set| IndexSet {
      set: RefSet(set),
      domain: self.col_domain.clone(),
    })
  }

  pub fn rows<'a>(&'a self) -> impl Iterator<Item = (R::Index, IndexSet<C, RefSet<'a, C>>)> + 'a {
    self.matrix.iter().map(move |(row, col)| {
      (
        *row,
        IndexSet {
          set: RefSet(col),
          domain: self.col_domain.clone(),
        },
      )
    })
  }

  pub fn clear_row(&mut self, row: impl ToIndex<R>) {
    let row = row.to_index(&self.row_domain);
    self.matrix.remove(&row);
  }
}

impl<R: IndexedValue, C: IndexedValue> PartialEq for IndexMatrix<R, C> {
  fn eq(&self, other: &Self) -> bool {
    self.matrix.len() == other.matrix.len()
      && self
        .matrix
        .iter()
        .all(|(row, col)| match other.matrix.get(row) {
          Some(other_col) => col == other_col,
          None => false,
        })
  }
}

impl<R: IndexedValue, C: IndexedValue> Eq for IndexMatrix<R, C> {}

impl<R: IndexedValue, C: IndexedValue> JoinSemiLattice for IndexMatrix<R, C> {
  fn join(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for (row, col) in other.matrix.iter() {
      changed |= self.ensure_row(*row).union(col);
    }
    return changed;
  }
}

impl<R: IndexedValue, C: IndexedValue> Clone for IndexMatrix<R, C> {
  fn clone(&self) -> Self {
    Self {
      matrix: self.matrix.clone(),
      row_domain: self.row_domain.clone(),
      col_domain: self.col_domain.clone(),
    }
  }

  fn clone_from(&mut self, source: &Self) {
    for col in self.matrix.values_mut() {
      col.clear();
    }

    for (row, col) in source.matrix.iter() {
      self.ensure_row(*row).clone_from(col);
    }

    self.row_domain = source.row_domain.clone();
    self.col_domain = source.col_domain.clone();
  }
}

impl<R: IndexedValue + fmt::Debug, C: IndexedValue + fmt::Debug> fmt::Debug for IndexMatrix<R, C> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;

    for row in self.matrix.keys() {
      let n = self.matrix.get(row).map(|set| set.count()).unwrap_or(0);
      if n == 0 {
        continue;
      }

      write!(
        f,
        "  {:?}: {:?},",
        self.row_domain.value(*row),
        self.row_set(*row).unwrap()
      )?;
    }

    write!(f, "}}")
  }
}

impl<R: IndexedValue + fmt::Debug, C: IndexedValue + fmt::Debug, Ctx> DebugWithContext<Ctx>
  for IndexMatrix<R, C>
{
  fn fmt_with(&self, ctxt: &Ctx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{{")?;

    for row in self.matrix.keys() {
      if let Some(row_set) = self.row_set(*row) {
        if row_set.len() == 0 {
          continue;
        }

        write!(f, "  {}: ", Escape(self.row_domain.value(*row)))?;
        row_set.fmt_with(ctxt, f)?;
        write!(f, "]<br align=\"left\" />")?;
      }
    }

    write!(f, "}}<br align=\"left\" />")
  }

  fn fmt_diff_with(&self, old: &Self, ctxt: &Ctx, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self == old {
      return Ok(());
    }

    let empty = IndexSet::new(self.col_domain.clone());
    let empty = empty.as_ref();
    for (row, set) in self.rows() {
      let row_value = self.row_domain.value(row);
      let old_set = old.row_set(row);
      let old_set = old_set.as_ref().unwrap_or(&empty);

      if old_set == &set {
        continue;
      }

      write!(f, "{}: ", Escape(row_value))?;
      set.fmt_diff_with(old_set, ctxt, f)?;
      writeln!(f)?;
    }

    Ok(())
  }
}
