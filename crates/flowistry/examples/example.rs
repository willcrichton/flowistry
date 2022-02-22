//! A basic example that shows how to invoke rustc and use Flowistry to compute
//! information flows. It takes a source file with one function, and prints out
//! the forward dependencies of the first argument in that function.
//!
//! To run it from the Flowistry workspace, do:
//! ```bash
//! echo "fn example(x: i32) {
//!  let mut y = 1;
//!  if x > 0 { y = 2; }
//!  let z = 3;
//! }" > test.rs
//! cargo run --example example -- test.rs
//! ```

#![feature(rustc_private)]

extern crate rustc_borrowck;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;

use std::process::Command;

use flowistry::{
  infoflow::Direction,
  mir::{
    borrowck_facts,
    utils::{BodyExt, PlaceExt},
  },
};
use rustc_borrowck::BodyWithBorrowckFacts;
use rustc_hir::{BodyId, ItemKind};
use rustc_middle::{
  mir::{Local, Location, Place},
  ty::TyCtxt,
};

// This is the core analysis. Everything below this function is plumbing to
// call into rustc's API.
fn analysis<'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &BodyWithBorrowckFacts<'tcx>,
) {
  println!("Body:\n{}", body_with_facts.body.to_string(tcx).unwrap());

  // This computes the core information flow data structure. But it's not very
  // visualizable, so we need to post-process it with a specific query.
  let results = flowistry::infoflow::compute_flow(tcx, body_id, body_with_facts);

  // We construct a target of the first argument at the start of the function.
  let arg = Place::make(Local::from_usize(1), &[], tcx);
  let targets = vec![vec![(arg, Location::START)]];

  // Then use Flowistry to compute the locations and places influenced by the target.
  let (location_deps, _place_deps) = flowistry::infoflow::compute_dependencies(
    &results,
    targets.clone(),
    Direction::Forward,
  )
  .remove(0);

  // And print out those forward dependencies.
  println!("The forward dependencies of targets {targets:?} are:");
  let source_map = tcx.sess.source_map();
  for location in location_deps.iter() {
    let span = body_with_facts.body.source_info(*location).span;
    println!(
      "Location {location:?}: \"{}\"",
      source_map.span_to_snippet(span).unwrap()
    );
  }
}

struct Callbacks;
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    // You MUST configure rustc to ensure `get_body_with_borrowck_facts` will work.
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let hir = tcx.hir();

      // Get the first body body we can find
      let body_id = hir
        .items()
        .filter_map(|item| match item.kind {
          ItemKind::Fn(_, _, body) => Some(body),
          _ => None,
        })
        .next()
        .unwrap();

      let def_id = hir.body_owner_def_id(body_id);
      let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
      analysis(tcx, body_id, body_with_facts)
    });
    rustc_driver::Compilation::Stop
  }
}

fn main() {
  // Get the sysroot so rustc can find libstd
  let print_sysroot = Command::new("rustc")
    .args(&["--print", "sysroot"])
    .output()
    .unwrap()
    .stdout;
  let sysroot = String::from_utf8(print_sysroot).unwrap().trim().to_owned();

  let mut args = std::env::args().collect::<Vec<_>>();
  args.extend(["--sysroot".into(), sysroot]);

  // Run rustc with the given arguments
  let mut callbacks = Callbacks;
  rustc_driver::catch_fatal_errors(|| {
    rustc_driver::RunCompiler::new(&args, &mut callbacks)
      .run()
      .unwrap()
  })
  .unwrap();
}
