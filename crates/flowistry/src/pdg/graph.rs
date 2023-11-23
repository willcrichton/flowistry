use std::{fmt, path::Path};

use either::Either;
use petgraph::{dot, graph::DiGraph};
use rustc_hir::def_id::DefId;
use rustc_middle::{
  mir::{Body, Location, Place},
  ty::{tls, TyCtxt},
};
use rustc_utils::{mir::borrowck_facts, PlaceExt};

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum LocationOrStart {
  Location(Location),
  Start,
}

impl fmt::Debug for LocationOrStart {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      LocationOrStart::Location(loc) => loc.fmt(f),
      LocationOrStart::Start => write!(f, "start"),
    }
  }
}

impl From<Location> for LocationOrStart {
  fn from(value: Location) -> Self {
    LocationOrStart::Location(value)
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct GlobalLocation {
  pub function: DefId,
  pub location: LocationOrStart,
}

impl fmt::Debug for GlobalLocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}::", self.location)?;
    tls::with_opt(|opt_tcx| match opt_tcx {
      Some(tcx) => write!(f, "{}", tcx.item_name(self.function)),
      None => write!(f, "{:?}", self.function),
    })
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum DepNode<'tcx> {
  Place {
    place: Place<'tcx>,
    at: GlobalLocation,
  },
  Op(GlobalLocation),
}

impl<'tcx> DepNode<'tcx> {
  pub fn expect_place(self) -> Place<'tcx> {
    match self {
      DepNode::Place { place, .. } => place,
      DepNode::Op(..) => panic!("Expected a place, got an op"),
    }
  }
}

impl fmt::Debug for DepNode<'_> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    tls::with_opt(|opt_tcx| match opt_tcx {
      Some(tcx) => match self {
        DepNode::Place { place, at } => {
          let place_str = match at.function.as_local() {
            Some(def_id) => {
              let body = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
              let tcx =
                unsafe { std::mem::transmute::<TyCtxt<'_>, TyCtxt<'static>>(tcx) };
              let place =
                unsafe { std::mem::transmute::<Place<'_>, Place<'static>>(*place) };
              let body = unsafe {
                std::mem::transmute::<&'_ Body<'_>, &'_ Body<'static>>(&body.body)
              };

              place
                .to_string(tcx, body)
                .unwrap_or_else(|| format!("{place:?}"))
            }
            None => format!("{place:?}"),
          };
          write!(f, "{place_str} @ {at:?}")
        }
        DepNode::Op(global_loc) => {
          let loc_str = match global_loc.location {
            LocationOrStart::Start => "start".to_string(),
            LocationOrStart::Location(loc) => match global_loc.function.as_local() {
              Some(def_id) => {
                let body = borrowck_facts::get_body_with_borrowck_facts(tcx, def_id);
                match body.body.stmt_at(loc) {
                  Either::Left(stmt) => format!("{stmt:?}"),
                  Either::Right(term) => format!("{term:?}"),
                }
              }
              None => format!("{loc:?}"),
            },
          };
          write!(f, "{loc_str} @ {}", tcx.item_name(global_loc.function))
        }
      },
      None => todo!(),
    })
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum DepEdge {
  Control,
  Data,
}

pub struct DepGraph<'tcx> {
  pub graph: DiGraph<DepNode<'tcx>, DepEdge>,
}

impl<'tcx> DepGraph<'tcx> {
  pub fn generate_graphviz(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
    let graph_dot = format!("{:?}", dot::Dot::with_config(&self.graph, &[]));
    rustc_utils::mir::body::run_dot(path.as_ref(), graph_dot.into_bytes())
  }
}
