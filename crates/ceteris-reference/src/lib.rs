#![allow(clippy::disallowed_methods)] // independent oracle: deliberately uses std, never feeds a hash
//! Independent reference model of the Ceteris Paribus economy, written from the spec: it reads
//! the `ceteris export` Arrow directory and never the structs of the crates it checks.

pub mod demand;
pub mod io;
pub mod linalg;
pub mod model;
pub mod output;
pub mod path;
pub mod ppml;
pub mod sinkhorn;
pub mod solve;
pub mod uniqueness;
