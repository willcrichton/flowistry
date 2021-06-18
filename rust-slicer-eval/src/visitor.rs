use itertools::iproduct;
use log::debug;
use rust_slicer::analysis::{eval_extensions::REACHED_LIBRARY, intraprocedural, utils};
use rust_slicer::config::{Config, ContextMode, EvalMode, MutabilityMode, PointerMode, Range};
use rustc_ast::{
  token::Token,
  tokenstream::{TokenStream, TokenTree},
};
use rustc_data_structures::{
  fx::FxHashMap as HashMap,
  sync::{par_iter, ParallelIterator},
};
use rustc_hir::{
  itemlikevisit::{ItemLikeVisitor, ParItemLikeVisitor},
  BodyId, ImplItemKind, ItemKind,
};
use rustc_middle::{
  mir::{
    visit::Visitor, Body, HasLocalDecls, Location, Mutability, Place, Terminator, TerminatorKind,
  },
  ty::{Ty, TyCtxt, TyS},
};
use rustc_span::Span;
use serde::Serialize;
use std::cell::RefCell;
use std::sync::{
  atomic::{AtomicUsize, Ordering},
  Mutex,
};
use std::time::Instant;

struct EvalBodyVisitor<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  has_immut_ptr_in_call: bool,
  has_same_type_ptrs_in_call: bool,
  has_same_type_ptrs_in_input: bool,
}

impl EvalBodyVisitor<'_, 'tcx> {
  fn place_ty(&self, place: Place<'tcx>) -> Ty<'tcx> {
    self
      .tcx
      .erase_regions(place.ty(self.body.local_decls(), self.tcx).ty)
  }

  fn any_same_type_ptrs(&self, places: Vec<Place<'tcx>>) -> bool {
    places.iter().enumerate().any(|(i, place)| {
      places
        .iter()
        .enumerate()
        .filter(|(j, _)| i != *j)
        .any(|(_, place2)| TyS::same_type(self.place_ty(*place), self.place_ty(*place2)))
    })
  }
}

impl Visitor<'tcx> for EvalBodyVisitor<'_, 'tcx> {
  fn visit_body(&mut self, body: &Body<'tcx>) {
    self.super_body(body);

    let input_ptrs = body
      .args_iter()
      .map(|local| {
        let place = utils::local_to_place(local, self.tcx);
        utils::interior_pointers(place, self.tcx, self.body).into_iter()
      })
      .flatten()
      .filter_map(|(_, (place, mutability))| (mutability == Mutability::Mut).then(|| place))
      .collect::<Vec<_>>();

    let has_same_type_ptrs = self.any_same_type_ptrs(input_ptrs);
    self.has_same_type_ptrs_in_input |= has_same_type_ptrs;
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    if let TerminatorKind::Call {
      args, destination, ..
    } = &terminator.kind
    {
      let input_ptrs = args
        .iter()
        .filter_map(|operand| utils::operand_to_place(operand))
        .map(|place| utils::interior_pointers(place, self.tcx, self.body).into_iter())
        .flatten()
        .collect::<Vec<_>>();

      let output_ptrs = destination
        .map(|(place, _)| utils::interior_pointers(place, self.tcx, self.body))
        .unwrap_or_else(HashMap::default);

      let all_ptr_places = input_ptrs
        .clone()
        .into_iter()
        .chain(output_ptrs.into_iter())
        .filter_map(|(_, (place, mutability))| (mutability == Mutability::Mut).then(|| place))
        .collect::<Vec<_>>();

      let has_immut_ptr = input_ptrs
        .iter()
        .any(|(_, (_, mutability))| *mutability == Mutability::Not);

      let has_same_type_ptrs = self.any_same_type_ptrs(all_ptr_places);

      self.has_immut_ptr_in_call |= has_immut_ptr;
      self.has_same_type_ptrs_in_call |= has_same_type_ptrs;
    }
  }
}

pub struct EvalCrateVisitor<'tcx> {
  tcx: TyCtxt<'tcx>,
  count: AtomicUsize,
  total: usize,
  pub eval_results: Mutex<Vec<Vec<EvalResult>>>,
}

#[derive(Debug, Serialize)]
pub struct EvalResult {
  mutability_mode: MutabilityMode,
  context_mode: ContextMode,
  pointer_mode: PointerMode,
  sliced_local: usize,
  function_range: Range,
  function_path: String,
  num_instructions: usize,
  num_relevant_instructions: usize,
  num_tokens: usize,
  num_relevant_tokens: usize,
  duration: f64,
  has_immut_ptr_in_call: bool,
  has_same_type_ptrs_in_call: bool,
  has_same_type_ptrs_in_input: bool,
  reached_library: bool,
}

fn flatten_stream(stream: TokenStream) -> Vec<Token> {
  stream
    .into_trees()
    .map(|tree| match tree {
      TokenTree::Token(token) => vec![token].into_iter(),
      TokenTree::Delimited(_, _, stream) => flatten_stream(stream).into_iter(),
    })
    .flatten()
    .collect()
}

impl EvalCrateVisitor<'tcx> {
  pub fn new(tcx: TyCtxt<'tcx>, total: usize) -> Self {
    EvalCrateVisitor {
      tcx,
      count: AtomicUsize::new(1),
      total,
      eval_results: Mutex::new(Vec::new()),
    }
  }

  fn analyze(&self, body_span: Span, body_id: &BodyId) {
    if body_span.from_expansion() {
      return;
    }

    let source_map = self.tcx.sess.source_map();
    let source_file = &source_map.lookup_source_file(body_span.lo());
    if source_file.src.is_none() {
      return;
    }

    let (token_stream, _) =
      rustc_parse::maybe_file_to_stream(&self.tcx.sess.parse_sess, source_file.clone(), None)
        .unwrap();
    let tokens = &flatten_stream(token_stream);

    let local_def_id = self.tcx.hir().body_owner_def_id(*body_id);

    let function_path = &self.tcx.def_path_debug_str(local_def_id.to_def_id());
    let count = self.count.fetch_add(1, Ordering::SeqCst);

    debug!("Visiting {} ({} / {})", function_path, count, self.total);

    let borrowck_result = self.tcx.mir_borrowck(local_def_id);
    let body = &borrowck_result.intermediates.body;
    let locals = body
      .local_decls
      .indices()
      .collect::<Vec<_>>();

    let mut body_visitor = EvalBodyVisitor {
      tcx: self.tcx,
      body,
      has_immut_ptr_in_call: false,
      has_same_type_ptrs_in_call: false,
      has_same_type_ptrs_in_input: false,
    };
    body_visitor.visit_body(body);

    let tcx = self.tcx;
    let has_immut_ptr_in_call = body_visitor.has_immut_ptr_in_call;
    let has_same_type_ptrs_in_input = body_visitor.has_same_type_ptrs_in_input;
    let has_same_type_ptrs_in_call = body_visitor.has_same_type_ptrs_in_call;

    let eval_results = par_iter(locals)
      .map(|local| {
        let source_map = self.tcx.sess.source_map();

        iproduct!(
          vec![MutabilityMode::DistinguishMut, MutabilityMode::IgnoreMut].into_iter(),
          vec![ContextMode::Recurse, ContextMode::SigOnly].into_iter(),
          vec![PointerMode::Precise, PointerMode::Conservative].into_iter()
        )
        .filter_map(move |(mutability_mode, context_mode, pointer_mode)| {
          let config = Config {
            eval_mode: EvalMode {
              mutability_mode,
              context_mode,
              pointer_mode,
            },
            ..Default::default()
          };

          let start = Instant::now();
          let (output, reached_library) = REACHED_LIBRARY.set(RefCell::new(false), || {
            let output = intraprocedural::analyze_function(
              &config,
              tcx,
              *body_id,
              &intraprocedural::SliceLocation::PlacesOnExit(vec![Place {
                local,
                projection: tcx.intern_place_elems(&[]),
              }]),
            )
            .unwrap();
            rust_slicer::analysis::intraprocedural::RESULT_CACHE.with(|result_cache| {
              result_cache.borrow_mut().clear();
            });
            let reached_library =
              REACHED_LIBRARY.get(|reached_library| *reached_library.unwrap().borrow());
            (output, reached_library)
          });

          let num_tokens = tokens.len();
          let slice_spans = output
            .ranges()
            .iter()
            .filter_map(|range| range.to_span(&source_file))
            .collect::<Vec<_>>();
          let num_relevant_tokens = tokens
            .iter()
            .filter(|token| slice_spans.iter().any(|span| span.contains(token.span)))
            .count();

          Some(EvalResult {
            context_mode,
            mutability_mode,
            pointer_mode,
            sliced_local: local.as_usize(),
            function_range: Range::from_span(body_span, source_map).ok()?,
            function_path: function_path.clone(),
            num_instructions: output.num_instructions,
            num_relevant_instructions: output.num_relevant_instructions,
            num_tokens,
            num_relevant_tokens,
            duration: (start.elapsed().as_nanos() as f64) / 10e9,
            has_immut_ptr_in_call,
            has_same_type_ptrs_in_call,
            has_same_type_ptrs_in_input,
            reached_library,
          })
        })
        .collect::<Vec<_>>()
      })
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

pub struct ItemCounter<'tcx> {
  pub tcx: TyCtxt<'tcx>,
  pub count: usize,
}

impl ItemCounter<'_> {
  fn analyze(&mut self, body_span: Span) {
    if body_span.from_expansion() {
      return;
    }

    let source_map = self.tcx.sess.source_map();
    let source_file = &source_map.lookup_source_file(body_span.lo());
    if source_file.src.is_none() {
      return;
    }

    self.count += 1;
  }
}

impl ItemLikeVisitor<'tcx> for ItemCounter<'tcx> {
  fn visit_item(&mut self, item: &'tcx rustc_hir::Item<'tcx>) {
    match &item.kind {
      ItemKind::Fn(_, _, _) => {
        self.analyze(item.span);
      }
      _ => {}
    }
  }

  fn visit_impl_item(&mut self, impl_item: &'tcx rustc_hir::ImplItem<'tcx>) {
    match &impl_item.kind {
      ImplItemKind::Fn(_, _) => {
        self.analyze(impl_item.span);
      }
      _ => {}
    }
  }

  fn visit_trait_item(&mut self, _trait_item: &'tcx rustc_hir::TraitItem<'tcx>) {}

  fn visit_foreign_item(&mut self, _foreign_item: &'tcx rustc_hir::ForeignItem<'tcx>) {}
}
