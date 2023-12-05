//! Proxies for Rustc types used within the PDG.
//!
//! Each type has an identical set of fields to the corresponding Rustc type.
//! Paralegal serializes the PDG into these types, which are read by downstream property checkers.

use serde::{Deserialize, Serialize};

#[cfg(feature = "rustc")]
use crate::{
  rustc::{def_id, hir, mir},
  rustc_impls::*,
};

/// Generates a struct that is a proxy for a Rustc type.
///
/// This works by telling Serde to the proxy struct as "remote" for the Rustc type.
/// Each field of the struct is either the actual Rustc type if the "rustc" feature is enabled,
/// or the proxy type otherwise.
macro_rules! proxy_struct {
    ($(
      $(#[$attr:meta])*
      $name:ident($rustc:expr) {
        $($field:ident : $rustc_ty:ty  => $proxy_ty:ty , $proxy_str:expr),*
      }
    )*) => {
        $(
            #[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
            #[cfg_attr(feature = "rustc", serde(remote = $rustc))]
            $(#[$attr])*
            pub struct $name {
                $(
                    #[cfg(feature = "rustc")]
                    #[serde(with = $proxy_str)]
                    pub $field: $rustc_ty,
                    #[cfg(not(feature = "rustc"))]
                    pub $field: $proxy_ty,
                )*
            }
        )*
    }
}

/// Generates a struct that is a proxy for a Rustc index type.
macro_rules! proxy_index {
    ($(
        $(#[$attr:meta])*
        $name:ident($rustc:expr) from $fn:expr
    );*) => {
        $(
            #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Debug, Serialize, Deserialize)]
            #[cfg_attr(feature = "rustc", serde(remote = $rustc))]
            $(#[$attr])*
            pub struct $name {
                #[cfg_attr(feature = "rustc", serde(getter = $fn))]
                pub(crate) private: u32
            }

            #[cfg(not(feature = "rustc"))]
            impl $name {
                pub fn index(self) -> usize {
                    self.private as usize
                }
            }
        )*
    }
}

proxy_index! {
    /// Proxy for `mir::BasicBlock`
    BasicBlock("mir::BasicBlock") from "bbref_to_u32";

    /// Proxy for `hir::ItemLocalId`
    ItemLocalId("hir::ItemLocalId") from "item_local_id_as_u32";

    /// Proxy for `def_id::DefIndex`
    DefIndex("def_id::DefIndex") from "def_index_as_u32";

    /// Proxy for `hir::def_id::CrateNum`
    CrateNum("hir::def_id::CrateNum") from "crate_num_as_u32"
}

proxy_struct! {
    /// Proxy for `mir::Location`
    #[derive(PartialOrd, Ord)]
    Location("mir::Location") {
        block: mir::BasicBlock => BasicBlock, "BasicBlock",
        statement_index: usize => usize, "usize"
    }

    /// Proxy for `def_id::LocalDefId`
    LocalDefId("def_id::LocalDefId") {
        local_def_index: def_id::DefIndex => DefIndex, "DefIndex"
    }

    /// Proxy for `hir_id::OwnerHid`
    OwnerId("hir::hir_id::OwnerId") {
        def_id: def_id::LocalDefId => LocalDefId, "LocalDefId"
    }

    /// Proxy for `hir::HirId`
    HirId("hir::HirId") {
        owner: hir::OwnerId => OwnerId, "OwnerId",
        local_id: hir::ItemLocalId => ItemLocalId, "ItemLocalId"
    }

    /// Proxy for `hir::BodyId`
    BodyId("hir::BodyId") {
        hir_id: hir::HirId => HirId, "HirId"
    }

    #[derive(Ord, PartialOrd)]
    /// Proxy for `def_id::DefId`
    DefId("def_id::DefId") {
        index: def_id::DefIndex => DefIndex, "DefIndex",
        krate: hir::def_id::CrateNum => CrateNum, "CrateNum"
    }
}

impl HirId {
  fn index(self) -> (usize, usize) {
    (
      self.owner.def_id.local_def_index.index(),
      self.local_id.index(),
    )
  }
}

impl Ord for HirId {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    (self.index()).cmp(&(other.index()))
  }
}

impl PartialOrd for HirId {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}
