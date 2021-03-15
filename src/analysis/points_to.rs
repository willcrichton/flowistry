use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{self, tcx::PlaceTy, visit::Visitor, *},
  ty::{ParamEnv, TyCtxt, TyKind},
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
  pub fn ty<'tcx>(
    &self,
    local_decls: &impl HasLocalDecls<'tcx>,
    tcx: TyCtxt<'tcx>,
  ) -> PlaceTy<'tcx> {
    let ty = local_decls.local_decls()[self.local].ty;
    self
      .projection
      .iter()
      .fold(PlaceTy::from_ty(ty), |place_ty, prim| {
        let elem = match prim {
          ProjectionPrim::Field(field) => ProjectionElem::Field(*field, ()),
          ProjectionPrim::Downcast(idx) => ProjectionElem::Downcast(None, *idx),
        };

        let place_ty = match place_ty.ty.kind() {
          // If type is [closure@...] then this is actually referring to the upvars
          TyKind::Closure(_def, substs) => PlaceTy::from_ty(substs.as_closure().tupled_upvars_ty()),
          _ => place_ty,
        };

        place_ty.projection_ty_core(tcx, ParamEnv::empty(), &elem, |ty, field, _| {
          ty.field_ty(tcx, field)
        })
      })
  }

  pub fn sub_places<'tcx>(
    &self,
    local_decls: &impl HasLocalDecls<'tcx>,
    tcx: TyCtxt<'tcx>,
    module: DefId,
  ) -> HashSet<PlacePrim> {
    use TyKind::*;
    let place_ty = self.ty(local_decls, tcx);
    let ty = place_ty.ty;

    let map_fields = |place: &PlacePrim, fields: Vec<usize>| {
      fields
        .into_iter()
        .map(|i| {
          let mut place = place.clone();
          place
            .projection
            .push(ProjectionPrim::Field(Field::from_usize(i)));
          place.sub_places(local_decls, tcx, module)
        })
        .fold(HashSet::new(), |s1, s2| &s1 | &s2)
    };

    let mut places: HashSet<_> = match ty.kind() {
      Tuple(tys) => map_fields(self, (0..tys.types().count()).collect()),

      Adt(def, _) => {
        def
          .variants
          .iter_enumerated()
          .map(|(idx, variant)| {
            let mut place = self.clone();
            if def.is_struct() {
              // leave as is
            } else if def.is_enum() {
              place.projection.push(ProjectionPrim::Downcast(idx));
            } else {
              unimplemented!("{:?}", def);
            };

            let public_fields = variant
              .fields
              .iter()
              .enumerate()
              .filter(|(_, field)| field.vis.is_accessible_from(module, tcx))
              .map(|(i, _)| i)
              .collect();

            map_fields(&place, public_fields)
          })
          .fold(HashSet::new(), |s1, s2| &s1 | &s2)
      }

      Closure(_def, substs) => {
        let num_upvars =  substs.as_closure().upvar_tys().count();
        map_fields(self, (0 .. num_upvars).collect())
      }

      // TODO: is this correct, eps. for array types?
      _ if ty.is_primitive_ty() || ty.is_ref() || ty.is_array() => HashSet::new(),

      // Functions don't hold any fields
      _ if ty.is_fn_ptr() => HashSet::new(),

      _ => unimplemented!("{:?} {:?}", self, ty),
    };

    places.insert(self.clone());
    places
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
          write!(f, ".{:?})", field.index())?;
        }
        ProjectionPrim::Downcast(index) => {
          write!(f, " as {:?})", index)?;
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
  // e.g. if if place = *x then output is all pointed locations of x
  pub fn possible_prims(&self, place: Place) -> HashSet<PlacePrim> {
    let mut possibly_assigned = HashSet::new();
    possibly_assigned.insert(PlacePrim::local(place.local));

    place
      .iter_projections()
      .fold(possibly_assigned, |acc, (_, projection)| match projection {
        ProjectionElem::Deref => {
          let mut possibly_assigned = HashSet::new();
          for prim in acc.iter() {
            if let Some(prims) = self.0.get(prim) {
              possibly_assigned = &possibly_assigned | prims;
            }
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

        ProjectionElem::Downcast(_, variant) => acc
          .into_iter()
          .map(|mut place| {
            place.projection.push(ProjectionPrim::Downcast(variant));
            place
          })
          .collect(),

        _ => unimplemented!("{:?}", place),
      })
  }

  pub fn points_to(&self, prim: &PlacePrim) -> Option<&HashSet<PlacePrim>> {
    self.0.get(prim)
  }

  pub fn add_alias(&mut self, lplace: Place, rplace: Place) {
    let rprims = self.possible_prims(rplace);
    let rprims_pointed = rprims
      .into_iter()
      .map(|prim| self.0.get(&prim).cloned().unwrap_or_else(HashSet::new))
      .fold(HashSet::new(), |s1, s2| &s1 | &s2);
    for lprim in self.possible_prims(lplace).into_iter() {
      let lprims = self.0.entry(lprim).or_insert_with(HashSet::new);
      *lprims = &*lprims | &rprims_pointed;
    }
  }

  pub fn add_borrow(&mut self, lplace: Place, rplace: Place) {
    let rprims = self.possible_prims(rplace);
    for lprim in self.possible_prims(lplace).into_iter() {
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

struct TransferFunction<'a, 'mir, 'tcx> {
  analysis: &'a PointsToAnalysis<'mir, 'tcx>,
  state: &'a mut PointsToDomain,
}

impl<'a, 'mir, 'tcx> Visitor<'tcx> for TransferFunction<'a, 'mir, 'tcx> {
  fn visit_assign(&mut self, lplace: &Place<'tcx>, rvalue: &Rvalue<'tcx>, location: Location) {
    self.super_assign(lplace, rvalue, location);

    match &rvalue {
      Rvalue::Ref(_region, BorrowKind::Mut { .. }, rplace) => {
        self.state.add_borrow(*lplace, *rplace);
      }
      Rvalue::Use(op) => match op {
        Operand::Move(rplace) | Operand::Copy(rplace) => {
          if lplace
            .ty(self.analysis.body.local_decls(), self.analysis.tcx)
            .ty
            .is_ref()
          {
            self.state.add_alias(*lplace, *rplace);
          }
        }
        Operand::Constant(_) => {}
      },
      _ => {}
    }
  }

  fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, _location: Location) {
    match &terminator.kind {
      TerminatorKind::Call {
        func,
        args,
        destination: Some((dst_place, _)),
        ..
      } => {
        let tcx = self.analysis.tcx;
        let func_ty = func.ty(self.analysis.body.local_decls(), tcx);
        match func_ty.kind() {
          TyKind::FnDef(_, _) => {
            let sig = func_ty.fn_sig(tcx).skip_binder();

            let output_ty = sig.output();
            if let TyKind::Ref(output_region, _, Mutability::Mut) = output_ty.kind() {
              sig
                .inputs()
                .iter()
                .zip(args.iter())
                .filter(|(input_ty, _)| {
                  if let TyKind::Ref(input_region, _, Mutability::Mut) = input_ty.kind() {
                    input_region == output_region
                  } else {
                    false
                  }
                })
                .for_each(|(_, op)| match op {
                  Operand::Move(src_place) => {
                    self.state.add_alias(*dst_place, *src_place);
                  }
                  _ => unimplemented!("{:?}", op),
                });
            }
          }
          _ => unimplemented!("{:?}", func_ty),
        }
      }
      _ => {}
    }
  }
}

pub struct PointsToAnalysis<'mir, 'tcx> {
  pub tcx: TyCtxt<'tcx>,
  pub body: &'mir Body<'tcx>,
}

impl<'mir, 'tcx> AnalysisDomain<'tcx> for PointsToAnalysis<'mir, 'tcx> {
  type Domain = PointsToDomain;
  const NAME: &'static str = "PointsToAnalysis";

  fn bottom_value(&self, _body: &mir::Body<'tcx>) -> Self::Domain {
    PointsToDomain(HashMap::new())
  }

  fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut Self::Domain) {
    // TODO?
  }
}

impl<'mir, 'tcx> Analysis<'tcx> for PointsToAnalysis<'mir, 'tcx> {
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
