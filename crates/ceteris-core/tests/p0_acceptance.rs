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

fn boost(toy: &mut Toy) {
    toy.firm_capacity[0] *= 1.5;
}

/// Hashes at every tick from `from` to `to` of `sim`, stepping.
fn hashes(sim: &mut Sim<Toy>, from: u32, to: u32) -> Vec<(u32, [u8; 32])> {
    sim.seek(from).unwrap();
    let mut out = vec![(from, sim.state_hash())];
    while sim.tick() < to {
        sim.step().unwrap();
        out.push((sim.tick(), sim.state_hash()));
    }
    out
}

/// A seek from a snapshot before an intervention replays it at its tick, in both directions,
/// also when the intervention falls on a snapshot tick (the snapshot holds the world before it).
#[test]
fn seek_across_an_intervention_replays_it() {
    for at in [17, 24] {
        let mut sim = Toy::sim(10, SEED);
        sim.run_until(at).unwrap();
        sim.intervene(boost);
        let continuous = hashes(&mut sim, at, 60);
        let mut direct = Toy::sim(10, SEED);
        direct.run_until(at).unwrap();
        boost(direct.world_mut());
        direct.run_until(60).unwrap();
        assert_eq!(sim.state_hash(), direct.state_hash());
        for from in [14, at, 26, 3] {
            let replayed = hashes(&mut sim, from, 60);
            let tail: Vec<_> = continuous.iter().filter(|(t, _)| *t >= from).collect();
            assert!(
                replayed
                    .iter()
                    .filter(|(t, _)| *t >= at)
                    .eq(tail.iter().copied()),
                "intervention at {at}, seek to {from}"
            );
        }
    }
}

/// An intervention after a seek back starts a new timeline: the old one's later intervention
/// and snapshots are gone.
#[test]
fn intervening_after_a_seek_back_drops_the_later_timeline() {
    let mut sim = Toy::sim(10, SEED);
    sim.run_until(30).unwrap();
    sim.intervene(boost);
    sim.run_until(60).unwrap();
    sim.seek(20).unwrap();
    sim.intervene(|t: &mut Toy| t.firm_capacity[1] *= 2.0);
    assert!(sim.snapshot_hashes().iter().all(|(t, _)| *t <= 20));
    sim.seek(60).unwrap();
    let mut direct = Toy::sim(10, SEED);
    direct.run_until(20).unwrap();
    direct.world_mut().firm_capacity[1] *= 2.0;
    direct.run_until(60).unwrap();
    assert_eq!(sim.state_hash(), direct.state_hash());
    sim.seek(48).unwrap();
    sim.run_until(60).unwrap();
    assert_eq!(sim.state_hash(), direct.state_hash());
}
