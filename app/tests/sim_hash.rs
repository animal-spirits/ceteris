//! The app reads the run and never changes it (specs/app SPEC, Invariants): views published
//! through the run-ahead ring carry the same state hashes as a headless run, whatever the
//! ring's size (ADR 0208).

use std::sync::Arc;

use ceteris_app::sim::Runner;
use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;

const TICKS: u32 = 24;

fn world() -> Sim<EconWorld> {
    EconWorld::sim(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    )
}

fn through_ring(ring: usize) -> Vec<(u32, [u8; 32])> {
    let runner = Runner::spawn(
        ring,
        || Ok(world()),
        |sim| (sim.tick(), sim.state_hash()),
        || {},
    );
    (0..=TICKS)
        .map(|_| runner.wait().expect("runner alive").expect("no violation"))
        .collect()
}

#[test]
fn ring_views_match_a_headless_run_at_every_tick() {
    let mut sim = world();
    let mut headless = vec![(0, sim.state_hash())];
    for _ in 0..TICKS {
        sim.step().expect("no violation");
        headless.push((sim.tick(), sim.state_hash()));
    }
    assert_eq!(through_ring(4), headless);
}

#[test]
fn ring_size_changes_no_hash() {
    assert_eq!(through_ring(1), through_ring(4));
}
