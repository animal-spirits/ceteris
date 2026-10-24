//! The P2 economic core: enterprises, labour markets, wages and final demand on top of
//! the trade layer. See `specs/econ/SPEC.md`.

pub mod demand;
pub mod equilibrium;
pub mod export;
pub mod fixture;
pub mod flows;
pub mod inputs;
pub mod labour;
pub mod params;
pub mod production;
pub mod systems;
pub mod world;

pub use inputs::EconInputs;
pub use params::EconParams;
pub use world::EconWorld;
