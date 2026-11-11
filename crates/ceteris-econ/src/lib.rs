//! The P2 economic core: enterprises, labour markets, wages and final demand on top of
//! the trade layer. See `specs/econ/SPEC.md`.

pub mod admission;
pub mod calibration;
pub mod channels;
pub mod demand;
pub mod edit;
pub mod ensemble;
pub mod equilibrium;
pub mod export;
pub mod fixture;
pub mod flows;
pub mod history;
pub mod inputs;
pub mod labour;
pub mod lanes;
pub mod local;
pub mod migration;
pub mod pack;
pub mod params;
pub mod pop;
pub mod production;
pub mod synthetic;
pub mod systems;
pub mod toy;
pub mod wealth;
pub mod world;

pub use inputs::EconInputs;
pub use params::EconParams;
pub use world::EconWorld;
