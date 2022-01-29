#![feature(rustc_private, in_band_lifetimes, box_patterns, slice_group_by)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return,
  clippy::len_zero
)]

extern crate either;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_macros;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_serialize;
extern crate rustc_span;

pub mod analysis;
pub mod decompose;
pub mod effects;
pub mod focus;
mod hir;
pub mod mutations;
pub mod playground;
pub mod range;
pub mod slicing;
