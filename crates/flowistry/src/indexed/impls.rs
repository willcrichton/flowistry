use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, ToIndex};
use crate::{mir::utils, to_index_impl};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_index::vec::IndexVec;
use rustc_infer::infer::TyCtxtInferExt;
use rustc_middle::{
  mir::{BasicBlock, Body, Local, Location, Place, ProjectionElem},
  traits::ObligationCause,
  ty::TyCtxt,
};
use rustc_span::def_id::DefId;
use rustc_trait_selection::infer::InferCtxtExt;
use std::{cell::RefCell, rc::Rc};

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
      let place = tcx.erase_regions(place);
      let place = tcx.infer_ctxt().enter(|infcx| {
        infcx
          .partially_normalize_associated_types_in(ObligationCause::dummy(), param_env, place)
          .value
      });

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

  pub fn all_args(&self, body: &Body<'tcx>) -> Vec<PlaceIndex> {
    self
      .domain
      .as_vec()
      .iter_enumerated()
      .filter(|(_, place)| utils::is_arg(**place, body))
      .map(|(index, _)| index)
      .collect()
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

  fn as_vec(&self) -> &IndexVec<Self::Index, Self::Value> {
    self.domain.as_vec()
  }
}

impl IndexedValue for Place<'tcx> {
  type Index = PlaceIndex;
  type Domain = PlaceDomain<'tcx>;
}

pub type PlaceSet<'tcx> = IndexSet<Place<'tcx>>;

rustc_index::newtype_index! {
  pub struct LocationIndex {
      DEBUG_FORMAT = "l{}"
  }
}

to_index_impl!(Location);

impl IndexedValue for Location {
  type Index = LocationIndex;
  type Domain = LocationDomain;
}

pub type LocationSet = IndexSet<Location>;
pub struct LocationDomain {
  domain: DefaultDomain<LocationIndex, Location>,
  arg_to_location: HashMap<PlaceIndex, LocationIndex>,
}

impl LocationDomain {
  pub fn new(body: &Body, place_domain: &Rc<PlaceDomain>) -> Rc<Self> {
    let mut locations = body
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

    let arg_block = BasicBlock::from_usize(body.basic_blocks().len());

    let (arg_places, arg_locations): (Vec<_>, Vec<_>) = place_domain
      .as_vec()
      .iter()
      .filter(|place| utils::is_arg(**place, body))
      .enumerate()
      .map(|(i, place)| {
        (
          *place,
          Location {
            block: arg_block,
            statement_index: i,
          },
        )
      })
      .unzip();

    locations.extend(&arg_locations);

    let domain = DefaultDomain::new(locations);

    let arg_to_location = arg_places
      .iter()
      .zip(arg_locations.iter())
      .map(|(place, location)| (place_domain.index(place), domain.index(location)))
      .collect::<HashMap<_, _>>();

    Rc::new(LocationDomain {
      domain,
      arg_to_location,
    })
  }

  pub fn num_real_locations(&self) -> usize {
    self.domain.size() - self.arg_to_location.len()
  }

  pub fn arg_to_location(&self, arg: PlaceIndex) -> LocationIndex {
    *self.arg_to_location.get(&arg).unwrap()
  }
}

impl IndexedDomain for LocationDomain {
  type Index = LocationIndex;
  type Value = Location;

  fn value(&self, index: Self::Index) -> &Self::Value {
    self.domain.value(index)
  }

  fn index(&self, value: &Self::Value) -> Self::Index {
    self.domain.index(value)
  }

  fn contains(&self, value: &Self::Value) -> bool {
    self.domain.contains(value)
  }

  fn as_vec(&self) -> &IndexVec<Self::Index, Self::Value> {
    self.domain.as_vec()
  }
}
