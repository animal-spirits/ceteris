//! Measurement harness of MIGRATION Task 6 (expected inflation, ADR 0019): V1 from the data
//! state and the growth factor per tick of a small deviation from the fixture's GE
//! equilibrium for a list of P45 gains at given kappa1 and kappa2 (the trade SPEC's P03a, P03b
//! and P45 rows, its Nominal mode, and the fixture growth factors behind the stability
//! region). Every test is ignored and prints its table; run on a fixture export and its
//! reference:
//!
//! ```text
//! CETERIS_FIXTURE_DIR=$PWD/target/p2-wip GAINS=0,0.006,0.1 [K1=0.05] [K2=0] [LAG=3] \
//!   [DEFAULT=1] cargo test -p ceteris-econ --release --test p2_expectation_sweep -- \
//!   --ignored --nocapture {v1_by_gain|decay_by_gain}
//! ```
//!
//! `CETERIS_FIXTURE_DIR` defaults to `tests/golden/p2-fixture`; `GAINS`, `K1` and `K2` to the
//! defaults. `pack_decay_by_gain` measures the same growth factors on the calibrated full pack
//! (`CETERIS_PACK`, default `data/packs/2000`; PLAN Task 20). The harness of MIGRATION Task 6
//! (`task-6-gain-harness.rs.txt`) is this file's `v1_by_gain` and `decay_by_gain`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_trade::TradeParams;
use ceteris_trade::goods::p2::N as N_GOODS;

fn solution() -> Equilibrium {
    let dir = std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    );
    Equilibrium::read(&dir.join("reference")).unwrap()
}

fn gains() -> Vec<f64> {
    std::env::var("GAINS").map_or_else(
        |_| vec![TradeParams::default().expectation_gain],
        |v| v.split(',').map(|g| g.parse().unwrap()).collect(),
    )
}

fn trade(gain: f64) -> TradeParams {
    let d = TradeParams::default();
    let var =
        |name: &str, default: f64| std::env::var(name).map_or(default, |v| v.parse().unwrap());
    TradeParams {
        expectation_gain: gain,
        kappa1: var("K1", d.kappa1),
        kappa2: var("K2", d.kappa2),
        ..d
    }
}

/// Max |ln p - ln p*| over active cells.
fn distance(w: &EconWorld, s: &Equilibrium) -> f64 {
    (0..w.markets.price.len())
        .filter(|&c| w.markets.active[c])
        .map(|c| (w.markets.price[c].ln() - s.price[c].ln()).abs())
        .fold(0.0, f64::max)
}

/// V1 deviations every 200 ticks of the run from the data state, the peak world price level,
/// the largest |pi^e| and the clip counts, per gain (`DEFAULT` for the default mode).
#[test]
#[ignore = "measurement harness"]
fn v1_by_gain() {
    let s = solution();
    let default = std::env::var_os("DEFAULT").is_some();
    for g in gains() {
        let params = if default {
            EconParams::default()
        } else {
            EconParams::frictionless()
        };
        let w = EconWorld::new(Arc::new(small_world()), params, trade(g));
        let mut sim = Sim::new(w, schedule(), SEED);
        let (mut line, mut fail) = (String::new(), None);
        let (mut peak, mut pi_max) = (0.0f64, 0.0f64);
        for t in 1..=1200u32 {
            if let Err(v) = sim.step() {
                fail = Some(format!("{v}"));
                break;
            }
            let w = sim.world();
            peak = peak.max(w.price_level);
            pi_max = pi_max.max(w.expected_inflation.abs());
            if t % 200 == 0 {
                let index = w.world_price_index();
                let p = (0..w.markets.price.len())
                    .filter(|&c| w.markets.active[c])
                    .map(|c| (w.markets.price[c] / index / s.price[c] - 1.0).abs())
                    .fold(0.0, f64::max);
                let wage = (0..w.wage.len())
                    .map(|i| (w.wage[i] / index / s.wage[i] - 1.0).abs())
                    .fold(0.0, f64::max);
                line += &format!(
                    " t{t}: p {p:.2e} w {wage:.2e} I {index:.3} pi {:.1e};",
                    w.expected_inflation
                );
            }
        }
        let w = sim.world();
        let clips: u32 = w.markets.clip_ticks.iter().sum();
        let wage_clips: u32 = w.wage_clip_ticks.iter().sum();
        eprintln!(
            "GAIN {g} default {default}: peak I {peak:.2} max|pi| {pi_max:.3} clips {clips} \
             wage clips {wage_clips} {fail:?}{line}"
        );
    }
}

/// Growth per tick between ticks 300 and 600 of the distance from the equilibrium after a
/// deterministic 1% perturbation of every active price, per gain, in both modes (`LAG` for
/// crossing lags).
#[test]
#[ignore = "measurement harness"]
fn decay_by_gain() {
    let s = solution();
    let lag: Option<u8> = std::env::var("LAG").ok().map(|v| v.parse().unwrap());
    for g in gains() {
        for params in [EconParams::frictionless(), EconParams::default()] {
            let f = params.frictionless;
            let mut w = EconWorld::new(Arc::new(small_world()), params, trade(g));
            if let Some(l) = lag {
                w.set_crossing_lags(l);
            }
            w.settle(&s);
            let cells: Vec<usize> = (0..w.markets.price.len())
                .filter(|&c| w.markets.active[c])
                .collect();
            let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
            for c in cells {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let u = (state >> 11) as f64 / (1u64 << 53) as f64;
                w.markets.price[c] *= 1.0 + 0.01 * (2.0 * u - 1.0);
            }
            let mut sim = Sim::new(w, schedule(), SEED);
            let r = sim.run_until(300).map_err(|v| format!("{v}"));
            let mid = distance(sim.world(), &s);
            let r2 = sim.run_until(600).map_err(|v| format!("{v}"));
            let end = distance(sim.world(), &s);
            let t = trade(g);
            eprintln!(
                "DECAY k1 {} k2 {} lag {lag:?} gain {g} frictionless {f}: {mid:.2e} at 300, \
                 {end:.2e} at 600, growth {:.5} {:?} {:?}",
                t.kappa1,
                t.kappa2,
                (end / mid).powf(1.0 / 300.0),
                r.err(),
                r2.err()
            );
        }
    }
}

/// `decay_by_gain` on the calibrated full pack from its steady state (crossing lags as built):
/// the growth per tick between ticks 300 and 600 of the distance from the steady state after
/// the same 1% perturbation of every active price, per gain, in both modes. `EXCLUDE` (a
/// comma list of provinces) leaves their cells out of the distance, to measure the rest of
/// the world past a local cycle.
#[test]
#[ignore = "measurement harness"]
fn pack_decay_by_gain() {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    assert!(
        EconInputs::has_calibration(&dir),
        "no calibrated pack at {}",
        dir.display()
    );
    let pack = ceteris_geo::pack::Pack::load(&dir, &TransportParams::default()).unwrap();
    let inputs = Arc::new(EconInputs::from_pack(&pack, &dir).unwrap());
    for g in gains() {
        for params in [EconParams::frictionless(), EconParams::default()] {
            let f = params.frictionless;
            let mut w = EconWorld::new(Arc::clone(&inputs), params, trade(g));
            let steady = w.markets.price.clone();
            let cells: Vec<usize> = (0..w.markets.price.len())
                .filter(|&c| w.markets.active[c])
                .collect();
            let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
            for c in cells {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let u = (state >> 11) as f64 / (1u64 << 53) as f64;
                w.markets.price[c] *= 1.0 + 0.01 * (2.0 * u - 1.0);
            }
            let excluded: Vec<usize> = std::env::var("EXCLUDE").map_or_else(
                |_| vec![],
                |v| v.split(',').map(|i| i.parse().unwrap()).collect(),
            );
            let distance = |w: &EconWorld| {
                (0..w.markets.price.len())
                    .filter(|&c| w.markets.active[c] && !excluded.contains(&(c / N_GOODS)))
                    .map(|c| (w.markets.price[c].ln() - steady[c].ln()).abs())
                    .fold(0.0, f64::max)
            };
            let mut sim = Sim::new(w, schedule(), SEED);
            let r = sim.run_until(300).map_err(|v| format!("{v}"));
            let mid = distance(sim.world());
            let r2 = sim.run_until(600).map_err(|v| format!("{v}"));
            let end = distance(sim.world());
            let t = trade(g);
            eprintln!(
                "PACK DECAY k1 {} k2 {} gain {g} frictionless {f} excluding {excluded:?}: {mid:.2e} at 300, {end:.2e} at \
                 600, growth {:.5} {:?} {:?}",
                t.kappa1,
                t.kappa2,
                (end / mid).powf(1.0 / 300.0),
                r.err(),
                r2.err()
            );
        }
    }
}
