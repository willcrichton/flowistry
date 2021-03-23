#![feature(rustc_private)]
#![feature(const_panic)] // needed for rustc_index::newtype_index

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

mod analysis;
mod config;

pub use analysis::{slice, SliceOutput};
pub use config::{Config, Range};
