use std::{ops::Deref, path::PathBuf, rc::Rc};

use rustc_data_structures::fx::FxHashSet as HashSet;
use rustc_middle::mir::{Body, Local, Location, Place};
use serde::Serialize;

use super::{DefaultDomain, IndexSet, IndexedDomain, IndexedValue, OwnedSet, ToIndex};
use crate::{
  mir::utils::{BodyExt, PlaceExt},
  to_index_impl,
};

/// Used to represent dependencies of places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocationOrArg {
  Location(Location),
  Arg(Local),
}

impl LocationOrArg {
  pub fn from_place<'tcx>(place: Place<'tcx>, body: &Body<'tcx>) -> Option<Self> {
    place
      .is_arg(body)
      .then_some(LocationOrArg::Arg(place.local))
  }
}

impl From<Location> for LocationOrArg {
  fn from(location: Location) -> Self {
    LocationOrArg::Location(location)
  }
}

impl ToIndex<LocationOrArg> for Location {
  fn to_index(&self, domain: &LocationOrArgDomain) -> LocationOrArgIndex {
    domain.index(&LocationOrArg::Location(*self))
  }
}

impl From<Local> for LocationOrArg {
  fn from(local: Local) -> Self {
    LocationOrArg::Arg(local)
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Filename(pub PathBuf);

rustc_index::newtype_index! {
  #[derive(Serialize)]
  #[debug_format = "f{}"]
  pub struct FilenameIndex {}
}

// Filenames are interned at the thread-level, so they should only be
// used within a given thread. Generally sending an index across a thread
// boundary is a logical error.
impl !Send for FilenameIndex {}

to_index_impl!(Filename);

pub type FilenameDomain = DefaultDomain<FilenameIndex, Filename>;

impl IndexedValue for Filename {
  type Index = FilenameIndex;
  type Domain = FilenameDomain;
}

impl Deref for Filename {
  type Target = PathBuf;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
