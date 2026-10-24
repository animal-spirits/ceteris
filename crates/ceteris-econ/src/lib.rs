//! The P2 economic core: enterprises, labour markets, wages and final demand on top of
//! the trade layer. See `specs/econ/SPEC.md`.

pub mod demand;
pub mod labour;
pub mod params;
pub mod production;

pub use params::EconParams;
