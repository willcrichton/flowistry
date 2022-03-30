#![feature(rustc_private)]

extern crate rustc_borrowck;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;

use std::process::Command;

use criterion::{criterion_group, criterion_main, Criterion};
use flowistry::{
  infoflow::Direction,
  mir::{borrowck_facts, utils::PlaceExt},
};
use glob::glob;
use rustc_borrowck::BodyWithBorrowckFacts;
use rustc_hir::{BodyId, ItemKind};
use rustc_middle::{
  mir::{Location, Place},
  ty::TyCtxt,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum AnalysisType {
  FlowOnly,
  FlowAndDeps,
}

fn analysis<'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &BodyWithBorrowckFacts<'tcx>,
  ty: AnalysisType,
) {
  let results = flowistry::infoflow::compute_flow(tcx, body_id, body_with_facts);

  if ty == AnalysisType::FlowAndDeps {
    let mut targets = vec![];

    for local in body_with_facts.body.local_decls.indices() {
      let arg = Place::make(local, &[], tcx);
      targets.push(vec![(arg, Location::START)]);
    }
    
    flowistry::infoflow::compute_dependencies(
      &results,
      targets,
      Direction::Forward,
    );
  }
}

struct UnsafeBenchGroup(
  criterion::BenchmarkGroup<'static, criterion::measurement::WallTime>,
);

unsafe impl Send for UnsafeBenchGroup {}
unsafe impl Sync for UnsafeBenchGroup {}

struct Callbacks {
  ty: AnalysisType,
  group: UnsafeBenchGroup,
}
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_parsing<'tcx>(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    queries: &'tcx rustc_interface::Queries<'tcx>,
  ) -> rustc_driver::Compilation {
    queries.global_ctxt().unwrap().take().enter(|tcx| {
      let hir = tcx.hir();
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

      let bench_id = match self.ty {
        AnalysisType::FlowOnly => "Flow",
        AnalysisType::FlowAndDeps => "Flow + Deps",
      };

      self.group.0.bench_function(bench_id, |b| {
        b.iter(|| analysis(tcx, body_id, body_with_facts, self.ty))
      });
    });
    rustc_driver::Compilation::Stop
  }
}

fn criterion_benchmark(c: &mut Criterion) {
  let tests = vec![
    ("Locations", "locations.rs"),
    ("Unique Lifetimes", "lifetimes_unique.rs"),
    ("Infoflow", "infoflow.rs"),
    ("Places", "places.rs"),
    ("Same Lifetime", "lifetimes_same.rs"),
  ];
  let current_exe = std::env::current_exe().unwrap();
  let curr_dir = current_exe.parent().unwrap();
  let test_dir = curr_dir.join("../../../crates/flowistry/benches/tests");
  let bench_crate_pattern = curr_dir.join("*libbench_utils*.so");

  let print_sysroot = Command::new("rustc")
    .args(&["--print", "sysroot"])
    .output()
    .unwrap()
    .stdout;
  let sysroot = String::from_utf8(print_sysroot).unwrap().trim().to_owned();

  // Find bench_utils .so file
  let shared_object = glob(bench_crate_pattern.to_str().unwrap())
    .unwrap()
    .nth(0)
    .unwrap()
    .unwrap();

  let mut run_bench = |test: (&str, &str)| {
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

      for analysis_ty in [AnalysisType::FlowOnly, AnalysisType::FlowAndDeps] {
        let group = unsafe {
          UnsafeBenchGroup(std::mem::transmute::<
            criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
            criterion::BenchmarkGroup<'static, criterion::measurement::WallTime>,
          >(c.benchmark_group(&test_name)))
        };

        let mut callbacks = Callbacks {
          ty: analysis_ty,
          group,
        };
        rustc_driver::catch_fatal_errors(|| {
          rustc_driver::RunCompiler::new(&args, &mut callbacks)
            .run()
            .unwrap()
        })
        .unwrap();
      }
    }
  };

  if let Ok(test_file) = std::env::var("FLOWISTRY_BENCH_TEST") {
    let test = tests
      .clone()
      .into_iter()
      .find(|t| t.1 == test_file)
      .unwrap();
    return run_bench(test);
  }

  for test in tests {
    run_bench(test);
  }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
