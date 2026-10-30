//! The benchmark harness (ADR 0200, ADR 0207): a calibrated pack run frictionless (V1) from its
//! steady state under [`crate::shock`]s, reduced to country aggregates, and the welfare, ACR and
//! openness measures computed from them.
//!
//! Welfare is the change in real income: d ln of the country's pop income less the
//! Omega-weighted d ln of its buyer prices. Home shares lambda are value shares of the
//! country's absorption of a traded good, both at origin prices over the last
//! `ceteris_econ::flows::WINDOW` ticks; absorption counts home purchases, domestic pairs and
//! imports.

use std::path::Path;
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::goods::{N_GOODS, TRADED};

use crate::shock::{self, Shock};
use crate::write::Table;

/// Ticks over which [`Outcome::max_dlnp_last`] is measured.
pub const LAST: u32 = 50;

/// Country aggregates at the end of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// Pop income per country, numeraire.
    pub income: Vec<f64>,
    /// Final demand shares Omega per country.
    pub omega: Vec<[f64; N_GOODS]>,
    /// Buyer prices, population-weighted over the country's provinces where finite; NaN
    /// where no province has one.
    pub buyer_price: Vec<[f64; N_GOODS]>,
    /// lambda: the home share of absorption by value; NaN for local goods and where
    /// absorption is 0.
    pub home_share: Vec<[f64; N_GOODS]>,
    /// Value bought by the country, at origin prices; 0 for local goods.
    pub absorption: Vec<[f64; N_GOODS]>,
    /// Largest |ln(p_T / p_{T - LAST})| over the active markets (from tick 0 if the run is
    /// shorter).
    pub max_dlnp_last: f64,
    pub ticks: u32,
}

/// ISO3 codes of the pack's countries, in row order.
pub fn codes(pack: &Path) -> Result<Vec<String>, String> {
    Table::open(&pack.join("econ_countries.arrow"))?.strings("code")
}

/// The calibrated world of `pack` with `shocks` applied, frictionless.
pub fn world(pack: &Path, shocks: &[Shock]) -> Result<EconWorld, String> {
    let graph = Pack::load(pack, &TransportParams::default()).map_err(|e| e.to_string())?;
    let mut inputs = EconInputs::from_pack(&graph, pack).map_err(|e| e.to_string())?;
    if inputs.steady.is_none() {
        return Err(format!("{}: no calibrated steady state", pack.display()));
    }
    let trade = inputs.trade_params();
    shock::apply(&mut inputs, shocks, &trade);
    let trade = shock::trade_params(&inputs, trade);
    Ok(EconWorld::new(
        Arc::new(inputs),
        EconParams::frictionless(),
        trade,
    ))
}

/// Runs the calibrated pack in `pack` under `shocks` for `ticks` ticks, frictionless from its
/// steady state.
pub fn run(pack: &Path, shocks: &[Shock], ticks: u32) -> Result<Outcome, String> {
    let mut sim = Sim::new(world(pack, shocks)?, schedule(), SEED);
    sim.run_until(ticks.saturating_sub(LAST))
        .map_err(|v| v.to_string())?;
    let before = sim.world().markets.price.clone();
    sim.run_until(ticks).map_err(|v| v.to_string())?;
    let w = sim.world();
    Ok(outcome(w, max_dlnp(w, &before), sim.tick()))
}

/// Largest |d ln p| over the active markets of `w` since prices `before`.
pub fn max_dlnp(w: &EconWorld, before: &[f64]) -> f64 {
    w.markets
        .price
        .iter()
        .zip(before)
        .zip(&w.markets.active)
        .filter(|&(_, &on)| on)
        .map(|((p, q), _)| (p / q).ln().abs())
        .fold(0.0, f64::max)
}

/// The country aggregates of `w` at its current tick.
pub fn outcome(w: &EconWorld, max_dlnp_last: f64, ticks: u32) -> Outcome {
    let inp = w.inputs();
    let (nc, np) = (inp.n_countries(), inp.n_provinces());
    let country = |i: usize| inp.country_of(i);

    let mut income = vec![0.0; nc];
    for k in 0..inp.n_pops() {
        income[country(inp.pops.province[k] as usize)] += w.income[k];
    }
    let (mut q, mut qw) = (vec![[0.0; N_GOODS]; nc], vec![[0.0; N_GOODS]; nc]);
    for i in 0..np {
        let c = country(i);
        for g in 0..N_GOODS {
            let p = w.price_index[i * N_GOODS + g];
            if p.is_finite() {
                q[c][g] += inp.provinces.population[i] * p;
                qw[c][g] += inp.provinces.population[i];
            }
        }
    }
    let buyer_price = q
        .iter()
        .zip(&qw)
        .map(|(q, w)| std::array::from_fn(|g| if w[g] > 0.0 { q[g] / w[g] } else { f64::NAN }))
        .collect();

    let mean = w.scratch.flows.mean();
    let (mut home, mut absorption) = (vec![[0.0; N_GOODS]; nc], vec![[0.0; N_GOODS]; nc]);
    for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
        for ex in 0..nc {
            for im in 0..nc {
                let v = mean.pair_value[(ex * nc + im) * N_GOODS + g];
                absorption[im][g] += v;
                if ex == im {
                    home[im][g] += v;
                }
            }
        }
        for i in 0..np {
            let v = mean.home_value[i * N_GOODS + g];
            absorption[country(i)][g] += v;
            home[country(i)][g] += v;
        }
    }
    let home_share = home
        .iter()
        .zip(&absorption)
        .map(|(h, a)| std::array::from_fn(|g| if a[g] > 0.0 { h[g] / a[g] } else { f64::NAN }))
        .collect();
    Outcome {
        income,
        omega: inp.countries.omega.clone(),
        buyer_price,
        home_share,
        absorption,
        max_dlnp_last,
        ticks,
    }
}

/// Welfare change per country from `base` to `shocked`: d ln income less the Omega-weighted
/// d ln buyer prices, over the goods with Omega > 0.
pub fn welfare(base: &Outcome, shocked: &Outcome) -> Vec<f64> {
    (0..base.income.len())
        .map(|c| {
            let prices: f64 = (0..N_GOODS)
                .filter(|&g| base.omega[c][g] > 0.0)
                .map(|g| {
                    base.omega[c][g] * (shocked.buyer_price[c][g] / base.buyer_price[c][g]).ln()
                })
                .sum();
            (shocked.income[c] / base.income[c]).ln() - prices
        })
        .collect()
}

/// The multi-sector ACR welfare change per country (Arkolakis, Costinot and Rodriguez-Clare
/// 2012): -sum over traded g of Omega_cg / theta_g d ln lambda_cg. A good the country buys
/// nothing of at home in both runs adds nothing; one it buys at home in one run only gives NaN.
pub fn acr(base: &Outcome, shocked: &Outcome, theta: &[f64; N_GOODS]) -> Vec<f64> {
    (0..base.income.len())
        .map(|c| {
            -(0..N_GOODS)
                .filter(|&g| TRADED[g] && base.omega[c][g] > 0.0)
                .map(|g| {
                    let (l0, l1) = (base.home_share[c][g], shocked.home_share[c][g]);
                    let none = |l: f64| l.is_nan() || l <= 0.0;
                    if none(l0) && none(l1) {
                        0.0
                    } else {
                        base.omega[c][g] / theta[g] * (l1 / l0).ln()
                    }
                })
                .sum::<f64>()
        })
        .collect()
}

/// The world's absorption-weighted foreign share 1 - lambda of `goods`, over the countries
/// with absorption.
pub fn foreign_share(o: &Outcome, goods: &[usize]) -> f64 {
    let (mut abroad, mut total) = (0.0, 0.0);
    for (l, a) in o.home_share.iter().zip(&o.absorption) {
        for &g in goods {
            if a[g] > 0.0 {
                abroad += (1.0 - l[g]) * a[g];
                total += a[g];
            }
        }
    }
    abroad / total
}
