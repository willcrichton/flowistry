use df::{AnalysisDomain, JoinSemiLattice};
use log::debug;
use petgraph::graph::DiGraph;
use rustc_borrowck::consumers::{
  places_conflict, BodyWithBorrowckFacts, PlaceConflictBias,
};
use rustc_data_structures::graph::{self as rustc_graph, WithStartNode};
use rustc_hash::{FxHashMap, FxHashSet};
use rustc_hir::BodyId;
use rustc_index::IndexVec;
use rustc_middle::{
  mir::{visit::Visitor, BasicBlock, Body, Location, Place, TerminatorKind},
  ty::TyCtxt,
};
use rustc_mir_dataflow::{self as df};
use rustc_utils::{mir::control_dependencies::ControlDependencies, BodyExt, PlaceExt};

use super::graph::{DepEdge, DepGraph, DepNode, LocationOrStart};
use crate::{
  infoflow::mutation::{ModularMutationVisitor, Mutation, MutationStatus},
  mir::placeinfo::PlaceInfo,
};

#[derive(PartialEq, Eq, Default, Clone)]
pub struct PartialGraph<'tcx> {
  edges: FxHashSet<(DepNode<'tcx>, DepNode<'tcx>, DepEdge)>,
  last_mutation: FxHashMap<Place<'tcx>, FxHashSet<LocationOrStart>>,
}

impl<'tcx> df::JoinSemiLattice for PartialGraph<'tcx> {
  fn join(&mut self, other: &Self) -> bool {
    let orig_len = self.edges.len();
    self.edges.extend(&other.edges);
    let changed1 = self.edges.len() != orig_len;

    let mut changed2 = false;
    for (place, other_locs) in &other.last_mutation {
      let self_locs = self.last_mutation.entry(*place).or_default();
      let orig_len = self_locs.len();
      self_locs.extend(other_locs);
      changed2 |= orig_len != self_locs.len();
    }

    changed1 || changed2
  }
}

pub struct GraphConstructor<'a, 'tcx> {
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  place_info: PlaceInfo<'a, 'tcx>,
  control_dependencies: ControlDependencies<BasicBlock>,
  start_loc: FxHashSet<LocationOrStart>,
}

impl<'a, 'tcx> GraphConstructor<'a, 'tcx> {
  pub fn new(
    tcx: TyCtxt<'tcx>,
    body_id: BodyId,
    body_with_facts: &'a BodyWithBorrowckFacts<'tcx>,
  ) -> Self {
    let body = &body_with_facts.body;
    let def_id = tcx.hir().body_owner_def_id(body_id).to_def_id();
    let place_info = PlaceInfo::build(tcx, def_id, body_with_facts);
    let control_dependencies = body.control_dependencies();

    let mut start_loc = FxHashSet::default();
    start_loc.insert(LocationOrStart::Start);

    GraphConstructor {
      tcx,
      body,
      place_info,
      control_dependencies,
      start_loc,
    }
  }

  fn add_input_to_op(
    &self,
    graph: &mut PartialGraph<'tcx>,
    input: Place<'tcx>,
    op: DepNode<'tcx>,
  ) {
    let aliases = self.place_info.aliases(input);
    let provenance = input.refs_in_projection().flat_map(|(place_ref, _)| {
      self
        .place_info
        .aliases(Place::from_ref(place_ref, self.tcx))
        .iter()
    });

    for alias in aliases.iter().chain(provenance).copied() {
      let conflicts = graph
        .last_mutation
        .keys()
        .filter(|place| {
          places_conflict(
            self.tcx,
            self.body,
            **place,
            alias,
            PlaceConflictBias::Overlap,
          )
        })
        .map(|place| (*place, &graph.last_mutation[place]));

      let alias_last_mut = if alias.is_arg(self.body) {
        Some((alias, &self.start_loc))
      } else {
        None
      };

      for (conflict, last_mut) in conflicts.chain(alias_last_mut) {
        for loc in last_mut {
          debug!("mutation from {conflict:?} at {loc:?} to op");
          let input_node = DepNode::Place {
            place: conflict,
            at: *loc,
          };
          graph.edges.insert((input_node, op, DepEdge::Data));
        }
      }
    }
  }

  fn add_op_to_mutated(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    op_node: DepNode<'tcx>,
    Mutation {
      mutated, status, ..
    }: Mutation<'tcx>,
  ) {
    let dsts = self.place_info.aliases(mutated);
    for dst in dsts {
      let dst_node = DepNode::Place {
        place: *dst,
        at: LocationOrStart::Location(location),
      };
      state.edges.insert((op_node, dst_node, DepEdge::Data));

      let dst_mutations = state.last_mutation.entry(*dst).or_default();
      if dsts.len() == 1 && matches!(status, MutationStatus::Definitely) {
        dst_mutations.clear();
      }
      dst_mutations.insert(LocationOrStart::Location(location));
    }
  }

  fn apply_mutations(
    &self,
    state: &mut PartialGraph<'tcx>,
    location: Location,
    mutations: Vec<Mutation<'tcx>>,
  ) {
    let op_node = DepNode::Op(location);

    if let Some(ctrl_deps) = self.control_dependencies.dependent_on(location.block) {
      let ctrl_edges = ctrl_deps.iter().map(|block| {
        let ctrl_node = DepNode::Op(self.body.terminator_loc(block));
        (ctrl_node, op_node, DepEdge::Control)
      });
      state.edges.extend(ctrl_edges);
    }

    for mutation in mutations {
      for input in &mutation.inputs {
        self.add_input_to_op(state, *input, op_node);
      }

      self.add_op_to_mutated(state, location, op_node, mutation);
    }
  }

  fn visit_basic_block(&self, block: BasicBlock, state: &mut PartialGraph<'tcx>) {
    ModularMutationVisitor::new(&self.place_info, |location, mutations| {
      self.apply_mutations(state, location, mutations)
    })
    .visit_basic_block_data(block, &self.body.basic_blocks[block]);

    let terminator = self.body.basic_blocks[block].terminator();
    if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind {
      if let Some(place) = discr.place() {
        self.add_input_to_op(state, place, DepNode::Op(self.body.terminator_loc(block)));
      }
    }
  }

  fn domain_to_petgraph(&self, domain: PartialGraph<'tcx>) -> DepGraph<'tcx> {
    let mut graph: DiGraph<DepNode<'tcx>, DepEdge> = DiGraph::new();
    let mut nodes = FxHashMap::default();
    macro_rules! add_node {
      ($n:expr) => {
        *nodes.entry($n).or_insert_with(|| graph.add_node($n))
      };
    }
    for (src, dst, kind) in domain.edges {
      let src_idx = add_node!(src);
      let dst_idx = add_node!(dst);
      graph.add_edge(src_idx, dst_idx, kind);
    }

    DepGraph { graph }
  }

  pub fn construct(&self) -> DepGraph<'tcx> {
    let bb_graph = &self.body.basic_blocks;
    let blocks =
      rustc_graph::iterate::reverse_post_order(bb_graph, bb_graph.start_node());

    let bot = self.bottom_value(self.body);
    let mut domains = IndexVec::<BasicBlock, _>::from_elem_n(bot, bb_graph.len());

    for block in blocks {
      for parent in bb_graph.predecessors()[block].iter() {
        let (child, parent) = domains.pick2_mut(block, *parent);
        child.join(parent);
      }

      self.visit_basic_block(block, &mut domains[block]);
    }

    let mut all_returns = self.body.all_returns();
    let Some(first_return) = all_returns.next() else {
      todo!()
    };
    for other_return in all_returns {
      let (first, other) = domains.pick2_mut(first_return.block, other_return.block);
      first.join(other);
    }

    let domain = domains[first_return.block].clone();
    self.domain_to_petgraph(domain)
  }
}

impl<'a, 'tcx> df::AnalysisDomain<'tcx> for GraphConstructor<'a, 'tcx> {
  type Domain = PartialGraph<'tcx>;
  type Direction = df::Forward;
  const NAME: &'static str = "GraphConstructor";

  fn bottom_value(&self, _body: &Body<'tcx>) -> Self::Domain {
    PartialGraph::default()
  }

  fn initialize_start_block(&self, _body: &Body<'tcx>, _state: &mut Self::Domain) {}
}
