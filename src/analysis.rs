use rustc_index::bit_set::{BitSet, HybridBitSet};
use rustc_middle::{
  mir::{
    self,
    visit::{PlaceContext, Visitor},
    *,
  },
  ty::TyCtxt,
};
use rustc_mir::dataflow::{
  fmt::DebugWithContext, impls::MaybeMutBorrowedLocals, Analysis, AnalysisDomain, Backward,
  GenKill, GenKillAnalysis, JoinSemiLattice, ResultsVisitor,
};
use rustc_mir::util::write_mir_pretty;
use rustc_span::{BytePos, Span, source_map::SourceMap};
use std::{collections::HashSet, fmt, io};
use serde::Serialize;

type SliceSet = HashSet<(Local, Location)>;

// #[derive(Clone, Debug, PartialEq, Eq)]
// struct RelevanceDomain {
//   relevant: BitSet<Local>
// }

// impl RelevanceDomain {
//   fn bottom_value<'tcx>(body: &Body<'tcx>) -> Self {
//     let relevant = BitSet::new_empty(body.local_decls().len());
//     RelevanceDomain { relevant }
//   }
// }

// impl JoinSemiLattice for RelevanceDomain {
//   fn join(&mut self, other: &Self) -> bool {
//       self.relevant.join(&other.relevant)
//   }
// }

// impl<C> DebugWithContext<C> for RelevanceDomain {
//   fn fmt_with(&self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//     self.relevant.fmt_with(ctxt, f)
//   }

//   fn fmt_diff_with(&self, old: &Self, ctxt: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//       self.relevant.fmt_diff_with(old.relevant, ctxt, f)
//   }
// }

type RelevanceDomain = BitSet<Local>;

struct CollectLocals {
  locals: HybridBitSet<Local>,
}

impl<'tcx> Visitor<'tcx> for CollectLocals {
  fn visit_local(&mut self, local: &Local, _context: PlaceContext, _location: Location) {
    self.locals.insert(*local);
  }
}

struct TransferFunction<'a> {
  analysis: &'a RelevanceAnalysis,
  state: &'a mut RelevanceDomain,
}

impl<'a, 'tcx> Visitor<'tcx> for TransferFunction<'a> {
  // fn visit_statement(&mut self, stmt: &mir::Statement<'tcx>, location: Location) {
  //   self.super_statement(stmt, location);

  //   // When we reach a `StorageDead` statement, we can assume that any pointers to this memory
  //   // are now invalid.
  //   if let StatementKind::StorageDead(local) = stmt.kind {
  //     self.gen_kill.kill(local);
  //   }

  fn visit_assign(&mut self, place: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(place, rvalue, location);

    let lvalue = place.local;

    // Kill defined variables
    let defined_relevant = self.state.remove(lvalue);

    // Add used variables if killed was relevant
    if defined_relevant {
      let mut collector = CollectLocals {
        locals: HybridBitSet::new_empty(self.state.domain_size()),
      };
      collector.visit_rvalue(rvalue, location);
      self.state.union(&collector.locals);
    }
  }

  fn visit_local(&mut self, local: &Local, _context: PlaceContext, location: Location) {
    if self.analysis.slice_set.contains(&(*local, location)) {
      self.state.insert(*local);
    }
  }

  fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_rvalue(rvalue, location);
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
    self.super_terminator(terminator, location);
  }
}

struct RelevanceAnalysis {
  slice_set: SliceSet,
}

impl<'tcx> AnalysisDomain<'tcx> for RelevanceAnalysis {
  type Domain = RelevanceDomain;
  type Direction = Backward;
  const NAME: &'static str = "RelevanceAnslysis";

  fn bottom_value(&self, body: &mir::Body<'tcx>) -> Self::Domain {
    BitSet::new_empty(body.local_decls().len())
    //RelevanceDomain::bottom_value(body)
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {
    // TODO?
  }
}

impl<'tcx> Analysis<'tcx> for RelevanceAnalysis {
  fn apply_statement_effect(
    &self,
    state: &mut Self::Domain,
    statement: &mir::Statement<'tcx>,
    location: Location,
  ) {
    TransferFunction {
      state,
      analysis: self,
    }
    .visit_statement(statement, location);
  }

  fn apply_terminator_effect(
    &self,
    state: &mut Self::Domain,
    terminator: &mir::Terminator<'tcx>,
    location: Location,
  ) {
    TransferFunction {
      state,
      analysis: self,
    }
    .visit_terminator(terminator, location);
  }

  fn apply_call_return_effect(
    &self,
    _state: &mut Self::Domain,
    _block: BasicBlock,
    _func: &mir::Operand<'tcx>,
    _args: &[mir::Operand<'tcx>],
    _return_place: mir::Place<'tcx>,
  ) {
  }
}

struct CollectResults<'a, 'tcx>  {
  body: &'a Body<'tcx>,
  relevant_spans: Vec<Span>
}

impl<'a, 'mir, 'tcx> ResultsVisitor<'mir, 'tcx> for CollectResults<'a, 'tcx> {
  type FlowState = <RelevanceAnalysis as AnalysisDomain<'tcx>>::Domain;

  fn visit_statement_before_primary_effect(
    &mut self,
    state: &Self::FlowState,
    statement: &'mir mir::Statement<'tcx>,
    location: Location,
  ) {
    match &statement.kind {
      StatementKind::Assign(assign) => {
        let (place, rvalue) = &**assign;
        let local = place.local;
        //println!("{:?} {:?} {:?}", state, local, state.contains(local));
        if state.contains(local) {
          let source_info = self.body.source_info(location);
          self.relevant_spans.push(source_info.span);
        }
      }
      _ => {}
    }
  }
}

struct FindInitialSliceSet<'a, 'tcx> {
  slice_span: Span,
  slice_set: HashSet<(Local, Location)>,
  body: &'a Body<'tcx>,
}

impl<'a, 'tcx> Visitor<'tcx> for FindInitialSliceSet<'a, 'tcx> {
  fn visit_local(&mut self, local: &Local, _context: PlaceContext, location: Location) {
    let source_info = self.body.source_info(location);
    let span = source_info.span;

    if self.slice_span.contains(span) {
      self.slice_set.insert((*local, location));
    }
  }
}

#[derive(Serialize)]
struct Range {
  start_line: usize,
  start_col: usize,
  end_line: usize,
  end_col: usize
}

impl Range {
  pub fn from_span(span: Span, source_map: &SourceMap) -> Self {
    let lines = source_map.span_to_lines(span).unwrap();
    let start_line = lines.lines.first().unwrap();
    let end_line = lines.lines.last().unwrap();
    Range {
      start_line: start_line.line_index,
      start_col: start_line.start_col.0,
      end_line: end_line.line_index,
      end_col: end_line.end_col.0
    }
  }
}

#[derive(Serialize)]
struct SliceOutput {
  ranges: Vec<Range>
}

pub fn analyze(tcx: TyCtxt, body_id: &rustc_hir::BodyId, range: ((i32, i32), (i32, i32))) {
  let local_def_id = body_id.hir_id.owner;

  println!("MIR");
  write_mir_pretty(
    tcx,
    Some(local_def_id.to_def_id()),
    &mut io::stdout().lock(),
  )
  .unwrap();
  println!("============");

  // let borrowck_result = tcx.mir_borrowck(local_def_id);
  // println!("{:#?}", borrowck_result);

  let body = tcx.optimized_mir(body_id.hir_id.owner);
  let param_env = tcx.param_env(local_def_id.to_def_id());

  let ((start_line, start_col), (end_line, end_col)) = range;

  let source_map = tcx.sess.source_map();
  let source_file = source_map.lookup_source_file(BytePos(0));
  let start_pos = source_file.line_bounds(start_line as usize).start + BytePos(start_col as u32);
  let end_pos = source_file.line_bounds(end_line as usize).start + BytePos(end_col as u32);
  let slice_span = Span::with_root_ctxt(start_pos, end_pos);

  println!("range {:?}", range);
  println!("start_pos {:?}, end_pos {:?}", start_pos, end_pos);

  let mut finder = FindInitialSliceSet {
    slice_span,
    slice_set: HashSet::new(),
    body,
  };
  finder.visit_body(body);
  println!("Initial slice set: {:?}", finder.slice_set);

  let mut visitor = CollectResults { body, relevant_spans: vec![] };
  let results = RelevanceAnalysis {
    slice_set: finder.slice_set,
  }
  .into_engine(tcx, body)
  .iterate_to_fixpoint()
  .visit_reachable_with(body, &mut visitor);

  let ranges = visitor.relevant_spans.into_iter().map(|span| Range::from_span(span, source_map)).collect();
  let output = SliceOutput { ranges };
  println!("{}", serde_json::to_string(&output).unwrap());
}

//println!("{:?} {:?} {:?} {:?}", place, span.lo(), span.hi(), source_lines.lines);
