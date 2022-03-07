//! This crate provides a modular information flow analysis for Rust programs,
//! as described in the paper ["Modular Information Flow Through Ownership"](https://arxiv.org/abs/2111.13662) (Crichton et al. 2022).
//! See [example.rs](https://github.com/willcrichton/flowistry/tree/master/crates/flowistry/examples/example.rs)
//! for an example of how to use the Flowistry API.
//!
//! [Information flow](https://en.wikipedia.org/wiki/Information_flow_(information_theory))
//! describes whether one instruction or variable can affect another during a
//! program's execution. Information flow can be used to analyze whether secure values
//! can leak to insecure places (["information flow control"](https://www.cse.chalmers.se/~andrei/mod11.pdf)),
//! and to analyze which parts of a program are relevant to a given variable (["program slicing"](https://en.wikipedia.org/wiki/Program_slicing)).
//!
//! This analysis uses the Rust compiler via the
//! [rustc API](https://doc.rust-lang.org/nightly/nightly-rustc/).
//! Given a [MIR](https://rustc-dev-guide.rust-lang.org/mir/index.html) body,
//! the function [`infoflow::compute_flow`] computes the information flow
//! within that body. Check out those docs for more information on the specific
//! data structure that is computed.
//!
//! If you are interested in using Flowistry, please reach out directly to
//! Will Crichton ([wcrichto@cs.stanford.edu](mailto:wcrichto@cs.stanford.edu))
//! for questions or support. If you use Flowistry in your research, then please cite
//! our paper:
//!
//! ```bibtex
//! @misc{crichton2021modular,
//!   title={Modular Information Flow Through Ownership},
//!   author={Will Crichton and Marco Patrignani and Maneesh Agrawala and Pat Hanrahan},
//!   year={2021},
//!   eprint={2111.13662},
//!   archivePrefix={arXiv},
//!   primaryClass={cs.PL}
//! }
//! ```

#![feature(
  rustc_private,             // for rustc internals
  box_patterns,              // for conciseness
  in_band_lifetimes,         // for conciseness
  associated_type_defaults,  // for crate::indexed::Indexed
  min_specialization,        // for rustc_index::newtype_index
  type_alias_impl_trait,     // for impl Trait in trait definition, eg crate::mir::utils 
  generic_associated_types,  // for impl Trait in trait definition
  crate_visibility_modifier, // for crate-wide shared private items
  trait_alias,
)]
#![allow(
  clippy::single_match,
  clippy::needless_lifetimes,
  clippy::needless_return,
  clippy::len_zero
)]

extern crate either;
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

mod cached;
pub mod extensions;
pub mod indexed;
pub mod infoflow;
pub mod mir;
pub mod range;
pub mod source_map;
#[cfg(feature = "test")]
pub mod test_utils;
pub mod timer;
