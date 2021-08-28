#![feature(
  rustc_private,
  box_patterns,
  in_band_lifetimes,
  associated_type_defaults,
  type_alias_impl_trait,
  generic_associated_types
)]
#![feature(const_panic, min_specialization)] // needed for rustc_index::newtype_index
#![feature(control_flow_enum)] // needed for alias analysis

extern crate polonius_engine;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_serialize;
extern crate rustc_span;
extern crate rustc_target;
extern crate smallvec;

mod backward_slicing;
mod core;
mod effects;
mod flow;
mod forward_slicing;

pub use backward_slicing::{backward_slice, Config, Range, SliceOutput};
pub use effects::effects;
pub use flow::flow;
pub use forward_slicing::forward_slice;
