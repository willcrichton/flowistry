//! Exports either rustc identifiers or their proxies depending on whether the
//! `rustc` feature is enabled.
//!
//! The idea is that you can then define your data structure over this
//! (including serialization) like so, using `cfg_attr:
//!
//! ```
//! pub struct GlobalLocationS {
//!     #[cfg_attr(feature = "rustc", serde(with = "rustc_proxies::BodyId"))]
//!     pub function: BodyId,
//!
//!     #[cfg_attr(feature = "rustc", serde(with = "rustc_proxies::Location"))]
//!     pub location: Location,
//! }
//! ```

cfg_if::cfg_if! {
    if #[cfg(feature = "rustc")] {
        use crate::rustc::{hir, mir, def_id};
        // We are redefining these here as a type alias instead of just `pub
        // use`, because the latter requires of consumers of this library to use
        // the `rustc_private` feature, whereas it doesn't with type aliases.
        pub type Location = mir::Location;
        pub type BasicBlock = mir::BasicBlock;
        pub type BodyId = hir::BodyId;
        pub type ItemLocalId = hir::ItemLocalId;
        pub type OwnerId = hir::hir_id::OwnerId;
        pub type HirId = hir::HirId;
        pub type DefIndex = def_id::DefIndex;
        pub type LocalDefId = def_id::LocalDefId;
        pub type DefId = def_id::DefId;
        pub type Place<'tcx> = mir::Place<'tcx>;
    } else {
        pub use crate::rustc_proxies::*;
    }
}
