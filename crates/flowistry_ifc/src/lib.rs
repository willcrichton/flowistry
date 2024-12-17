#![feature(rustc_private)]

extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_mir_dataflow;
extern crate rustc_span;
extern crate rustc_trait_selection;
extern crate rustc_traits;

mod analysis;

use std::{borrow::Cow, io::Write};

use analysis::IssueFound;
use flowistry::infoflow;
use rustc_hir::{
  intravisit::{self, Visitor},
  BodyId,
};
use rustc_middle::{hir::nested_filter::OnlyBodies, ty::TyCtxt};
use rustc_plugin::{CrateFilter, RustcPlugin, RustcPluginArgs};
use rustc_utils::mir::borrowck_facts;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
pub struct IfcPlugin;

impl RustcPlugin for IfcPlugin {
  type Args = ();

  fn driver_name(&self) -> Cow<'static, str> {
    "ifc-driver".into()
  }

  fn version(&self) -> Cow<'static, str> {
    env!("CARGO_PKG_VERSION").into()
  }

  fn args(&self, _target_dir: &rustc_plugin::Utf8Path) -> RustcPluginArgs<Self::Args> {
    RustcPluginArgs {
      args: (),
      filter: CrateFilter::OnlyWorkspace,
    }
  }

  fn run(
    self,
    compiler_args: Vec<String>,
    _plugin_args: Self::Args,
  ) -> rustc_interface::interface::Result<()> {
    rustc_driver::RunCompiler::new(&compiler_args, &mut Callbacks).run();
    Ok(())
  }
}

pub struct IfcVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  issue_found: IssueFound,
}

impl<'tcx> Visitor<'tcx> for IfcVisitor<'tcx> {
  type NestedFilter = OnlyBodies;

  fn nested_visit_map(&mut self) -> Self::Map {
    self.tcx.hir()
  }

  fn visit_nested_body(&mut self, body_id: BodyId) {
    intravisit::walk_body(self, self.tcx.hir().body(body_id));

    let tcx = self.tcx;
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts = borrowck_facts::get_body_with_borrowck_facts(tcx, local_def_id);
    let flow = &infoflow::compute_flow(tcx, body_id, body_with_facts);
    if let IssueFound::Yes = analysis::analyze(&body_id, flow).unwrap() {
      self.issue_found = IssueFound::Yes;
    }
  }
}

pub struct Callbacks;
impl rustc_driver::Callbacks for Callbacks {
  fn config(&mut self, config: &mut rustc_interface::Config) {
    borrowck_facts::enable_mir_simplification();
    config.override_queries = Some(borrowck_facts::override_queries);
  }

  fn after_analysis(
    &mut self,
    _compiler: &rustc_interface::interface::Compiler,
    tcx: TyCtxt,
  ) -> rustc_driver::Compilation {
    let mut visitor = IfcVisitor {
      tcx,
      issue_found: IssueFound::No,
    };
    tcx.hir().visit_all_item_likes_in_crate(&mut visitor);

    if let IssueFound::No = visitor.issue_found {
      let mut stdout = StandardStream::stderr(ColorChoice::Auto);
      let mut green_spec = ColorSpec::new();
      green_spec.set_fg(Some(Color::Green));
      stdout.set_color(&green_spec).unwrap();
      writeln!(stdout, "No security issues found!",).unwrap();
    }

    rustc_driver::Compilation::Stop
  }
}
