#![feature(
  rustc_private,             // for rustc internals
  box_patterns,              // nice-to-have
  in_band_lifetimes,         // nice-to-have
  associated_type_defaults,  // for crate::core::indexed::Indexed
  min_specialization,        // for rustc_index::newtype_index
)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return
)]

extern crate polonius_engine;
extern crate rustc_borrowck;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_mir_transform;
extern crate rustc_serialize;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate smallvec;

mod core;
mod effects;
mod flow;
mod slicing;

pub use crate::core::{
  analysis::{FlowistryError, FlowistryResult},
  config::{self, Range},
  utils,
  extensions,
};
pub use effects::{effects, FunctionIdentifier};
pub use flow::{compute_dependencies, compute_flow, Direction};
pub use slicing::{slice, SliceOutput};
