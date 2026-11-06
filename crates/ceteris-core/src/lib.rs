//! Core machinery for Ceteris Paribus: stable entity IDs, the system scheduler,
//! counter-based random streams, the money and goods ledger, the event log,
//! deterministic reductions, and snapshot and restore. See `specs/core/SPEC.md`.

pub mod entity;
pub mod event;
pub mod invariant;
pub mod ledger;
pub mod reduce;
pub mod rng;
pub mod rows;
pub mod schedule;
pub mod sim;
pub mod snapshot;
pub mod timing;
pub mod toy;
