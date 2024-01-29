#![feature(rustc_private)]

extern crate rustc_borrowck;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
use std::{env::consts::DLL_SUFFIX, process::Command};

use anyhow::{Context, Result};
use criterion::{
  criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion,
};
use flowistry::infoflow::Direction;
use glob::glob;
use rustc_borrowck::consumers::BodyWithBorrowckFacts;
use rustc_hir::{BodyId, ItemKind};
use rustc_middle::{
  mir::{Location, Place},
  ty::TyCtxt,
};
use rustc_utils::{mir::borrowck_facts, PlaceExt};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnalysisType {
  FlowOnly,
  FlowAndDeps,
}

fn analysis<'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'tcx BodyWithBorrowckFacts<'tcx>,
  ty: AnalysisType,
) {
  let results = flowistry::infoflow::compute_flow(tcx, body_id, body_with_facts);

  if ty == AnalysisType::FlowAndDeps {
    let targets = body_with_facts
      .body
      .local_decls
      .indices()
      .map(|local| {
        let arg = Place::make(local, &[], tcx);
        vec![(arg, Location::START.into())]
      })
      .collect::<Vec<_>>();

    flowistry::infoflow::compute_dependencies(&results, targets, Direction::Both);
  }
}

struct UnsafeBenchGroup(
  criterion::BenchmarkGroup<'static, criterion::measurement::WallTime>,
);

impl UnsafeBenchGroup {
  fn new(bench_group: BenchmarkGroup<WallTime>) -> Self {
    unsafe {
      UnsafeBenchGroup(std::mem::transmute::<
        criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
        criterion::BenchmarkGroup<'static, criterion::measurement::WallTime>,
      >(bench_group))
    }
  }
}

// SAFETY: Rustc requires that Callbacks implements Send in case the compiler is
// run with multiple threads (which we don't do)
unsafe impl Send for UnsafeBenchGroup {}

struct Callbacks {
  group: UnsafeBenchGroup,
}
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    borrowck_facts::enable_mir_simplification();
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().enter(|tcx| {
      let hir = tcx.hir();
      let body_id = hir
        .items()
        .filter_map(|id| match hir.item(id).kind {
          ItemKind::Fn(_, _, body) => Some(body),
          _ => None,
        })
        .next()
        .unwrap();

      let def_id = hir.body_owner_def_id(body_id);
      let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);

      for analysis_ty in [AnalysisType::FlowOnly, AnalysisType::FlowAndDeps] {
        let bench_id = match analysis_ty {
          AnalysisType::FlowOnly => "Flow",
          AnalysisType::FlowAndDeps => "Flow + Deps",
        };

        self.group.0.bench_function(bench_id, |b| {
          b.iter(|| analysis(tcx, body_id, body_with_facts, analysis_ty))
        });
      }
    });
    rustc_driver::Compilation::Stop
  }
}

fn criterion_benchmark(c: &mut Criterion) {
  const TESTS: &[(&str, &str)] = &[
    ("Locations", "locations.rs"),
    ("Unique Lifetimes", "lifetimes_unique.rs"),
    ("Infoflow", "infoflow.rs"),
    ("Places", "places.rs"),
    ("Same Lifetime", "lifetimes_same.rs"),
    ("Nested Structs", "nested_struct.rs"),
  ];

  (|| -> Result<()> {
    // The current binary should be in target/<profile>/deps/
    let current_exe =
      std::env::current_exe().context("Failed to find current executable")?;
    let curr_dir = current_exe
      .parent()
      .context("Failed to find path to current exe parent")?;
    let test_dir = curr_dir.join("../../../crates/flowistry/benches/tests");

    // The shared object for the bench_utils crate should also be in deps/
    let bench_crate_pattern = curr_dir.join(format!("*libbench_utils*{}", DLL_SUFFIX));

    let print_sysroot = Command::new("rustc")
      .args(&["--print", "sysroot"])
      .output()
      .context("Failed to print rustc sysroot")?
      .stdout;
    let sysroot = String::from_utf8(print_sysroot)?.trim().to_owned();

    // Find bench_utils .so file
    let shared_object = glob(bench_crate_pattern.to_str().unwrap())?
      .nth(0)
      .with_context(|| {
        format!(
          "Failed to find bench_utils shared object in dir {}",
          curr_dir.display()
        )
      })??;

    let mut run_bench = |test: (&str, &str)| {
      // Stress types correspond to bench files within ./tests/
      for stress_ty in ["min", "max"] {
        let test_name = format!("{} ({})", test.0, stress_ty);

        let mut args: Vec<String> = vec!["".into()];

        // Add test file to compiler args
        let test_file = std::path::Path::new(stress_ty).join(test.1);
        args.extend([test_dir.join(test_file).to_str().unwrap().into()]);

        // Add bench utils .so as extern
        args.extend([
          "--extern".into(),
          format!("bench_utils={}", shared_object.to_str().unwrap()),
        ]);

        args.extend(["--sysroot".into(), sysroot.clone()]);

        let group = UnsafeBenchGroup::new(c.benchmark_group(&test_name));

        let mut callbacks = Callbacks { group };
        rustc_driver::catch_fatal_errors(|| {
          rustc_driver::RunCompiler::new(&args, &mut callbacks)
            .run()
            .unwrap()
        })
        .unwrap();
      }
    };

    match std::env::var("FLOWISTRY_BENCH_TEST") {
      Ok(test_file) => {
        let test = TESTS
          .into_iter()
          .find(|t| t.1 == test_file)
          .with_context(|| format!("Failed to find test file '{test_file}'"))?;
        run_bench(*test);
      }
      _ => {
        for test in TESTS {
          run_bench(*test);
        }
      }
    }

    Ok(())
  })()
  .unwrap()
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
