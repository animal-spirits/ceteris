//! W5: literature baseline on the fixture (`reports/toy-models-and-literature-baseline-2026-11-11.md`
//! section 3). Research profile of revert switches, each citing the ADR it reverts, all off by
//! default; the matrix runs only in the ignored test. Default behaviour and goldens are untouched.
//!
//! `cargo test -p ceteris-econ --release --test literature_baseline -- --ignored --nocapture`

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::params::Transfers;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;
use ceteris_trade::network::Wedges;

const TICKS: u32 = 1200;
const THETA_SW: f64 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    /// ADR 0274: one Simonovska-Waugh theta for every traded good.
    Theta,
    /// ADR 0092: no fitted border nu.
    BorderNu,
    /// ADR 0112: no loss write-off.
    WriteOff,
    /// ADR 0100: no trade-imbalance transfers.
    Transfers,
    /// ADR 0338: aggregate price rule, no money anchor (already the default; listed so the
    /// matrix shows the anchored side too).
    NoAnchor,
}

// ADR 0247 (bulk rate) is not a switch here: the fixture network carries its bulk legs, so
// reverting it is a network rebuild (`unreachable_island` folds them in); build-time.
const ALL: [Switch; 5] = [
    Switch::Theta,
    Switch::BorderNu,
    Switch::WriteOff,
    Switch::Transfers,
    Switch::NoAnchor,
];

fn apply(on: &[Switch], mut p: EconParams, mut t: TradeParams) -> (EconParams, TradeParams) {
    for s in on {
        match s {
            Switch::Theta => {
                for g in 0..t.goods.n() {
                    if t.goods.is_traded(g) {
                        t.theta[g] = THETA_SW;
                    }
                }
            }
            Switch::BorderNu => t.border_nu.iter_mut().for_each(|v| *v = 0.0),
            Switch::WriteOff => p.write_off_months = f64::INFINITY,
            Switch::Transfers => p.transfers = Transfers::Off,
            Switch::NoAnchor => t.anchor = None,
        }
    }
    (p, t)
}

/// From the data state: the reference steady state is hashed to main's parameters, so it cannot
/// be settled under a reverted switch.
fn data_state(p: EconParams, t: TradeParams) -> EconWorld {
    EconWorld::new(Arc::new(small_world()), p, t)
}

struct Outcome {
    /// Tick of the first violation, if any.
    failed_at: Option<u32>,
    /// Log points the world price index moved from its initial value at the end or at failure.
    index_drift: f64,
}

fn run(on: &[Switch], frictionless: bool, x10: bool) -> Outcome {
    let p = if frictionless {
        EconParams::frictionless()
    } else {
        EconParams::default()
    };
    let (p, t) = apply(on, p, TradeParams::default());
    let mut w = data_state(p, t);
    let i0 = w.world_price_index();
    if x10 {
        let mut wedges = Wedges::none(w.inputs().n_countries());
        wedges.ntb = vec![10.0; wedges.tariff.len()];
        w.set_wedges(wedges);
    }
    let mut sim = Sim::new(w, schedule(), SEED);
    let mut failed_at = None;
    let mut last = i0;
    for tick in 1..=TICKS {
        // A ledger overflow panics instead of returning a violation; both count as failure.
        let step = catch_unwind(AssertUnwindSafe(|| sim.run_until(tick)));
        if !matches!(step, Ok(Ok(()))) {
            failed_at = Some(tick);
            break;
        }
        last = sim.world().world_price_index();
    }
    Outcome {
        failed_at,
        index_drift: libm::log(last / i0),
    }
}

#[test]
fn default_has_every_revertible_switch_in_its_default_position_or_off() {
    let t = TradeParams::default();
    assert!(t.anchor.is_none());
    // ADR 0280: rho = 1 is the ADR 0091 competitive-storage form.
    assert!(!t.partial_adjustment());
    assert!(t.bulk_off().bulk_rate().is_none());
}

#[test]
fn switches_change_only_their_own_field() {
    let (p0, t0) = (EconParams::default(), TradeParams::default());
    let (p, t) = apply(&[Switch::WriteOff], p0.clone(), t0.clone());
    assert!(p.write_off_months.is_infinite());
    assert_eq!(t, t0);
    let (p, t) = apply(&[Switch::Theta, Switch::BorderNu], p0.clone(), t0.clone());
    assert_eq!(p, p0);
    assert_ne!(t.theta, t0.theta);
}

fn name(on: &[Switch]) -> String {
    if on.is_empty() {
        "main".into()
    } else if on.len() == ALL.len() {
        "literature".into()
    } else if on.len() == ALL.len() - 1 {
        let off = ALL.iter().find(|s| !on.contains(s)).unwrap();
        format!("literature minus {off:?}")
    } else {
        format!("main plus {on:?}")
    }
}

#[test]
#[ignore = "research matrix, minutes in release"]
fn literature_matrix() {
    let mut configs: Vec<Vec<Switch>> = vec![vec![], ALL.to_vec()];
    configs.extend(ALL.iter().map(|&s| vec![s]));
    configs.extend(
        ALL.iter()
            .map(|&s| ALL.iter().copied().filter(|&o| o != s).collect()),
    );
    std::panic::set_hook(Box::new(|i| eprintln!("panic: {i}")));
    println!("| config | mode | shock | failed_at | index_drift |");
    for on in &configs {
        for (mode, fl) in [("default", false), ("frictionless", true)] {
            for (shock, x10) in [("none", false), ("x10", true)] {
                let o = run(on, fl, x10);
                let f = o.failed_at.map_or("-".into(), |t| t.to_string());
                println!(
                    "| {} | {mode} | {shock} | {f} | {:.4} |",
                    name(on),
                    o.index_drift
                );
            }
        }
    }
}
