#![feature(rustc_private)]
#![allow(unused_variables, dead_code, unused_imports)]

use crate::analysis::analyze;
use rustc_hir::{itemlikevisit::ItemLikeVisitor, ForeignItem, ImplItem, Item, ItemKind, TraitItem};
use rustc_middle::ty::TyCtxt;
use std::env;
use std::process::Command;

mod analysis;

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_index;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir;
extern crate rustc_span;

struct BorrowChecker<'tcx> {
  tcx: TyCtxt<'tcx>,
  range: ((i32, i32), (i32, i32))
}

impl<'hir, 'tcx> ItemLikeVisitor<'hir> for BorrowChecker<'tcx> {
  fn visit_item(&mut self, item: &'hir Item<'hir>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        println!("FN {}", item.ident);
        analyze(self.tcx, body_id, self.range);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'hir TraitItem<'hir>) {}
  fn visit_impl_item(&mut self, _impl_item: &'hir ImplItem<'hir>) {}
  fn visit_foreign_item(&mut self, _foreign_item: &'hir ForeignItem<'hir>) {}
}

struct Callbacks {
  range: ((i32, i32), (i32, i32))
}

impl rustc_driver::Callbacks for Callbacks {
  fn after_analysis<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let mut borrow_checker = BorrowChecker { tcx, range: self.range };
      tcx.hir().krate().visit_all_item_likes(&mut borrow_checker);
    });

    rustc_driver::Compilation::Stop
  }
}

fn main() {
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

  let args = env::args().collect::<Vec<_>>();
  if let [path, start_line, start_col, end_line, end_col] = &args[1..6] {
    let args = format!("--crate-name=simple --edition=2018 {path} \
    --sysroot {sysroot} \
    --crate-type bin --emit=dep-info,link -C embed-bitcode=no \
    -C debuginfo=2 -C metadata=c83e72487cf0751d --out-dir /Users/will/Code/tmp/simple/target/debug/deps \
    -C incremental=/Users/will/Code/tmp/simple/target/debug/incremental \
    -L dependency=/Users/will/Code/tmp/simple/target/debug/deps -Z dump-mir=RelevanceAnalysis -C opt-level=0", 
    path = path,
    sysroot = sysroot)
    .split(" ").map(str::to_string).collect::<Vec<_>>();

    let range = (
      (
        start_line.parse::<i32>().unwrap(),
        start_col.parse::<i32>().unwrap(),
      ),
      (
        end_line.parse::<i32>().unwrap(),
        end_col.parse::<i32>().unwrap(),
      ),
    );
    let mut callbacks = Callbacks { range };

    rustc_driver::catch_fatal_errors(|| {
      rustc_driver::RunCompiler::new(&args, &mut callbacks)
        .run()
        .unwrap();
    })
    .unwrap();
  }
}
