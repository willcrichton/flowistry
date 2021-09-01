use crate::{
  backward_slicing::{Range, SliceOutput},
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    utils,
  },
  flow::{compute_flow, dependencies},
};
use anyhow::Result;
use log::debug;
use rustc_hir::BodyId;
use rustc_middle::{
  mir::*,
  ty::{TyCtxt, WithOptConstParam},
};
use rustc_mir::{
  consumers::get_body_with_borrowck_facts,
  transform::{simplify, MirPass},
};
use rustc_span::Span;

pub use dependencies::Direction;

struct ForwardSliceAnalysis {
  direction: Direction,
  range: Range,
  // extensions: Slic
}

struct SimplifyMir;
impl MirPass<'tcx> for SimplifyMir {
  fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    for block in body.basic_blocks_mut() {
      block.retain_statements(|stmt| match stmt.kind {
        // TODO: variable_select_lhs test fails if we remove FakeRead
        // StatementKind::FakeRead(..)
        StatementKind::StorageLive(..) | StatementKind::StorageDead(..) => false,
        _ => true,
      });

      let terminator = block.terminator_mut();
      terminator.kind = match terminator.kind {
        TerminatorKind::FalseEdge { real_target, .. } => TerminatorKind::Goto {
          target: real_target,
        },
        TerminatorKind::FalseUnwind { real_target, .. } => TerminatorKind::Goto {
          target: real_target,
        },
        _ => continue,
      }
    }
  }
}

impl FlowistryAnalysis for ForwardSliceAnalysis {
  type Output = SliceOutput;

  fn locations(&self, tcx: TyCtxt) -> Result<Vec<Span>> {
    Ok(vec![self.range.to_span(tcx.sess.source_map())?])
  }

  fn analyze_function(&mut self, tcx: TyCtxt, body_id: BodyId) -> Result<Self::Output> {
    let local_def_id = tcx.hir().body_owner_def_id(body_id);
    let body_with_facts =
      get_body_with_borrowck_facts(tcx, WithOptConstParam::unknown(local_def_id));
    let mut body = body_with_facts.body.clone();

    SimplifyMir.run_pass(tcx, &mut body);
    simplify::SimplifyCfg::new("flowistry").run_pass(tcx, &mut body);

    let body = &body;
    debug!("{}", utils::mir_to_string(tcx, body)?);

    let results = compute_flow(tcx, body, &body_with_facts.input_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/flow.png", body, &results)?;
    }

    let source_map = tcx.sess.source_map();
    let sliced_places = utils::span_to_places(tcx, body, self.range.to_span(source_map)?);
    debug!("sliced_places {:?}", sliced_places);

    let hir_body = tcx.hir().body(body_id);
    let spanner = utils::HirSpanner::new(hir_body);

    let deps =
      dependencies::compute_dependency_ranges(&results, sliced_places, self.direction, &spanner);

    let mut output = SliceOutput::empty();
    output.ranges = deps.into_iter().map(|v| v.into_iter()).flatten().collect();
    Ok(output)
  }
}

pub fn slice(direction: Direction, range: Range, compiler_args: &[String]) -> Result<SliceOutput> {
  ForwardSliceAnalysis { direction, range }.run(compiler_args)
}
