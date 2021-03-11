use rustc_middle::mir::{
  self,
  visit::{Visitor},
  *,
};
use rustc_mir::dataflow::{fmt::DebugWithContext, Analysis, AnalysisDomain, JoinSemiLattice};
use rustc_target::abi::VariantIdx;
use std::{
  collections::{HashMap, HashSet},
  fmt,
};

// TODO: represent place without borrowing
// features are
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProjectionPrim {
  Field(Field),
  Downcast(VariantIdx),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PlacePrim {
  local: Local,
  projection: Vec<ProjectionPrim>,
}

impl PlacePrim {
  pub fn from_place(place: Place) -> Option<Self> {
    place.projection.iter().fold(Some(Vec::new()), |acc, elem| {
      acc.and_then(|mut elems| {
        let new_elem = match elem {
          ProjectionElem::Field(field, _) => Some(ProjectionPrim::Field(field)),
          _ => None,
        };

        new_elem.map(move |new_elem| {
          elems.push(new_elem);
          elems
        })
      })
    }).map(|projection| {
      PlacePrim {
        local: place.local,
        projection
      }
    })
  }
}

impl fmt::Debug for PlacePrim {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for _elem in self.projection.iter().rev() {
      write!(f, "(")?;
    }

    write!(f, "{:?}", self.local)?;

    for elem in self.projection.iter() {
      match elem {
        ProjectionPrim::Field(field) => {
          write!(f, ".{:?}", field.index())?;
        }
        ProjectionPrim::Downcast(index) => {
          write!(f, "as {:?}", index)?;
        }
      }
    }

    Ok(())
  }
}

impl PlacePrim {
  pub fn local(local: Local) -> Self {
    PlacePrim {
      local,
      projection: vec![],
    }
  }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PointsToDomain(pub HashMap<PlacePrim, HashSet<PlacePrim>>);

impl PointsToDomain {
  pub fn points_to(&self, place: Place) -> HashSet<PlacePrim> {
    let mut possibly_assigned = HashSet::new();
    possibly_assigned.insert(PlacePrim::local(place.local));

    place
      .iter_projections()
      .fold(possibly_assigned, |acc, (_, projection)| match projection {
        ProjectionElem::Deref => {
          let mut possibly_assigned = HashSet::new();
          for local in acc.iter() {
            possibly_assigned = &possibly_assigned | &self.0[local];
          }
          possibly_assigned
        }
        ProjectionElem::Field(field, _ty) => acc
          .into_iter()
          .map(|mut place| {
            place.projection.push(ProjectionPrim::Field(field));
            place
          })
          .collect(),
        _ => unimplemented!("{:?}", place),
      })
  }

  pub fn add_borrow(&mut self, lplace: Place, rplace: Place) {
    let rprims = self.points_to(rplace);
    for lprim in self.points_to(lplace).into_iter() {
      let lprims = self.0.entry(lprim).or_insert_with(HashSet::new);
      *lprims = &*lprims | &rprims;
    }
  }
}

impl JoinSemiLattice for PointsToDomain {
  fn join(&mut self, other: &Self) -> bool {
    let mut changed = false;
    for (k, v) in other.0.iter() {
      match self.0.get_mut(k) {
        Some(v2) => {
          let orig_len = v2.len();
          *v2 = v | v2;
          if v2.len() != orig_len {
            changed = true;
          }
        }
        None => {
          self.0.insert(k.clone(), v.clone());
          changed = true;
        }
      }
    }
    changed
  }
}

impl fmt::Debug for PointsToDomain {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for (idx, value) in self.0.iter() {
      f.write_fmt(format_args!("{:?}:{:?} ", idx, value))?;
    }

    Ok(())
  }
}

impl<C> DebugWithContext<C> for PointsToDomain {}

struct TransferFunction<'a> {
  analysis: &'a PointsToAnalysis,
  state: &'a mut PointsToDomain,
}

impl<'a, 'tcx> Visitor<'tcx> for TransferFunction<'a> {
  fn visit_assign(&mut self, lplace: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(lplace, rvalue, location);

    match *rvalue {
      Rvalue::Ref(_region, BorrowKind::Mut { .. }, rplace) => {
        self.state.add_borrow(*lplace, rplace);
      }
      _ => {}
    }
  }
}

pub struct PointsToAnalysis;
impl<'tcx> AnalysisDomain<'tcx> for PointsToAnalysis {
  type Domain = PointsToDomain;
  const NAME: &'static str = "PointsToAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    PointsToDomain(HashMap::new())
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {
    // TODO?
  }
}

impl<'tcx> Analysis<'tcx> for PointsToAnalysis {
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
