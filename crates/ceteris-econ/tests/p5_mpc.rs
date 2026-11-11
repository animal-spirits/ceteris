//! P5 Branch H, the intertemporal MPC harness of ADR 0025: the response of a pop group's
//! spending in every later tick to a one-off transfer, from the real schedule.
use std::path::Path;
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::equilibrium::Equilibrium;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::world::{MINT, to_numeraire};
use ceteris_econ::{EconParams, EconWorld};
use ceteris_trade::TradeParams;

const HORIZON: usize = 6;
/// Transfer as a share of the group's balance, small so the response is the linear one.
const SHOCK: f64 = 1e-3;

fn settled() -> EconWorld {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture/reference");
    let mut w = EconWorld::new(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    );
    w.settle(&Equilibrium::read(&dir).unwrap());
    w
}

/// Spending of the pops `group` at the tick just run, in numeraire: notional demand at buyer prices.
fn spending(w: &EconWorld, group: &[usize]) -> f64 {
    let ng = w.inputs().n_goods();
    group
        .iter()
        .map(|&k| {
            let i = w.inputs().pops.province[k] as usize;
            (0..ng)
                .map(|g| w.scratch.pop_demand[k * ng + g] * w.price_index[i * ng + g])
                .sum::<f64>()
        })
        .sum()
}

/// Spending by tick over the horizon, with `shock` = (tick, minor units) paid from the mint
/// to each pop of the group before that tick.
fn path(group: &[usize], shock: Option<(usize, i64)>) -> Vec<f64> {
    let mut sim = Sim::new(settled(), schedule(), SEED);
    (0..HORIZON)
        .map(|t| {
            if let Some((_, amount)) = shock.filter(|&(at, _)| at == t) {
                let w = sim.world_mut();
                let acc = w.layout().accounts.clone();
                for &k in group {
                    w.ledger.transfer(MINT, acc.mint, acc.pop[k], amount);
                }
            }
            sim.run_until(t as u32 + 1).unwrap();
            spending(sim.world(), group)
        })
        .collect()
}

/// M[t][s]: spending at tick t per unit of numeraire transferred at tick s.
fn mpc_matrix(group: &[usize]) -> Vec<Vec<f64>> {
    let w = settled();
    let acc = w.layout().accounts.clone();
    let balance: i64 = group.iter().map(|&k| w.ledger.money(acc.pop[k])).sum();
    let each = (balance as f64 * SHOCK / group.len() as f64) as i64;
    let paid = to_numeraire(each * group.len() as i64);
    let base = path(group, None);
    let columns: Vec<Vec<f64>> = (0..HORIZON)
        .map(|s| {
            let shocked = path(group, Some((s, each)));
            shocked
                .iter()
                .zip(&base)
                .map(|(x, b)| (x - b) / paid)
                .collect()
        })
        .collect();
    let m: Vec<Vec<f64>> = (0..HORIZON)
        .map(|t| columns.iter().map(|c| c[t]).collect())
        .collect();
    m
}

/// ADR 0025 closed form at m = 1, lambda = 0: pops spend their whole balance, so a transfer is
/// spent in the tick it arrives (the diagonal is 1: the injection bypasses the tax and saving
/// split of ADR 0099, which is why the plan's `1 - s_c - g_c` factor does not appear), nothing
/// is spent before it arrives, and the matrix depends on the lag only (settled state). The
/// entries below the diagonal are the general-equilibrium feedback of the spending returning as
/// income, reported and not asserted: the household block alone is the identity.
#[test]
fn closed_form_case_mpc_matrix_is_scaled_identity() {
    let n = settled().inputs().n_pops();
    let group: Vec<usize> = (0..n).collect();
    let m = mpc_matrix(&group);
    for t in 0..HORIZON {
        for s in 0..HORIZON {
            if t < s {
                assert!(
                    m[t][s].abs() < 1e-9,
                    "spent before received: M[{t}][{s}] = {}",
                    m[t][s]
                );
            } else if t == s {
                assert!(
                    (m[t][s] - 1.0).abs() < 1e-6,
                    "impact MPC M[{t}][{t}] = {}",
                    m[t][s]
                );
            }
            if t >= 1 && s >= 1 {
                assert!(
                    (m[t][s] - m[t - 1][s - 1]).abs() < 1e-9,
                    "not Toeplitz at ({t}, {s})"
                );
            }
        }
    }
}
