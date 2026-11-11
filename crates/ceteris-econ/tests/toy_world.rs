//! The toy-world builder runs end to end through the real schedule: invariants hold from the
//! data state, and the state hash is the same on every run.

use std::sync::Arc;

use ceteris_econ::toy::ToyWorld;
use ceteris_econ::{EconParams, EconWorld};

fn run(toy: &ToyWorld, ticks: u32) -> ceteris_core::snapshot::Hash {
    let (w, trade) = toy.build();
    let mut sim = EconWorld::sim(Arc::new(w), EconParams::default(), trade);
    sim.run_until(ticks).unwrap();
    sim.state_hash()
}

#[test]
fn symmetric_two_country_world_runs_without_violation() {
    let toy = ToyWorld::new(2, 2, ToyWorld::goods(3, 2)).tariff(0.05);
    assert_eq!(run(&toy, 120), run(&toy, 120));
}

#[test]
fn asymmetric_three_country_world_runs_without_violation() {
    let toy = ToyWorld::new(3, 2, ToyWorld::goods(4, 3))
        .tech(1, 0, 0.0)
        .tech(2, 1, 1.5);
    run(&toy, 120);
}
