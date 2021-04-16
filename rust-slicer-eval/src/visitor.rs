use itertools::iproduct;
use log::debug;
use rust_slicer::config::{Config, ContextMode, EvalMode, MutabilityMode, PointerMode, Range};
use rustc_hir::{
  intravisit::{self, NestedVisitorMap, Visitor},
  itemlikevisit::ParItemLikeVisitor,
  BodyId, Expr, ExprKind, ImplItemKind, ItemKind, Local,
};
use rustc_middle::{
  hir::map::Map,
  ty::{TyCtxt},
};
use rustc_span::{Span};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Instant;

// struct EvalBodyVisitor<'tcx> {
//   tcx: TyCtxt<'tcx>,
//   spans: Vec<Span>,
//   body_span: Span,
// }

// impl EvalBodyVisitor<'_> {
//   fn add_span(&mut self, span: Span) {
//     if self.body_span.contains(span) {
//       self.spans.push(span);
//     }
//   }
// }

// impl Visitor<'tcx> for EvalBodyVisitor<'tcx> {
//   type Map = Map<'tcx>;

//   fn nested_visit_map(&mut self) -> NestedVisitorMap<Self::Map> {
//     NestedVisitorMap::OnlyBodies(self.tcx.hir())
//   }

//   fn visit_local(&mut self, local: &'tcx Local<'tcx>) {
//     intravisit::walk_local(self, local);
//     self.add_span(local.span);
//   }

//   fn visit_expr(&mut self, ex: &'tcx Expr<'tcx>) {
//     intravisit::walk_expr(self, ex);

//     match ex.kind {
//       ExprKind::Assign(_, _, _) | ExprKind::AssignOp(_, _, _) => {
//         self.add_span(ex.span);
//       }
//       _ => {}
//     }
//   }
// }

pub struct EvalCrateVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  pub eval_results: Mutex<Vec<EvalResult>>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  mutability_mode: MutabilityMode,
  context_mode: ContextMode,
  pointer_mode: PointerMode,
  slice: Range,
  function_range: Range,
  function_path: String,
  output: Vec<Range>,
  num_instructions: usize,
  num_relevant_instructions: usize,
  num_tokens: usize,
  num_relevant_tokens: usize,
  duration: f64,
}

use rustc_ast::{token::Token, tokenstream::{TokenTree, TokenStream}};
fn flatten_stream(stream: TokenStream) -> Vec<Token> {
  stream.into_trees().map(|tree| {
    match tree {
      TokenTree::Token(token) => vec![token].into_iter(),
      TokenTree::Delimited(_, _, stream) => flatten_stream(stream).into_iter()
    }
  }).flatten().collect()
}

impl EvalCrateVisitor<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>) -> Self {
    EvalCrateVisitor {
      tcx,
      eval_results: Mutex::new(Vec::new()),
    }
  }

  fn analyze(&self, body_span: Span, body_id: &BodyId) {
    let source_map = self.tcx.sess.source_map();
    let source_file = &source_map.lookup_source_file(body_span.lo());
    if source_file.src.is_none() {
      return;
    }

    let (token_stream, _) = rustc_parse::maybe_file_to_stream(&self.tcx.sess.parse_sess, source_file.clone(), None).unwrap();
    let tokens = &flatten_stream(token_stream);

    let local_def_id = self.tcx.hir().body_owner_def_id(*body_id)
    let function_path = &self.tcx.def_path_debug_str(local_def_id.to_def_id());
    debug!("Visiting {}", function_path);

    // let body = self.tcx.hir().body(*body_id);
    // let mut body_visitor = EvalBodyVisitor {
    //   tcx: self.tcx,
    //   spans: Vec::new(),
    //   body_span
    // };
    // body_visitor.visit_expr(&body.value);
    // let body_spans = body_visitor.spans.into_iter();
    
    let borrowck_result = tcx.mir_borrowck(local_def_id);
    let body = &borrowck_result.intermediates.body;
    let return_locations = body.basic_blocks().iter_enumerated().filter_map(|(block, bb_data)| {
      if let TerminatorKind::Return = bb_data.terminator().kind {
        let statement_index = bb_data.statements.len();
        Some(Location { block, statement_index })
      } else {
        None
      }
    }).collect::<Vec<_>>();
    let body_spans = body.local_decls().indices().map(|local| {
      
    });


    let eval_results = body_spans.map(|span| {
        let source_map = self.tcx.sess.source_map();
        let tcx = self.tcx;

        iproduct!(
          vec![MutabilityMode::DistinguishMut, MutabilityMode::IgnoreMut].into_iter(),
          vec![ContextMode::Recurse, ContextMode::SigOnly].into_iter(),
          vec![PointerMode::Precise, PointerMode::Conservative].into_iter()
        )
        .filter_map(move |(mutability_mode, context_mode, pointer_mode)| {
          let config = Config {
            range: Range::from_span(span, source_map).ok()?,
            debug: false,
            eval_mode: EvalMode {
              mutability_mode,
              context_mode,
              pointer_mode,
            },
          };

          let start = Instant::now();
          let (output, _) = rust_slicer::analysis::intraprocedural::analyze_function(
            &config,
            tcx,
            *body_id,
            Some(span),
            Vec::new(),
          )
          .unwrap();

          let num_tokens = tokens.len();
          let slice_spans = output.ranges().iter().filter_map(|range| range.to_span(&source_file)).collect::<Vec<_>>();
          let num_relevant_tokens = tokens.iter().filter(|token| {
            slice_spans.iter().any(|span| span.contains(token.span))
          }).count();

          Some(EvalResult {
            context_mode,
            mutability_mode,
            pointer_mode,
            slice: config.range,
            function_range: Range::from_span(body_span, source_map).ok()?,
            function_path: function_path.clone(),
            output: output.ranges().to_vec(),
            num_instructions: output.num_instructions,
            num_relevant_instructions: output.num_relevant_instructions,
            num_tokens,
            num_relevant_tokens,
            duration: (start.elapsed().as_nanos() as f64) / 10e9,
          })
        })
      })
      .flatten()
      .collect::<Vec<_>>();

    self
      .eval_results
      .lock()
      .unwrap()
      .extend(eval_results.into_iter());
  }
}

impl ParItemLikeVisitor<'tcx> for EvalCrateVisitor<'tcx> {
  fn visit_item(&self, item: &'tcx rustc_hir::Item<'tcx>) {
    match &item.kind {
      ItemKind::Fn(_, _, body_id) => {
        self.analyze(item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_impl_item(&self, impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
    match &impl_item.kind {
      ImplItemKind::Fn(_, body_id) => {
        self.analyze(impl_item.span, body_id);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {}

  fn visit_foreign_item(&self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {}
}
