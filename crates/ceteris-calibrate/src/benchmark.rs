//! The benchmark harness (ADR 0200, ADR 0207): a calibrated pack run frictionless (V1) from its
//! steady state under [`crate::shock`]s, reduced to country aggregates, and the welfare, ACR and
//! openness measures computed from them.
//!
//! Welfare is the change in real income: d ln of the country's pop income less the
//! Omega-weighted d ln of its buyer prices. Home shares lambda are value shares of the
//! country's absorption of a traded good, both at origin prices over the last
//! `ceteris_econ::flows::WINDOW` ticks; absorption counts home purchases, domestic pairs and
//! imports. Two approximations, both exact to first order: a country's buyer price is the
//! population-weighted mean of its provinces' price levels, and Omega is the data's final
//! demand share, not the run's expenditure share.
//!
//! [`acr`] is the ACR prediction without input-output linkages, [`acr_io`] the one with them;
//! the model has Leontief intermediates, so its welfare follows the second. ADR 0207's
//! criterion grades [`acr_io_province`] against [`welfare_province`] (ADR 0268): the
//! input-output formula on each province's own home share, and the model's real income change
//! per province, both reduced to countries with base-run income weights (Ramondo,
//! Rodriguez-Clare and Saborio-Rodriguez 2016, Proposition 4).

use std::path::Path;
use std::sync::Arc;

use ceteris_core::rows::Rows;
use ceteris_core::sim::Sim;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;

use crate::shock::{self, Shock};
use crate::write::Table;

/// Ticks over which [`Outcome::max_dlnp_last`] is measured.
pub const LAST: u32 = 50;

/// Country aggregates at the end of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// Pop income per country, numeraire.
    pub income: Vec<f64>,
    /// Goods `0..traded` are traded.
    pub traded: usize,
    /// Final demand shares Omega per country.
    pub omega: Rows<f64>,
    /// Buyer prices, population-weighted over the country's provinces where finite; NaN
    /// where no province has one.
    pub buyer_price: Rows<f64>,
    /// lambda: the home share of absorption by value; NaN for local goods and where
    /// absorption is 0.
    pub home_share: Rows<f64>,
    /// Value bought by the country, at origin prices; 0 for local goods.
    pub absorption: Rows<f64>,
    /// Country of each province.
    pub province_country: Vec<usize>,
    /// Pop income per province, numeraire.
    pub province_income: Vec<f64>,
    /// Buyer price per province; not finite where the province has none.
    pub province_buyer_price: Rows<f64>,
    /// lambda per province: what it buys from itself over what it buys from every origin, by
    /// value at origin prices; NaN for local goods and where its absorption is 0.
    pub province_home_share: Rows<f64>,
    /// Largest |ln(p_T / p_{T - LAST})| over the active markets (from tick 0 if the run is
    /// shorter).
    pub max_dlnp_last: f64,
    pub ticks: u32,
}

/// ISO3 codes of the pack's countries, in row order.
pub fn codes(pack: &Path) -> Result<Vec<String>, String> {
    Table::open(&pack.join("econ_countries.arrow"))?.strings("code")
}

/// The pack's goods count (`econ_goods` rows).
pub fn n_goods(pack: &Path) -> Result<usize, String> {
    Ok(Table::open(&pack.join("econ_goods.arrow"))?.rows())
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
    let (ng, nt) = (inp.goods.n(), inp.goods.traded);
    let country = |i: usize| inp.country_of(i);

    let mut province_income = vec![0.0; np];
    for k in 0..inp.n_pops() {
        province_income[inp.pops.province[k] as usize] += w.income[k];
    }
    let mut income = vec![0.0; nc];
    for (i, y) in province_income.iter().enumerate() {
        income[country(i)] += y;
    }
    let (mut q, mut qw) = (Rows::filled(nc, ng, 0.0), Rows::filled(nc, ng, 0.0));
    for i in 0..np {
        let c = country(i);
        for g in 0..ng {
            let p = w.price_index[i * ng + g];
            if p.is_finite() {
                q[c][g] += inp.provinces.population[i] * p;
                qw[c][g] += inp.provinces.population[i];
            }
        }
    }
    let buyer_price = ratio(&q, &qw);

    let mean = w.scratch.flows.mean();
    let (mut home, mut absorption) = (Rows::filled(nc, ng, 0.0), Rows::filled(nc, ng, 0.0));
    for g in 0..nt {
        for ex in 0..nc {
            for im in 0..nc {
                let v = mean.pair_value[(ex * nc + im) * ng + g];
                absorption[im][g] += v;
                if ex == im {
                    home[im][g] += v;
                }
            }
        }
        for i in 0..np {
            let v = mean.home_value[i * ng + g];
            absorption[country(i)][g] += v;
            home[country(i)][g] += v;
        }
    }
    let home_share = ratio(&home, &absorption);
    let traded_only = |v: &[f64]| {
        let flat = v
            .iter()
            .enumerate()
            .map(|(k, &x)| if k % ng < nt { x } else { 0.0 })
            .collect();
        Rows::from_flat(flat, ng)
    };
    let province_home_share = ratio(
        &traded_only(&mean.home_value),
        &traded_only(&mean.absorption),
    );
    Outcome {
        traded: nt,
        income,
        omega: inp.countries.omega.clone(),
        buyer_price,
        home_share,
        absorption,
        province_country: (0..np).map(country).collect(),
        province_income,
        province_buyer_price: Rows::from_flat(w.price_index[..np * ng].to_vec(), ng),
        province_home_share,
        max_dlnp_last,
        ticks,
    }
}

/// `num / den` per entry where `den > 0`, else NaN.
fn ratio(num: &Rows<f64>, den: &Rows<f64>) -> Rows<f64> {
    let flat = num
        .as_flat()
        .iter()
        .zip(den.as_flat())
        .map(|(n, d)| if *d > 0.0 { n / d } else { f64::NAN })
        .collect();
    Rows::from_flat(flat, num.width())
}

/// Welfare change per country from `base` to `shocked`: d ln income less the Omega-weighted
/// d ln buyer prices, over the goods with Omega > 0.
pub fn welfare(base: &Outcome, shocked: &Outcome) -> Vec<f64> {
    (0..base.income.len())
        .map(|c| {
            let prices: f64 = (0..base.omega.width())
                .filter(|&g| base.omega[c][g] > 0.0)
                .map(|g| {
                    base.omega[c][g] * (shocked.buyer_price[c][g] / base.buyer_price[c][g]).ln()
                })
                .sum();
            (shocked.income[c] / base.income[c]).ln() - prices
        })
        .collect()
}

/// Welfare change per country from `base` to `shocked` reduced from its provinces (ADR 0268):
/// sum over provinces p of s_p (d ln I_p - sum over g with Omega_cg > 0 of Omega_cg d ln q_pg),
/// s_p = I_p / I_c in `base`. A province with no base income adds nothing; one with no buyer
/// price for a good with Omega_cg > 0 makes its country NaN.
pub fn welfare_province(base: &Outcome, shocked: &Outcome) -> Vec<f64> {
    by_province(base, |p, c| {
        let prices: f64 = (0..base.omega.width())
            .filter(|&g| base.omega[c][g] > 0.0)
            .map(|g| {
                base.omega[c][g]
                    * (shocked.province_buyer_price[p][g] / base.province_buyer_price[p][g]).ln()
            })
            .sum();
        (shocked.province_income[p] / base.province_income[p]).ln() - prices
    })
}

/// sum over provinces p of country c of s_p f(p, c), s_p = I_p / I_c in `base`, per country;
/// provinces with no base income are left out.
fn by_province(base: &Outcome, f: impl Fn(usize, usize) -> f64) -> Vec<f64> {
    let mut out = vec![0.0; base.income.len()];
    for (p, (&c, &y)) in base
        .province_country
        .iter()
        .zip(&base.province_income)
        .enumerate()
        .filter(|(_, (_, y))| **y > 0.0)
    {
        out[c] += y / base.income[c] * f(p, c);
    }
    out
}

/// The multi-sector ACR welfare change per country (Arkolakis, Costinot and Rodriguez-Clare
/// 2012): -sum over traded g of Omega_cg / theta_g d ln lambda_cg, over the goods with
/// Omega > 0. A good the country buys nothing of at home in both runs adds nothing; one it buys
/// at home in one run only makes the country's value NaN.
pub fn acr(base: &Outcome, shocked: &Outcome, theta: &[f64]) -> Vec<f64> {
    (0..base.income.len())
        .map(|c| {
            weighted_dln_home(
                base.traded,
                &base.home_share[c],
                &shocked.home_share[c],
                &base.omega[c],
                theta,
            )
        })
        .collect()
}

/// The multi-sector ACR welfare change per country with input-output linkages (Costinot and
/// Rodriguez-Clare 2014, section 3.4, eq. (28) under perfect competition, delta_k = 0, as in
/// NBER WP 18896; Caliendo and Parro 2015): d ln W_c = -Omega_c' (I - A_c')^-1 diag(1/theta)
/// d ln lambda_c, with `a[c]` the country's cost shares a_kg of input k in sector g at
/// `[k * n + g]` (`Countries::io`, which the calibrated coefficients reproduce at the
/// calibrated prices). Local goods have no lambda term but pass cost changes on through
/// (I - A_c')^-1. Goods with a zero weight in Omega_c' (I - A_c')^-1 add nothing; otherwise as
/// [`acr`], which this is when A_c is 0.
pub fn acr_io(base: &Outcome, shocked: &Outcome, theta: &[f64], a: &Rows<f64>) -> Vec<f64> {
    (0..base.income.len())
        .map(|c| {
            let weight = leontief_weights(&base.omega[c], &a[c]);
            weighted_dln_home(
                base.traded,
                &base.home_share[c],
                &shocked.home_share[c],
                &weight,
                theta,
            )
        })
        .collect()
}

/// [`acr_io`] graded per province (ADR 0268; Ramondo, Rodriguez-Clare and Saborio-Rodriguez
/// 2016, eq. (18)): d ln W_p = -Omega_c' (I - A_c')^-1 diag(1/theta) d ln lambda_p on each
/// province's own home share, with its country's Omega and A, reduced to countries as
/// sum_p s_p d ln W_p, s_p = I_p / I_c in `base`. A province NaN makes its country NaN; a
/// province with no base income adds nothing.
pub fn acr_io_province(
    base: &Outcome,
    shocked: &Outcome,
    theta: &[f64],
    a: &Rows<f64>,
) -> Vec<f64> {
    let weight: Vec<Vec<f64>> = (0..base.income.len())
        .map(|c| leontief_weights(&base.omega[c], &a[c]))
        .collect();
    by_province(base, |p, c| {
        weighted_dln_home(
            base.traded,
            &base.province_home_share[p],
            &shocked.province_home_share[p],
            &weight[c],
            theta,
        )
    })
}

/// -sum over traded g < `traded` with weight > 0 of weight_g / theta_g d ln lambda_g from
/// `base` to `shocked` home shares, as [`acr`].
fn weighted_dln_home(
    traded: usize,
    base: &[f64],
    shocked: &[f64],
    weight: &[f64],
    theta: &[f64],
) -> f64 {
    -(0..traded)
        .filter(|&g| weight[g] > 0.0)
        .map(|g| {
            let (l0, l1) = (base[g], shocked[g]);
            let none = |l: f64| l.is_nan() || l <= 0.0;
            match (none(l0), none(l1)) {
                (true, true) => 0.0,
                (false, false) => weight[g] / theta[g] * (l1 / l0).ln(),
                _ => f64::NAN,
            }
        })
        .sum::<f64>()
}

/// v solving (I - A) v = omega, that is v' = omega' (I - A')^-1, A at `[k * n + g]`:
/// Gaussian elimination with partial pivoting.
fn leontief_weights(omega: &[f64], a: &[f64]) -> Vec<f64> {
    let n = omega.len();
    assert_eq!(a.len(), n * n, "input-output table");
    let mut m: Vec<Vec<f64>> = (0..n)
        .map(|k| {
            (0..=n)
                .map(|g| match g {
                    _ if g == n => omega[k],
                    _ if g == k => 1.0 - a[k * n + g],
                    _ => -a[k * n + g],
                })
                .collect()
        })
        .collect();
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))
            .unwrap_or(col);
        m.swap(col, pivot);
        let top = m[col].clone();
        for row in &mut m[col + 1..] {
            let f = row[col] / top[col];
            for (x, t) in row[col..].iter_mut().zip(&top[col..]) {
                *x -= f * t;
            }
        }
    }
    let mut v = vec![0.0; n];
    for k in (0..n).rev() {
        let rest: f64 = (k + 1..n).map(|j| m[k][j] * v[j]).sum();
        v[k] = (m[k][n] - rest) / m[k][k];
    }
    v
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
