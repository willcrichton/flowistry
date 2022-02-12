use std::{cell::RefCell, hash::Hash, mem, pin::Pin};

use rustc_data_structures::fx::FxHashMap as HashMap;

pub struct Cached<In, Out>(RefCell<HashMap<In, Pin<Box<Out>>>>);

impl<In, Out> Cached<In, Out>
where
  In: Hash + Eq + Clone,
  Out: Unpin,
{
  pub fn get(&'a self, key: In, compute: impl FnOnce(In) -> Out) -> &'a Out {
    let mut cache = self.0.borrow_mut();
    let entry = cache
      .entry(key.clone())
      .or_insert_with(move || Pin::new(Box::new(compute(key))));
    unsafe { mem::transmute::<&'_ Out, &'a Out>(&**entry) }
  }
}

impl<In, Out> Default for Cached<In, Out> {
  fn default() -> Self {
    Cached(RefCell::new(HashMap::default()))
  }
}

#[test]
fn test_cached() {
  let cache: Cached<usize, usize> = Cached::default();
  let x = cache.get(0, |_| 0);
  let y = cache.get(1, |_| 1);
  let z = cache.get(0, |_| 2);
  assert_eq!(*x, 0);
  assert_eq!(*y, 1);
  assert_eq!(*z, 0);
  assert!(std::ptr::eq(x, z));
}
