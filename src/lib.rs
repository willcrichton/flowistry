#![feature(rustc_private, box_patterns, in_band_lifetimes)]
#![feature(const_panic, min_specialization)] // needed for rustc_index::newtype_index
#![feature(control_flow_enum)] // needed for alias analysis

extern crate indexmap;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_serialize;
extern crate rustc_span;
extern crate rustc_target;
extern crate smallvec;

mod core;
// mod flow;
mod slicing;

pub use slicing::{slice, Config, Range};