//! Infrastructure for analyzing MIR that supports the information flow analysis.

pub mod aliases;
pub mod borrowck_facts;
pub mod control_dependencies;
pub mod engine;
pub mod utils;
