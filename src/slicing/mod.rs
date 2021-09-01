use crate::{
  core::{
    analysis::{FlowistryAnalysis, FlowistryOutput},
    config::Range,
    utils,
  },
  flow::{self, Direction},
};
use anyhow::Result;
use log::debug;
use rustc_data_structures::fx::FxHashSet as HashSet;
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
use serde::Serialize;

struct ForwardSliceAnalysis {
  direction: Direction,
  range: Range,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceOutput {
  pub ranges: Vec<Range>,
  pub num_instructions: usize,
  pub num_relevant_instructions: usize,
  pub mutated_inputs: HashSet<usize>,
  pub relevant_inputs: HashSet<usize>,
}

impl SliceOutput {
  pub fn ranges(&self) -> &Vec<Range> {
    &self.ranges
  }
}

impl FlowistryOutput for SliceOutput {
  fn empty() -> Self {
    SliceOutput {
      ranges: Vec::new(),
      num_instructions: 0,
      num_relevant_instructions: 0,
      mutated_inputs: HashSet::default(),
      relevant_inputs: HashSet::default(),
    }
  }

  fn merge(&mut self, other: SliceOutput) {
    self.ranges.extend(other.ranges.into_iter());
    self.num_instructions = other.num_instructions;
    self.num_relevant_instructions = other.num_relevant_instructions;
    self.mutated_inputs = other.mutated_inputs;
    self.relevant_inputs = other.relevant_inputs;
  }
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

    let results = flow::compute_flow(tcx, body, &body_with_facts.input_facts);
    if std::env::var("DUMP_MIR").is_ok() {
      utils::dump_results("target/flow.png", body, &results)?;
    }

    let source_map = tcx.sess.source_map();
    let sliced_places = utils::span_to_places(body, self.range.to_span(source_map)?);
    debug!("sliced_places {:?}", sliced_places);

    let hir_body = tcx.hir().body(body_id);
    let spanner = utils::HirSpanner::new(hir_body);

    let deps = flow::compute_dependency_ranges(&results, sliced_places, self.direction, &spanner);

    let mut output = SliceOutput::empty();
    output.ranges = deps.into_iter().map(|v| v.into_iter()).flatten().collect();
    Ok(output)
  }
}

pub fn slice(direction: Direction, range: Range, compiler_args: &[String]) -> Result<SliceOutput> {
  ForwardSliceAnalysis { direction, range }.run(compiler_args)
}
