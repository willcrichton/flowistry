//! The core information flow analysis.
//!
//! The main function is [`compute_flow`]. See [`FlowResults`] and [`FlowDomain`] for an explanation
//! of what it returns.

use std::cell::RefCell;

use log::debug;
use rustc_borrowck::BodyWithBorrowckFacts;
use rustc_hir::BodyId;
use rustc_middle::ty::TyCtxt;
use rustc_utils::{block_timer, BodyExt};

pub use self::{
  analysis::{FlowAnalysis, FlowDomain},
  dependencies::{compute_dependencies, compute_dependency_spans, Direction},
};
use crate::mir::{aliases::Aliases, engine};

mod analysis;
mod dependencies;
pub mod mutation;
mod recursive;

/// The output of the information flow analysis.
///
/// Using the metavariables in [the paper](https://arxiv.org/abs/2111.13662): for each
/// [`LocationOrArg`](crate::indexed::impls::LocationOrArg) $\ell$ in a [`Body`](rustc_middle::mir::Body) $f$,
/// this type contains a [`FlowDomain`] $\Theta_\ell$ that maps from a [`Place`](rustc_middle::mir::Place) $p$
/// to a [`LocationOrArgSet`](crate::indexed::impls::LocationOrArgSet) $\kappa$. The domain of $\Theta_\ell$
/// is all places that have been defined up to $\ell$. For each place, $\Theta_\ell(p)$ contains the set of locations
/// (or arguments) that could influence the value of that place, i.e. the place's dependencies.
///
/// For example, to get the dependencies of the first argument at the first instruction, that would be:
/// ```
/// # #![feature(rustc_private)]
/// # extern crate rustc_middle;
/// # use rustc_middle::{ty::TyCtxt, mir::{Place, Location, Local}};
/// # use flowistry::{infoflow::{FlowDomain, FlowResults}, indexed::impls::LocationOrArgSet};
/// # use rustc_utils::PlaceExt;
/// fn example<'tcx>(tcx: TyCtxt<'tcx>, results: &FlowResults<'_, 'tcx>) {
///   let ℓ: Location            = Location::START;
///   let Θ: &FlowDomain         = results.state_at(ℓ);
///   let p: Place               = Place::make(Local::from_usize(1), &[], tcx);
///   let κ: LocationOrArgSet<_> = Θ.row_set(p);
///   for ℓ2 in κ.iter() {
///     println!("at location {ℓ:?}, place {p:?} depends on location {ℓ2:?}");
///   }
/// }
/// ```
///
/// To access a [`FlowDomain`] for a given location, use the method [`AnalysisResults::state_at`](engine::AnalysisResults::state_at).
/// See [`FlowDomain`] for more on how to access the location set for a given place.
///
/// **Note:** this analysis uses rustc's [dataflow analysis framework](https://rustc-dev-guide.rust-lang.org/mir/dataflow.html),
/// i.e. [`rustc_mir_dataflow`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_mir_dataflow/index.html).
/// You will see several types and traits from that crate here, such as
/// [`Analysis`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_mir_dataflow/trait.Analysis.html) and
/// [`AnalysisDomain`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_mir_dataflow/trait.AnalysisDomain.html).
/// However, for performance purposes, several constructs were reimplemented within Flowistry, such as [`AnalysisResults`](engine::AnalysisResults)
/// which replaces [`rustc_mir_dataflow::Results`](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_mir_dataflow/struct.Results.html).
pub type FlowResults<'a, 'tcx> = engine::AnalysisResults<'tcx, FlowAnalysis<'a, 'tcx>>;

thread_local! {
  pub(super) static BODY_STACK: RefCell<Vec<BodyId>> =
    RefCell::new(Vec::new());
}

/// Computes information flow for a MIR body.
///
/// See [example.rs](https://github.com/willcrichton/flowistry/tree/master/crates/flowistry/examples/example.rs)
/// for a complete example of how to call this function.
///
/// To get a `BodyWithBorrowckFacts`, you can use the
/// [`get_body_with_borrowck_facts`](crate::mir::borrowck_facts::get_body_with_borrowck_facts)
/// function.
///
/// See [`FlowResults`] for an explanation of how to use the return value.
pub fn compute_flow<'a, 'tcx>(
  tcx: TyCtxt<'tcx>,
  body_id: BodyId,
  body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
) -> FlowResults<'a, 'tcx> {
  BODY_STACK.with(|body_stack| {
    body_stack.borrow_mut().push(body_id);
    debug!(
      "{}",
      rustc_hir_pretty::to_string(rustc_hir_pretty::NO_ANN, |s| s
        .print_expr(tcx.hir().body(body_id).value))
    );
    debug!("{}", body_with_facts.body.to_string(tcx).unwrap());

    let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
    let aliases = Aliases::build(tcx, def_id, body_with_facts);
    let location_domain = aliases.location_domain().clone();

    let body = &body_with_facts.body;

    let results = {
      block_timer!("Flow");

      let analysis = FlowAnalysis::new(tcx, def_id, body, aliases);
      engine::iterate_to_fixpoint(tcx, body, location_domain, analysis)
      // analysis.into_engine(tcx, body).iterate_to_fixpoint()
    };

    if log::log_enabled!(log::Level::Info) {
      let counts = body
        .all_locations()
        .flat_map(|loc| {
          let state = results.state_at(loc);
          state
            .rows()
            .map(|(_, locations)| locations.len())
            .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

      let nloc = body.all_locations().count();
      let np = counts.len();
      let pavg = np as f64 / (nloc as f64);
      let nl = counts.into_iter().sum::<usize>();
      let lavg = nl as f64 / (nloc as f64);
      log::info!(
        "Over {nloc} locations, total number of place entries: {np} (avg {pavg:.0}/loc), total size of location sets: {nl} (avg {lavg:.0}/loc)",
      );
    }

    if std::env::var("DUMP_MIR").is_ok()
      && BODY_STACK.with(|body_stack| body_stack.borrow().len() == 1)
    {
      todo!()
      // utils::dump_results(body, &results, def_id, tcx).unwrap();
    }

    body_stack.borrow_mut().pop();

    results
  })
}
