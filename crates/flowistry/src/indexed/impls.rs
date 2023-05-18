use std::rc::Rc;

use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_middle::mir::{Body, Local, Location, Place};
pub use rustc_utils::source_map::spanner::LocationOrArg;
use rustc_utils::BodyExt;

use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, OwnedSet, ToIndex};
use crate::to_index_impl;

impl ToIndex<LocationOrArg> for Location {
  fn to_index(&self, domain: &LocationOrArgDomain) -> LocationOrArgIndex {
    domain.index(&LocationOrArg::Location(*self))
  }
}

impl ToIndex<LocationOrArg> for Local {
  fn to_index(&self, domain: &LocationOrArgDomain) -> LocationOrArgIndex {
    domain.index(&LocationOrArg::Arg(*self))
  }
}

rustc_index::newtype_index! {
  #[debug_format = "l{}"]
  pub struct LocationOrArgIndex {}
}

to_index_impl!(LocationOrArg);

impl IndexedValue for LocationOrArg {
  type Index = LocationOrArgIndex;
  type Domain = LocationOrArgDomain;
}

pub type LocationOrArgSet<S = OwnedSet<LocationOrArg>> = IndexSet<LocationOrArg, S>;
pub type LocationOrArgDomain = DefaultDomain<LocationOrArgIndex, LocationOrArg>;

pub fn build_location_arg_domain(body: &Body) -> Rc<LocationOrArgDomain> {
  let all_locations = body.all_locations().map(LocationOrArg::Location);
  let all_locals = body.args_iter().map(LocationOrArg::Arg);
  let domain = all_locations.chain(all_locals).collect::<Vec<_>>();
  Rc::new(LocationOrArgDomain::new(domain))
}

pub type PlaceSet<'tcx> = HashSet<Place<'tcx>>;
