use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::vec::Enumerated;
use rustc_middle::{
  mir::{Body, Local, Location, Place, ProjectionElem},
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;
use std::{cell::RefCell, rc::Rc, slice::Iter};

use super::{
  indexed::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, ToIndex},
  utils,
};
use crate::to_index_impl;

rustc_index::newtype_index! {
  pub struct PlaceIndex {
      DEBUG_FORMAT = "p{}"
  }
}

to_index_impl!(Place<'tcx>);

pub struct NormalizedPlaces<'tcx> {
  tcx: TyCtxt<'tcx>,
  def_id: DefId,
  cache: HashMap<Place<'tcx>, Place<'tcx>>,
}

impl NormalizedPlaces<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>, def_id: DefId) -> Self {
    NormalizedPlaces {
      tcx,
      def_id,
      cache: HashMap::default(),
    }
  }

  pub fn normalize(&mut self, place: Place<'tcx>) -> Place<'tcx> {
    let tcx = self.tcx;
    let def_id = self.def_id;
    *self.cache.entry(place).or_insert_with(|| {
      // consider a place _1: &'1 <T as SomeTrait>::Foo[2]
      //   we might encounter this type with a different region, e.g. &'2
      //   we might encounter this type with a more specific type for the associated type, e.g. &'1 [i32][0]
      // to account for this variation, we use normalize_erasing_regions
      //
      // we also want any index to be treated the same, so we replace [i] => [0]
      let param_env = tcx.param_env(def_id);
      let place = tcx.normalize_erasing_regions(param_env, place);
      let projection = place
        .projection
        .into_iter()
        .filter_map(|elem| match elem {
          ProjectionElem::Index(_) | ProjectionElem::ConstantIndex { .. } => {
            Some(ProjectionElem::Index(Local::from_usize(0)))
          }
          ProjectionElem::Subslice { .. } => None,
          _ => Some(elem),
        })
        .collect::<Vec<_>>();

      utils::mk_place(place.local, &projection, tcx)
    })
  }
}

#[derive(Clone)]
pub struct PlaceDomain<'tcx> {
  domain: DefaultDomain<PlaceIndex, Place<'tcx>>,
  normalized_places: Rc<RefCell<NormalizedPlaces<'tcx>>>,
}

impl PlaceDomain<'tcx> {
  pub fn new(
    places: HashSet<Place<'tcx>>,
    normalized_places: Rc<RefCell<NormalizedPlaces<'tcx>>>,
  ) -> Self {
    let domain = DefaultDomain::new(
      places
        .into_iter()
        .map(|place| normalized_places.borrow_mut().normalize(place))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
    );

    PlaceDomain {
      domain,
      normalized_places,
    }
  }

  pub fn normalize(&self, place: Place<'tcx>) -> Place<'tcx> {
    self.normalized_places.borrow_mut().normalize(place)
  }
}

impl IndexedDomain for PlaceDomain<'tcx> {
  type Index = PlaceIndex;
  type Value = Place<'tcx>;

  fn value(&self, index: Self::Index) -> &Self::Value {
    self.domain.value(index)
  }

  fn index(&self, value: &Self::Value) -> Self::Index {
    self
      .domain
      .index(&self.normalized_places.borrow_mut().normalize(*value))
  }

  fn contains(&self, value: &Self::Value) -> bool {
    self
      .domain
      .contains(&self.normalized_places.borrow_mut().normalize(*value))
  }

  fn len(&self) -> usize {
    self.domain.len()
  }

  fn iter_enumerated<'a>(&'a self) -> Enumerated<Self::Index, Iter<'a, Self::Value>> {
    self.domain.iter_enumerated()
  }
}

impl IndexedValue for Place<'tcx> {
  type Index = PlaceIndex;
  type Domain = PlaceDomain<'tcx>;
}

pub type PlaceSet<'tcx> = IndexSet<Place<'tcx>>;

// impl DebugWithContext<PlaceDomain<'tcx>> for PlaceSet {
//   fn fmt_with(&self, ctxt: &PlaceDomain<'tcx>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     let format_place = |place: Place| {
//       let mut s = format!("{:?}", place.local);
//       for elem in place.projection.iter() {
//         s = match elem {
//           ProjectionElem::Deref => format!("(*{})", s),
//           ProjectionElem::Field(field, _) => format!("{}.{:?}", s, field.as_usize()),
//           ProjectionElem::Index(_) => format!("{}[]", s),
//           _ => format!("TODO({})", s),
//         };
//       }
//       s
//     };

//     write!(
//       f,
//       "{{{}}}",
//       self
//         .iter(ctxt)
//         .map(|place| format_place(place))
//         .collect::<Vec<_>>()
//         .join(", ")
//     )
//   }
// }

rustc_index::newtype_index! {
  pub struct LocationIndex {
      DEBUG_FORMAT = "l{}"
  }
}

to_index_impl!(Location);

impl IndexedValue for Location {
  type Index = LocationIndex;
}

pub type LocationSet = IndexSet<Location>;
pub type LocationDomain = <Location as IndexedValue>::Domain;

pub fn build_location_domain(body: &Body) -> Rc<LocationDomain> {
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
  Rc::new(LocationDomain::new(locations))
}
