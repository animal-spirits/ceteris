//! P0 gate (Section 21.1): a toy world of 10 provinces runs 1,200 ticks; identical
//! hashes across two runs and across 1 vs 8 threads; restore-and-replay from any
//! snapshot matches.

use ceteris_core::rng::Seed;
use ceteris_core::sim::{SNAPSHOT_EVERY, Sim};
use ceteris_core::toy::Toy;

const TICKS: u32 = 1200;
const SEED: Seed = Seed(42);

fn run(threads: usize, provinces: usize, ticks: u32) -> Sim<Toy> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let mut sim = Toy::sim(provinces, SEED);
        sim.run_until(ticks).unwrap_or_else(|v| panic!("{v}"));
        sim
    })
}

#[test]
fn two_runs_give_identical_hashes_at_every_snapshot() {
    let (a, b) = (run(4, 10, TICKS), run(4, 10, TICKS));
    assert_eq!(a.snapshot_hashes().len() as u32, TICKS / SNAPSHOT_EVERY + 1);
    assert_eq!(a.snapshot_hashes(), b.snapshot_hashes());
}

#[test]
fn one_and_eight_threads_give_identical_hashes() {
    for provinces in [10, 300] {
        let ticks = if provinces == 10 { TICKS } else { 240 };
        assert_eq!(
            run(1, provinces, ticks).snapshot_hashes(),
            run(8, provinces, ticks).snapshot_hashes(),
            "{provinces} provinces"
        );
    }
}

#[test]
fn restore_and_replay_from_every_snapshot_matches() {
    let mut sim = run(4, 10, TICKS);
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

#[test]
fn seek_to_a_tick_between_snapshots_matches_a_direct_run() {
    let mut sim = run(4, 10, TICKS);
    for t in [1, 100, 777, 1199] {
        sim.seek(t).unwrap();
        assert_eq!(sim.state_hash(), run(4, 10, t).state_hash(), "tick {t}");
    }
}

#[test]
fn different_seeds_diverge() {
    let mut other = Toy::sim(10, Seed(43));
    other.run_until(TICKS).unwrap();
    assert_ne!(run(4, 10, TICKS).state_hash(), other.state_hash());
}
