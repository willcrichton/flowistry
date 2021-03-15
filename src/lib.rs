#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_span;
extern crate rustc_target;

mod analysis;
mod config;

pub use analysis::{slice, SliceOutput};
pub use config::{Config, Range};
