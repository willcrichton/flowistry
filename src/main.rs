#![feature(rustc_private)]
//#![allow(unused_variables, dead_code, unused_imports)]

use crate::analysis::analyze;
use crate::config::{Config, Range, CONFIG};
use anyhow::{Context, Result};
use clap::clap_app;
use rustc_hir::{itemlikevisit::ItemLikeVisitor, ForeignItem, ImplItem, Item, ItemKind, TraitItem};
use rustc_middle::ty::TyCtxt;

mod analysis;
mod config;
mod relevance;
mod points_to;

extern crate rustc_driver;
extern crate rustc_graphviz;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_span;
extern crate rustc_target;

struct BorrowChecker<'tcx> {
  tcx: TyCtxt<'tcx>,
}

impl<'hir, 'tcx> ItemLikeVisitor<'hir> for BorrowChecker<'tcx> {
  fn visit_item(&mut self, item: &'hir Item<'hir>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        println!("FN {}", item.ident);
        analyze(self.tcx, body_id).unwrap();
      }
      _ => {}
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'hir TraitItem<'hir>) {}
  fn visit_impl_item(&mut self, _impl_item: &'hir ImplItem<'hir>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'hir ForeignItem<'hir>) {}
}

struct Callbacks;

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let mut borrow_checker = BorrowChecker { tcx };
      tcx.hir().krate().visit_all_item_likes(&mut borrow_checker);
    });

    rustc_driver::Compilation::Stop
  }
}

fn run() -> Result<()> {
  env_logger::init();
  
  let sysroot = "/Users/will/Code/rust/build/x86_64-apple-darwin/stage1".to_string();
  // let sysroot = String::from_utf8(
  //   Command::new("rustc")
  //     .args(&["--print", "sysroot"])
  //     .output()
  //     .unwrap()
  //     .stdout,
  // )
  // .unwrap()
  // .trim()
  // .to_string();

  let matches = clap_app!(app =>
    (@arg debug: -d)
    (@arg path:)
    (@arg start_line:)
    (@arg start_col:)
    (@arg end_line:)
    (@arg end_col:)
  )
  .get_matches();

  macro_rules! arg {
    ($key:expr) => { matches.value_of($key).context($key)? }
  }

  let args = format!("--crate-name=simple --edition=2018 {path} \
  --sysroot {sysroot} \
  --crate-type bin --emit=dep-info,link -C embed-bitcode=no \
  -C debuginfo=2 -C metadata=c83e72487cf0751d --out-dir /Users/will/Code/tmp/simple/target/debug/deps \
  -C incremental=/Users/will/Code/tmp/simple/target/debug/incremental \
  -L dependency=/Users/will/Code/tmp/simple/target/debug/deps -Z dump-mir=RelevanceAnalysis \
  -C opt-level=0 -Z mir-opt-level=0",  // mir-opt-level is critical!
  path = arg!("path"),
  sysroot = sysroot)
  .split(" ").map(str::to_string).collect::<Vec<_>>();

  CONFIG
    .set(Config {
      range: Range {
        start_line: arg!("start_line").parse::<usize>()?,
        start_col: arg!("start_col").parse::<usize>().unwrap(),
        end_line: arg!("end_line").parse::<usize>().unwrap(),
        end_col: arg!("end_col").parse::<usize>().unwrap(),
      },
      debug: matches.is_present("debug")
    })
    .expect("Could not set config");

  let mut callbacks = Callbacks;

  rustc_driver::catch_fatal_errors(|| {
    rustc_driver::RunCompiler::new(&args, &mut callbacks)
      .run()
      .unwrap();
  })
  .unwrap();

  Ok(())
}

fn main() {
  run().unwrap();
}
