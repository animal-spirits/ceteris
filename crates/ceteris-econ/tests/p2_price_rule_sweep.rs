//! Measurement harness of MIGRATION Task 3 (price rule under origin clearing): the growth
//! factors per tick of a small deviation from the fixture's GE equilibrium over a grid of
//! (kappa1, kappa2), in both modes and with crossing lags as built, 0 or 3 (the trade SPEC's
//! P03a and P03b deviation row and Stability region). Every test is ignored and prints its
//! table; run on a fixture export and its reference:
//!
//! ```text
//! CETERIS_FIXTURE_DIR=$PWD/target/p2-wip [K1=0.05] [K2=0] [LAG=3] [DEFAULT=1] \
//!   cargo test -p ceteris-econ --release --test p2_price_rule_sweep -- --ignored --nocapture <test>
//! ```
//!
//! `CETERIS_FIXTURE_DIR` defaults to `tests/golden/p2-fixture`; `K1` and `K2` to the defaults.
//! The grid runs at the default P45 gain; the growth factors at pi^e = 0 come from
//! `p2_expectation_sweep.rs` (`decay_by_gain` with `GAINS=0`).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::{N_GOODS, Sourced, TradeParams, dispatch, source};

fn solution() -> Equilibrium {
    let dir = std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    );
    Equilibrium::read(&dir.join("reference")).unwrap()
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).map_or(default, |v| v.parse().ok().unwrap())
}

fn lag() -> Option<u8> {
    std::env::var("LAG").ok().map(|v| v.parse().unwrap())
}

fn world(params: EconParams, trade: TradeParams, lag: Option<u8>, s: &Equilibrium) -> EconWorld {
    let mut w = EconWorld::new(Arc::new(small_world()), params, trade);
    if let Some(l) = lag {
        w.set_crossing_lags(l);
    }
    w.settle(s);
    w
}

fn active(w: &EconWorld) -> impl Iterator<Item = usize> + '_ {
    (0..w.markets.price.len()).filter(|&c| w.markets.active[c])
}

/// Max |ln p - ln p*| over active cells.
fn distance(w: &EconWorld, s: &Equilibrium) -> f64 {
    active(w)
        .map(|c| (w.markets.price[c].ln() - s.price[c].ln()).abs())
        .fold(0.0, f64::max)
}

/// Max |ln p_a - ln p_b| over active cells of two runs.
fn gap(a: &EconWorld, b: &EconWorld) -> f64 {
    active(a)
        .map(|c| (a.markets.price[c].ln() - b.markets.price[c].ln()).abs())
        .fold(0.0, f64::max)
}

/// Every active price moved by a deterministic draw in [-1%, 1%].
fn perturbed(mut w: EconWorld) -> EconWorld {
    let cells: Vec<usize> = active(&w).collect();
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for c in cells {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let u = (state >> 11) as f64 / (1u64 << 53) as f64;
        w.markets.price[c] *= 1.0 + 0.01 * (2.0 * u - 1.0);
    }
    w
}

/// Max relative deviation of prices and wages from the reference, over the world index.
fn v1_deviation(w: &EconWorld, s: &Equilibrium) -> (f64, usize, f64) {
    let index = w.world_price_index();
    let (mut price, mut cell) = (0.0, 0);
    for c in active(w) {
        let d = (w.markets.price[c] / index / s.price[c] - 1.0).abs();
        if d > price {
            (price, cell) = (d, c);
        }
    }
    let wage = (0..w.wage.len())
        .map(|i| (w.wage[i] / index / s.wage[i] - 1.0).abs())
        .fold(0.0, f64::max);
    (price, cell, wage)
}

#[test]
#[ignore = "measurement harness"]
fn fixed_point_as_built() {
    let s = solution();
    for lag in [None, Some(3)] {
        let w = world(EconParams::frictionless(), TradeParams::default(), lag, &s);
        let mut sim = Sim::new(w, schedule(), SEED);
        for t in 1..=48 {
            sim.step().unwrap();
            if t % 12 == 0 {
                eprintln!("lag {lag:?} t {t} dist {:e}", distance(sim.world(), &s));
            }
        }
    }
}

/// Distance from the equilibrium every 50 ticks after a 1% perturbation at `K1`, `K2`, `LAG`.
#[test]
#[ignore = "measurement harness"]
fn growth() {
    let s = solution();
    let d = TradeParams::default();
    let trade = TradeParams {
        kappa1: env_or("K1", d.kappa1),
        kappa2: env_or("K2", d.kappa2),
        ..d
    };
    for params in [EconParams::frictionless(), EconParams::default()] {
        let f = params.frictionless;
        let w = perturbed(world(params, trade.clone(), lag(), &s));
        let mut sim = Sim::new(w, schedule(), SEED);
        let (mut traj, mut fail) = (vec![], None);
        for t in 1..=600u32 {
            if let Err(v) = sim.step() {
                fail = Some(format!("{v}"));
                break;
            }
            if t % 50 == 0 {
                traj.push(format!("{:.2e}", distance(sim.world(), &s)));
            }
        }
        eprintln!(
            "RESULT k1 {} k2 {} lag {:?} frictionless {f}: {} {fail:?}",
            trade.kappa1,
            trade.kappa2,
            lag(),
            traj.join(" ")
        );
    }
}

/// Inputs held at zero stock at the equilibrium, with what is on order.
#[test]
#[ignore = "measurement harness"]
fn clamped() {
    let s = solution();
    for lag in [Some(0u8), None] {
        let w = world(EconParams::frictionless(), TradeParams::default(), lag, &s);
        let inp = w.inputs();
        let mut n = 0;
        for e in 0..inp.n_enterprises() {
            let i = inp.enterprises.province[e] as usize;
            let (c, g) = (inp.country_of(i), usize::from(inp.enterprises.good[e]));
            for k in 0..7 {
                let a = inp.a(c, k, g);
                if a > 0.0 && w.ledger.goods(w.layout().accounts.enterprise[e], k) == 0.0 {
                    n += 1;
                    if n <= 12 {
                        eprintln!(
                            "lag {lag:?} enterprise {e} province {i} country {c} good {g} input \
                             {k} on order {:.3} of use {:.3}",
                            w.pipeline.on_order(inp.n_pops() + e, k),
                            a * w.output[e]
                        );
                    }
                }
            }
        }
        eprintln!("lag {lag:?}: {n} inputs with zero stock");
    }
}

/// r = alpha_d q_d (units arriving per unit of spending times the delivered unit price) over
/// destinations and goods, and the island's arrival slots.
#[test]
#[ignore = "measurement harness"]
fn ratio() {
    let s = solution();
    let w = world(EconParams::frictionless(), TradeParams::default(), None, &s);
    let np = w.inputs().n_provinces();
    let mut out = Sourced::new();
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    for g in 0..7 {
        let price: Vec<f64> = (0..np).map(|o| w.markets.price[o * N_GOODS + g]).collect();
        let active: Vec<bool> = (0..np).map(|o| w.markets.active[o * N_GOODS + g]).collect();
        source(
            w.kernel(),
            g,
            &price,
            &active,
            &vec![[1.0, 0.0]; np],
            &mut out,
        );
        dispatch(w.kernel(), g, &price, &vec![[1.0; 2]; np], &mut out);
        for d in 0..np {
            let alpha: f64 = out.arrive.iter().map(|a| a[d][0]).sum();
            let r = alpha * w.price_index[d * N_GOODS + g];
            lo = lo.min(r);
            hi = hi.max(r);
            if d == np - 1 {
                let slots: Vec<String> = out
                    .arrive
                    .iter()
                    .map(|a| format!("{:.3}", a[d][0] / alpha))
                    .collect();
                eprintln!("island good {g}: r {r:.3} slots {slots:?}");
            }
        }
    }
    eprintln!("r over all destinations and goods: {lo:.3} to {hi:.3}");
}

/// V1 from the data state at 1,200 ticks with crossing lags 0 and as built, with and without
/// the island.
#[test]
#[ignore = "measurement harness"]
fn v1_variants() {
    let s = solution();
    for lag in [Some(0u8), None] {
        let mut w = EconWorld::new(
            Arc::new(small_world()),
            EconParams::frictionless(),
            TradeParams::default(),
        );
        if let Some(l) = lag {
            w.set_crossing_lags(l);
        }
        let mut sim = Sim::new(w, schedule(), SEED);
        sim.run_until(1200).unwrap();
        let w = sim.world();
        let island = w.inputs().n_provinces() - 1;
        let index = w.world_price_index();
        let (price, cell, wage) = v1_deviation(w, &s);
        let off = |c: usize| c / N_GOODS != island;
        let (mut price_off, mut cell_off) = (0.0, 0);
        for c in active(w).filter(|&c| off(c)) {
            let d = (w.markets.price[c] / index / s.price[c] - 1.0).abs();
            if d > price_off {
                (price_off, cell_off) = (d, c);
            }
        }
        let wage_off = (0..island)
            .map(|i| (w.wage[i] / index / s.wage[i] - 1.0).abs())
            .fold(0.0, f64::max);
        eprintln!(
            "V1 lag {lag:?}: price {price:.3e} (cell {cell}), outside island {price_off:.3e} \
             (cell {cell_off}); wage {wage:.3e}, outside island {wage_off:.3e}"
        );
    }
}

/// V1 deviations every 25 ticks of the run from the data state (`DEFAULT` for the default
/// mode, `LAG` for crossing lags).
#[test]
#[ignore = "measurement harness"]
fn data_state_path() {
    let s = solution();
    let params = if std::env::var_os("DEFAULT").is_some() {
        EconParams::default()
    } else {
        EconParams::frictionless()
    };
    let mut w = EconWorld::new(Arc::new(small_world()), params, TradeParams::default());
    if let Some(l) = lag() {
        w.set_crossing_lags(l);
    }
    let mut sim = Sim::new(w, schedule(), SEED);
    for t in (25..=1200).step_by(25) {
        let run = catch_unwind(AssertUnwindSafe(|| sim.run_until(t).unwrap()));
        if let Err(v) = run {
            eprintln!("lag {:?} t {t}: {:?}", lag(), v.downcast_ref::<String>());
            break;
        }
        let w = sim.world();
        let (price, _, wage) = v1_deviation(w, &s);
        eprintln!(
            "lag {:?} t {t}: price {price:.3e} wage {wage:.3e} index {:.4} raw {:.3e}",
            lag(),
            w.world_price_index(),
            distance(w, &s)
        );
    }
}

/// The (kappa1, kappa2) grid: growth per tick between ticks 300 and 600 of the distance from
/// the equilibrium and, in parentheses, of the gap to an unperturbed run.
#[test]
#[ignore = "measurement harness"]
fn grid() {
    let s = solution();
    for lag in [None, Some(3u8), Some(0u8)] {
        for k2 in [0.0, 0.05] {
            for k1 in [0.02, 0.05, 0.1, 0.2] {
                let mut cells = vec![];
                for params in [EconParams::frictionless(), EconParams::default()] {
                    let trade = TradeParams {
                        kappa1: k1,
                        kappa2: k2,
                        ..TradeParams::default()
                    };
                    let base = world(params, trade, lag, &s);
                    let mut p = Sim::new(perturbed(base.clone()), schedule(), SEED);
                    let mut u = Sim::new(base, schedule(), SEED);
                    let r = catch_unwind(AssertUnwindSafe(|| -> Result<[f64; 4], String> {
                        let err = |v| format!("{v}");
                        p.run_until(300).map_err(err)?;
                        u.run_until(300).map_err(err)?;
                        let (m_ref, m_gap) = (distance(p.world(), &s), gap(p.world(), u.world()));
                        p.run_until(600).map_err(err)?;
                        u.run_until(600).map_err(err)?;
                        let (e_ref, e_gap) = (distance(p.world(), &s), gap(p.world(), u.world()));
                        Ok([m_ref, e_ref, m_gap, e_gap])
                    }))
                    .unwrap_or_else(|e| {
                        let msg = e.downcast_ref::<String>().cloned();
                        let msg = msg.or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                        Err(msg.unwrap_or_default())
                    });
                    cells.push(match r {
                        Ok([a, b, c, d]) => format!(
                            "{:.4} ({:.1e}) / {:.4} ({:.1e})",
                            (b / a).powf(1.0 / 300.0),
                            b,
                            (d / c).powf(1.0 / 300.0),
                            d
                        ),
                        Err(e) => format!("fails: {}", &e[..e.len().min(60)]),
                    });
                }
                eprintln!(
                    "GRID lag {lag:?} k1 {k1} k2 {k2} | frictionless {} | default {}",
                    cells[0], cells[1]
                );
            }
        }
    }
}
