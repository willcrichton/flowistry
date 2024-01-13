//! This crate provides the Flowistry API, a modular information flow analysis for Rust programs.
//! The theory and evaluation of Flowistry is described in the paper ["Modular Information Flow through Ownership"](https://arxiv.org/abs/2111.13662) (Crichton et al. 2022).
//! See [example.rs](https://github.com/willcrichton/flowistry/tree/master/crates/flowistry/examples/example.rs)
//! for an example of how to use the Flowistry API.
//!
//! [Information flow](https://en.wikipedia.org/wiki/Information_flow_(information_theory))
//! is whether one instruction or variable can affect another during a
//! program's execution. Information flow can be used to analyze whether secure values
//! can leak to insecure places (["information flow control"](https://www.cse.chalmers.se/~andrei/mod11.pdf)),
//! and to analyze which parts of a program are relevant to a given variable (["program slicing"](https://en.wikipedia.org/wiki/Program_slicing)).
//!
//! This analysis uses the Rust compiler via the
//! [rustc API](https://doc.rust-lang.org/nightly/nightly-rustc/).
//! Given a [MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) [`Body`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/struct.Body.html),
//! the function [`infoflow::compute_flow`] computes the information flow
//! within that body.
//!
//! If you are interested in using Flowistry, please reach out directly to
//! Will Crichton ([wcrichto@cs.stanford.edu](mailto:wcrichto@cs.stanford.edu)) or [join
//! our Discord](https://discord.gg/XkcpkQn2Ah). If you use Flowistry in your research,
//! then please cite our paper:
//!
//! ```bib
//! @inproceedings{crichton2022,
//!   author = {Crichton, Will and Patrignani, Marco and Agrawala, Maneesh and Hanrahan, Pat},
//!   title = {Modular Information Flow through Ownership}, year = {2022},
//!   isbn = {9781450392655}, publisher = {Association for Computing Machinery},
//!   address = {New York, NY, USA}, url = {https://doi.org/10.1145/3519939.3523445},
//!   booktitle = {Proceedings of the 43rd ACM SIGPLAN International Conference on Programming Language Design and Implementation},
//!   pages = {1–14}, numpages = {14}, keywords = {information flow, rust, ownership types},
//!   location = {San Diego, CA, USA}, series = {PLDI 2022}, doi = {10.1145/3519939.3523445},
//! }
//! ```

#![feature(
  rustc_private,             // for rustc internals
  box_patterns,              // for conciseness
  associated_type_defaults,  // for crate::indexed::Indexed
  min_specialization,        // for rustc_index::newtype_index
  type_alias_impl_trait,     // for impl Trait in trait definition, eg crate::mir::utils 
  trait_alias,
  negative_impls,
)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return,
  clippy::len_zero,
  clippy::len_without_is_empty
)]
#![warn(missing_docs)]

extern crate either;
extern crate polonius_engine;
extern crate rustc_abi;
extern crate rustc_ast;
extern crate rustc_borrowck;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hash;
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
extern crate rustc_type_ir;
extern crate smallvec;

pub mod extensions;
pub mod infoflow;
pub mod mir;
#[cfg(feature = "pdg")]
pub mod pdg;
#[cfg(feature = "test")]
pub mod test_utils;
