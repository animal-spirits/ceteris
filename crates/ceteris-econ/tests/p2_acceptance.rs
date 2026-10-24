//! P2 fixture determinism: identical hashes across two runs and across 1 vs 8 threads,
//! and restore-and-replay from every snapshot. Slow in debug builds; run with `--release`.

use std::sync::Arc;

use ceteris_core::sim::{SNAPSHOT_EVERY, Sim};
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;

const TICKS: u32 = 1200;

fn run(threads: usize, ticks: u32) -> Sim<EconWorld> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let mut sim = EconWorld::sim(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        );
        sim.run_until(ticks).unwrap_or_else(|v| panic!("{v}"));
        sim
    })
}

#[test]
fn two_runs_give_identical_hashes_at_every_snapshot() {
    let (a, b) = (run(4, TICKS), run(4, TICKS));
    assert_eq!(a.snapshot_hashes().len() as u32, TICKS / SNAPSHOT_EVERY + 1);
    assert_eq!(a.snapshot_hashes(), b.snapshot_hashes());
}

#[test]
fn one_vs_eight_threads_identical() {
    assert_eq!(
        run(1, TICKS).snapshot_hashes(),
        run(8, TICKS).snapshot_hashes()
    );
}

#[test]
fn replay_from_every_snapshot() {
    let mut sim = run(4, TICKS);
    let reference = sim.snapshot_hashes();
    let final_hash = sim.state_hash();
    for &(t, _) in &reference {
        sim.seek(t).unwrap();
        sim.run_until((t + SNAPSHOT_EVERY).min(TICKS)).unwrap();
        assert_eq!(sim.snapshot_hashes(), reference, "replay from tick {t}");
    }
    for t in [0, 600, 1188] {
        sim.seek(t).unwrap();
        sim.run_until(TICKS).unwrap();
        assert_eq!(sim.state_hash(), final_hash, "full replay from tick {t}");
    }
}
