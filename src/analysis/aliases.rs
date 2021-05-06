use super::intraprocedural::elapsed;
use crate::config::{Config, MutabilityMode};
use log::{debug, warn};
use rustc_data_structures::fx::{FxHashMap as HashMap, FxHashSet as HashSet};
use rustc_data_structures::graph::scc::Sccs;
use rustc_index::{bit_set::BitSet, vec::IndexVec};
use rustc_middle::{
  mir::{
    borrows::BorrowSet,
    regions::{ConstraintSccIndex, OutlivesConstraint},
    *,
  },
  ty::{self, RegionKind, RegionVid, Ty, TyCtxt, TyKind, TyS, TypeFoldable, TypeVisitor},
};
use rustc_target::abi::VariantIdx;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::hash::Hash;
use std::ops::ControlFlow;
use std::time::Instant;

pub type PlaceSet<'tcx> = HashSet<Place<'tcx>>;

pub fn place_set_join(this: &mut PlaceSet<'tcx>, other: &PlaceSet<'tcx>) -> bool {
  if other.is_subset(this) {
    false
  } else {
    this.extend(other.iter());
    true
  }
}

struct CollectRegions<'tcx> {
  tcx: TyCtxt<'tcx>,
  local: Local,
  place_stack: Vec<PlaceElem<'tcx>>,
  ty_stack: Vec<Ty<'tcx>>,
  regions: HashMap<RegionVid, (Place<'tcx>, Mutability)>,
}

impl TypeVisitor<'tcx> for CollectRegions<'tcx> {
  fn visit_ty(&mut self, ty: Ty<'tcx>) -> ControlFlow<Self::BreakTy> {
    if self
      .ty_stack
      .iter()
      .any(|visited_ty| TyS::same_type(ty, visited_ty))
    {
      return ControlFlow::Continue(());
    }

    self.ty_stack.push(ty);

    match ty.kind() {
      TyKind::Tuple(fields) => {
        for (i, field) in fields.iter().enumerate() {
          self.place_stack.push(ProjectionElem::Field(
            Field::from_usize(i),
            field.expect_ty(),
          ));
          field.super_visit_with(self);
          self.place_stack.pop();
        }
      }

      TyKind::Adt(adt_def, subst) => match adt_def.adt_kind() {
        ty::AdtKind::Struct => {
          for (i, field) in adt_def.all_fields().enumerate() {
            let ty = field.ty(self.tcx, subst);
            self
              .place_stack
              .push(ProjectionElem::Field(Field::from_usize(i), ty));
            self.visit_ty(ty);
            self.place_stack.pop();
          }
        }
        ty::AdtKind::Union => {
          // unsafe, so ignore
        }
        ty::AdtKind::Enum => {
          for (i, variant) in adt_def.variants.iter().enumerate() {
            let variant_index = VariantIdx::from_usize(i);
            let cast = PlaceElem::Downcast(
              Some(adt_def.variants[variant_index].ident.name),
              variant_index,
            );
            self.place_stack.push(cast);
            for (j, field) in variant.fields.iter().enumerate() {
              let ty = field.ty(self.tcx, subst);
              let field = ProjectionElem::Field(Field::from_usize(j), ty);
              self.place_stack.push(field);
              self.visit_ty(ty);
              self.place_stack.pop();
            }
            self.place_stack.pop();
          }
        }
      },

      TyKind::Array(elem_ty, _) | TyKind::Slice(elem_ty) => {
        self
          .place_stack
          .push(ProjectionElem::Index(Local::from_usize(0)));
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Ref(region, elem_ty, _) => {
        self.visit_region(region);
        self.place_stack.push(ProjectionElem::Deref);
        self.visit_ty(elem_ty);
        self.place_stack.pop();
      }

      TyKind::Closure(_, substs) => {
        self.visit_ty(substs.as_closure().tupled_upvars_ty());
      }

      TyKind::RawPtr(_)
      | TyKind::Projection(_)
      | TyKind::FnDef(_, _)
      | TyKind::FnPtr(_)
      | TyKind::Opaque(_, _)
      | TyKind::Foreign(_)
      | TyKind::Dynamic(_, _)
      | TyKind::Never => {}

      _ if ty.is_primitive_ty() => {}

      _ => {
        warn!("unimplemented {:?} ({:?})", ty, ty.kind());
      }
    };

    self.ty_stack.pop();
    ControlFlow::Continue(())
  }

  fn visit_region(&mut self, region: ty::Region<'tcx>) -> ControlFlow<Self::BreakTy> {
    match region {
      RegionKind::ReVar(region) => {
        let mutability = if self
          .ty_stack
          .iter()
          .any(|ty| ty.is_ref() && ty.ref_mutability().unwrap() == Mutability::Not)
        {
          Mutability::Not
        } else {
          Mutability::Mut
        };

        let place = Place {
          local: self.local,
          projection: self.tcx.intern_place_elems(&self.place_stack),
        };
        self.regions.insert(*region, (place, mutability));
      }
      RegionKind::ReStatic => {}
      _ => unreachable!("{:?}: {:?}", self.ty_stack.first().unwrap(), region),
    };

    ControlFlow::Continue(())
  }
}

pub(super) fn interior_pointers<'tcx>(
  place: Place<'tcx>,
  tcx: TyCtxt<'tcx>,
  body: &Body<'tcx>,
) -> HashMap<RegionVid, (Place<'tcx>, Mutability)> {
  let ty = place.ty(body.local_decls(), tcx).ty;
  let mut region_collector = CollectRegions {
    tcx,
    local: place.local,
    place_stack: vec![],
    ty_stack: Vec::new(),
    regions: HashMap::default(),
  };
  region_collector.visit_ty(ty);
  region_collector.regions
}

pub struct Aliases<'tcx> {
  loans: IndexVec<RegionVid, PlaceSet<'tcx>>,
  loan_cache: RefCell<HashMap<Place<'tcx>, PlaceSet<'tcx>>>,
}

#[derive(PartialEq, Eq, Debug)]
pub enum PlaceRelation {
  Super,
  Sub,
  Disjoint,
}

impl PlaceRelation {
  pub fn overlaps(&self) -> bool {
    *self != PlaceRelation::Disjoint
  }
}

pub fn place_relation(part_place: Place<'tcx>, whole_place: Place<'tcx>) -> PlaceRelation {
  let locals_match = part_place.local == whole_place.local;

  let projections_match = part_place
    .projection
    .iter()
    .zip(whole_place.projection.iter())
    .all(|(elem1, elem2)| {
      use ProjectionElem::*;
      match (elem1, elem2) {
        (Deref, Deref) => true,
        (Field(f1, _), Field(f2, _)) => f1 == f2,
        (Index(_), Index(_)) => true,
        (ConstantIndex { .. }, ConstantIndex { .. }) => true,
        (Subslice { .. }, Subslice { .. }) => true,
        (Downcast(_, v1), Downcast(_, v2)) => v1 == v2,
        _ => false,
      }
    });

  let is_sub_part = part_place.projection.len() >= whole_place.projection.len();

  if locals_match && projections_match {
    if is_sub_part {
      PlaceRelation::Sub
    } else {
      PlaceRelation::Super
    }
  } else {
    PlaceRelation::Disjoint
  }
}

impl Aliases<'tcx> {
  pub fn loans(&self, place: Place<'tcx>) -> PlaceSet<'tcx> {
    let compute_loans = || {
      self
        .loans
        .iter()
        .filter_map(|loans| {
          let matches_loan = loans
            .iter()
            .any(|loan| place_relation(*loan, place).overlaps());
          let is_deref = place.is_indirect();
          (matches_loan && is_deref).then(|| loans.clone().into_iter())
        })
        .flatten()
        .chain(vec![place].into_iter())
        .collect::<HashSet<_>>()
    };

    self
      .loan_cache
      .borrow_mut()
      .entry(place)
      .or_insert_with(compute_loans)
      .clone()
  }
}

pub fn compute_aliases(
  config: &Config,
  tcx: TyCtxt<'tcx>,
  body: &'a Body<'tcx>,
  borrow_set: &'a BorrowSet<'tcx>,
  outlives_constraints: &'a Vec<OutlivesConstraint>,
  constraint_sccs: &'a Sccs<RegionVid, ConstraintSccIndex>,
) -> Aliases<'tcx> {
  let all_regions = body
    .local_decls()
    .indices()
    .map(|local| {
      let place = Place {
        local,
        projection: tcx.intern_place_elems(&[]),
      };

      interior_pointers(place, tcx, body)
    })
    .fold(HashMap::default(), |mut h1, h2| {
      h1.extend(h2);
      h1
    });

  let start = Instant::now();
  let max_region = all_regions
    .keys()
    .map(|region| region.as_usize())
    .max()
    .unwrap_or(0)
    + 1;

  let regions_in_constraint = outlives_constraints
    .iter()
    .map(|constraint| vec![constraint.sup, constraint.sub].into_iter())
    .flatten()
    .collect::<HashSet<_>>();
  let mut regions_in_scc =
    IndexVec::from_elem_n(BitSet::new_empty(max_region), constraint_sccs.num_sccs());
  for region in 0..max_region {
    let region = RegionVid::from_usize(region);
    if regions_in_constraint.contains(&region) {
      let scc = constraint_sccs.scc(region);
      regions_in_scc[scc].insert(region);
    }
  }

  let root_region = RegionVid::from_usize(0);
  let root_scc = constraint_sccs.scc(root_region);
  let region_ancestors = compute_region_ancestors(constraint_sccs, &regions_in_scc, root_scc);
  debug!("region ancestors: {:#?}", region_ancestors);

  let mut aliases = Aliases {
    loans: IndexVec::from_elem_n(HashSet::default(), max_region),
    loan_cache: RefCell::new(HashMap::default()),
  };

  for (region, (sub_place, mutability)) in all_regions {
    if mutability == Mutability::Mut
      || config.eval_mode.mutability_mode == MutabilityMode::IgnoreMut
    {
      aliases.loans[region].insert(tcx.mk_place_deref(sub_place));
    }
  }

  for (_, borrow) in borrow_set.iter_enumerated() {
    let mutability = borrow.kind.to_mutbl_lossy();
    if mutability == Mutability::Mut
      || config.eval_mode.mutability_mode == MutabilityMode::IgnoreMut
    {
      aliases.loans[borrow.region].insert(borrow.borrowed_place);
    }
  }

  elapsed("Alias setup", start);

  debug!("initial aliases {:#?}", aliases.loans);

  let start = Instant::now();
  loop {
    let mut changed = false;
    let prev_aliases = aliases.loans.clone();
    for (region, places) in aliases.loans.iter_enumerated_mut() {
      let alias_places = region_ancestors
        .get(&region)
        .map(|sccs| {
          let alias_regions = sccs
            .iter()
            .map(|scc_index| regions_in_scc[scc_index].iter())
            .flatten();

          alias_regions
            .filter_map(|region| {
              prev_aliases
                .get(region)
                .map(|places| places.clone().into_iter())
            })
            .flatten()
            .collect::<HashSet<_>>()
        })
        .unwrap_or_else(HashSet::default);

      let n = places.len();
      places.extend(alias_places.into_iter());
      changed = places.len() > n;
    }

    // TODO: needed for parity with Oxide, but doesn't seem to be needed for actual correctness?
    // for (_, borrow) in borrow_set.iter_enumerated() {
    //   let place = borrow.borrowed_place;
    //   if let Some(ProjectionElem::Deref) = place.projection.first() {
    //     let outer_projection = &place.projection[1..];
    //     let ty = body.local_decls()[place.local].ty;
    //     if let TyKind::Ref(RegionKind::ReVar(region), _, _) = ty.kind() {
    //       let new_loans = aliases.loans[*region].clone().into_iter().map(|loan| {
    //         let mut projection = loan.projection.to_vec();
    //         projection.extend_from_slice(outer_projection);
    //         Place {
    //           local: loan.local,
    //           projection: tcx.intern_place_elems(&projection),
    //         }
    //       });
    //       aliases
    //         .loans
    //         .get_mut(borrow.region)
    //         .unwrap()
    //         .extend(new_loans);
    //     }
    //   }
    // }

    if !changed {
      break;
    }
  }

  for v in aliases.loans.iter_mut() {
    *v = v.iter().map(|place| tcx.erase_regions(*place)).collect();
  }
  elapsed("Alias compute", start);

  debug!(
    "Aliases: {}",
    aliases
      .loans
      .iter_enumerated()
      .map(|(region, places)| format!("{:?}: {:?}", region, places))
      .collect::<Vec<_>>()
      .join(", ")
  );

  aliases
}

fn hashmap_merge<K: Eq + Hash, V>(
  mut h1: HashMap<K, V>,
  h2: HashMap<K, V>,
  conflict: impl Fn(&mut V, V),
) -> HashMap<K, V> {
  for (k, v) in h2.into_iter() {
    match h1.entry(k) {
      Entry::Vacant(entry) => {
        entry.insert(v);
      }
      Entry::Occupied(mut entry) => {
        let entry = entry.get_mut();
        conflict(entry, v);
      }
    }
  }
  h1
}

fn compute_region_ancestors(
  sccs: &Sccs<RegionVid, ConstraintSccIndex>,
  regions_in_scc: &IndexVec<ConstraintSccIndex, BitSet<RegionVid>>,
  node: ConstraintSccIndex,
) -> HashMap<RegionVid, BitSet<ConstraintSccIndex>> {
  let new_set = || BitSet::new_empty(sccs.num_sccs());
  let set_merge = |s1: &mut BitSet<ConstraintSccIndex>, s2: BitSet<ConstraintSccIndex>| {
    s1.union(&s2);
  };

  let mut initial_set = new_set();
  initial_set.insert(node);

  let mut initial_map = HashMap::default();
  for r in regions_in_scc[node].iter() {
    initial_map.insert(r, initial_set.clone());
  }

  sccs
    .successors(node)
    .iter()
    .map(|child| {
      let in_child = regions_in_scc[*child]
        .iter()
        .map(|r| (r, initial_set.clone()))
        .collect::<HashMap<_, _>>();
      let grandchildren = compute_region_ancestors(sccs, regions_in_scc, *child)
        .into_iter()
        .map(|(region, mut parents)| {
          parents.insert(node);
          (region, parents)
        })
        .collect::<HashMap<_, _>>();
      hashmap_merge(in_child, grandchildren, set_merge)
    })
    .fold(initial_map, |h1, h2| hashmap_merge(h1, h2, set_merge))
}
