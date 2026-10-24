//! Stability of the P2 tick pipeline around the GE reference equilibrium of the fixture
//! (`tests/golden/p2-fixture/reference`, written by `python -m ceteris.reference solve`) and
//! from the data state. Slow in debug builds; run with `--release` for speed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;

/// Fixture export directory; `CETERIS_FIXTURE_DIR` overrides the committed one.
fn fixture_dir() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

fn reference() -> Equilibrium {
    let dir = fixture_dir().join("reference");
    Equilibrium::read(&dir).expect("reference solution of the fixture")
}

fn at_equilibrium(params: EconParams, solution: &Equilibrium) -> EconWorld {
    EconWorld::from_equilibrium(
        Arc::new(small_world()),
        params,
        TradeParams::default(),
        solution,
    )
}

fn sim_of(world: EconWorld) -> Sim<EconWorld> {
    Sim::new(world, schedule(), SEED)
}

fn active_cells(w: &EconWorld) -> Vec<usize> {
    (0..w.markets.price.len())
        .filter(|&c| w.markets.active[c])
        .collect()
}

/// Max |ln p - ln p*| over active cells.
fn distance(w: &EconWorld, solution: &Equilibrium, cells: &[usize]) -> f64 {
    cells
        .iter()
        .map(|&c| (libm::log(w.markets.price[c]) - libm::log(solution.price[c])).abs())
        .fold(0.0, f64::max)
}

#[test]
fn equilibrium_is_a_fixed_point() {
    let solution = reference();
    let mut sim = sim_of(at_equilibrium(EconParams::frictionless(), &solution));
    let cells = active_cells(sim.world());
    for tick in 0..24 {
        let (price, wage) = (sim.world().markets.price.clone(), sim.world().wage.clone());
        sim.step().unwrap();
        let w = sim.world();
        for &c in &cells {
            let rel = (w.markets.price[c] / price[c] - 1.0).abs();
            assert!(rel < 1e-8, "tick {tick} cell {c}: price moved {rel:e}");
        }
        for (i, (&now, &before)) in w.wage.iter().zip(&wage).enumerate() {
            let rel = (now / before - 1.0).abs();
            assert!(rel < 1e-8, "tick {tick} province {i}: wage moved {rel:e}");
        }
        let acc = &w.layout().accounts;
        assert!(
            acc.pop.iter().all(|&a| w.ledger.money(a) > 0),
            "tick {tick}: a pop holds no money"
        );
    }
}

#[test]
fn small_perturbation_decays() {
    let solution = reference();
    for params in [EconParams::frictionless(), EconParams::default()] {
        let frictionless = params.frictionless;
        let mut w = at_equilibrium(params, &solution);
        let cells = active_cells(&w);
        // every active price times a seeded factor in [0.99, 1.01] (xorshift64)
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        for &c in &cells {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let u = (state >> 11) as f64 / (1u64 << 53) as f64;
            w.markets.price[c] *= 1.0 + 0.01 * (2.0 * u - 1.0);
        }
        let mut sim = sim_of(w);
        sim.run_until(300).unwrap();
        let mid = distance(sim.world(), &solution, &cells);
        sim.run_until(600).unwrap();
        let end = distance(sim.world(), &solution, &cells);
        if frictionless {
            assert!(end < 1e-4, "frictionless: {end:e} at tick 600");
        } else {
            assert!(
                end < 1e-2 && end < mid,
                "default: {mid:e} at 300, {end:e} at 600"
            );
        }
    }
}

#[test]
fn data_state_runs_1200_ticks_without_violation() {
    for params in [EconParams::default(), EconParams::frictionless()] {
        let mut sim = EconWorld::sim(Arc::new(small_world()), params, TradeParams::default());
        sim.run_until(1200).unwrap();
    }
}
