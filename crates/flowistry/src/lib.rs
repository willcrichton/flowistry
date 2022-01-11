#![feature(
  rustc_private,             // for rustc internals
  box_patterns,              // for conciseness
  in_band_lifetimes,         // for conciseness
  associated_type_defaults,  // for crate::indexed::Indexed
  min_specialization,        // for rustc_index::newtype_index
  type_alias_impl_trait,     // for impl Trait in trait definition, eg crate::mir::utils 
  generic_associated_types,  // for impl Trait in trait definition
  crate_visibility_modifier, // for crate-wide shared private items
)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return,
  clippy::len_zero
)]

extern crate datafrog;
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
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;
extern crate rustc_trait_selection;
extern crate smallvec;

pub mod extensions;
pub mod indexed;
pub mod infoflow;
pub mod mir;
pub mod source_map;
pub mod test_utils;
pub mod timer;
