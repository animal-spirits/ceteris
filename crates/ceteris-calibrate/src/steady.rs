//! Data-state steady-state calibration (ADR 0089; `specs/econ/SPEC.md`, Calibration): the 2000
//! values are held fixed and prices are solved first, so the data state is the steady state.
//!
//! Stage 1, values. Output value V = VA / (1 - sum_k a_kg) per traded enterprise, VA the pack's.
//! Destinations spend on traded goods E_dg = households' PIGL spending at q_d of their income
//! (their province's wages s_L VA by effective labour share; their country's profits (1 - s_L) VA
//! and tariff rebate by population, national ownership) plus sum a_kg V of the province's
//! enterprises; country profits carry the spoilage of competitively stored goods, V delta s / (1 +
//! delta s) at their origins (ADR 0091). One scalar r_g per traded good rescales its V (and VA)
//! so that world spending net of tariffs equals world sales value, V / (1 + delta s). Spending scales with the r, so that fixes them only up to
//! one common factor; each pass rescales them so that world traded value added, sum_e r_g VA_e, is
//! the data's (ADR 0106: the fixed point is then unique, not one of a ray). Rebates depend on
//! sourcing, local output on income and income on local output, so passes of stages 1, 2 and 4
//! repeat until rebates, r, buyer prices and the PIGL weights settle ([`PASS_TOL`]).
//!
//! Stage 2, origin prices. Per traded good, the balanced scaling
//! w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o / (1 + delta_g s_g), Phi_d = sum_o K_od w_o,
//! on the two-level
//! kernel ([`sinkhorn`]), unique up to scale (Sinkhorn 1967). Then p_o = w_o^(-1/theta), one
//! scale per good, set so that the good's part of the world price index (base consumption
//! weights at the calibrated incomes) is 1; local goods take p = 1, so the index is 1. q_d, the
//! arrival slots and the lead L_d follow from sourcing (`Model::sourcing`).
//!
//! Stage 3, technology. coef_kg = a_kg p_o / q_ik (the 2000 cost shares at the solved prices),
//! a_VA = 1 - sum_k a_kg, so p_VA = p_o, A = 1 / p_VA and Y = V / p_o. Local output solves the
//! Leontief system of each province against its local demand (households at p = 1 and every
//! enterprise's a_kg V), and its VA, a_VA Y, is an output of calibration. w0 = sum_g s_L VA0 /
//! ((1 - u*) L^s) and L0 = s_L VA0 / w0.
//!
//! Stage 4, demand. Per country, PIGL weights inverted jointly over its pops, each at its own
//! province's buyer prices q and its income per person, weighted by its income, so that their
//! aggregate shares are the country's final demand shares (`demand::invert_pops`, ADR 0094);
//! every pop spends at the country's within-pop dispersion (`Inputs::within_sigma`), and
//! households then spend by PIGL at their own province's q and income, which is the E of stage 1.
//!
//! Stage 5, the state at the start of a tick (Rust `systems`): input stocks (1 + COVER) months
//! of use; the pipeline holds, j ticks ahead, one month of shipments of every arrival slot j or
//! later; merchant stocks coverage times sales (S* = coverage D; D = Q, or Q / (1 + delta s) for
//! the competitively stored goods, ADR 0091); pops hold one month
//! of income less the wages of the tick (dividends and rebates); pi^e = 0.
//!
//! Sums run in a fixed order (sequential per index, pairwise row sums), so the steady state is
//! reproducible to the last bit.

use std::f64::consts::LN_10;
use std::fmt;
use std::time::Instant;

use ceteris_econ::EconParams;
use ceteris_reference::demand::{self, N_CATEGORIES, Weights};
use ceteris_reference::io::{Inputs, Mat};
use ceteris_reference::linalg::lstsq;
use ceteris_reference::model::{self, Factors, Model, Sourcing, max_abs};
use ceteris_trade::{N_GOODS, TradeParams};

use crate::{block, elasticity};

/// P149, the order rule's cover (`systems::COVER`): stage 5's stocks and the block gain's
/// order-up-to position hold the months of use the run's order rule holds.
pub const COVER: f64 = ceteris_econ::systems::COVER;
/// `pipeline::SLOTS`: arrival slots 1 to 4 enter the pipeline, slot 0 is home.
pub const SLOTS: usize = 4;
/// Largest |ln(received / target)| over origins that ends the scaling.
pub const SINKHORN_TOL: f64 = 1e-13;
/// The cap on sweeps of one scaling: an iteration cap, not a tolerance. 100,000 since the B1
/// thetas, for a slow mode of coal and other mining (theta 16.91). On the current full pack it
/// is Brazil's 48 origins of that good, a near-closed block that keeps 0.999 of its Sinkhorn
/// coupling: the plain sweep contracts it by 0.99908 a sweep (the second eigenvalue of the map's
/// Jacobian, `sinkhorn_is_unique_up_to_scale`). The cap was set when an earlier pack's mode
/// (province 3906, 0.99995 a sweep) needed 34,614 sweeps in pass 7.
pub const MAX_SWEEPS: usize = 100_000;
pub const ANDERSON_MEMORY: usize = 8;
/// Sweeps without a new smallest residual before the mixing restarts.
pub const ANDERSON_STALL: usize = 20;
/// Largest relative change of rebates, r, buyer prices and PIGL weights that ends the passes
/// (P158): 1e-13, so the returned state is a fixed point to the 1e-12 its checks claim (r
/// against the scaling at its own spending, `sinkhorn_is_unique_up_to_scale`; stage 5's
/// Q = D, whose gap is about 3.8 times the last pass change plus about 3e-13 of rounding, so
/// the tolerance must be at most 1.8e-13); a solver tolerance, not a model parameter.
pub const PASS_TOL: f64 = 1e-13;
pub const MAX_PASSES: usize = 100;
/// Largest relative change of any local output that ends the income fixed point.
pub const LOCAL_TOL: f64 = 1e-14;
pub const MAX_LOCAL: usize = 1_000;
/// The `calibration_format` of the tables (`ceteris_econ::calibration::FORMAT`).
/// 9: the block limit gains (ADR 0267), and origins of competitively stored goods sell
/// Q / (1 + delta s) and hold coverage times sales, with the storage response in E_cell and the
/// block map (ADR 0091). Each branch numbered its own tables 8 before the merge. 10: the
/// metadata's `border_nu` is one value per good (ADR 0276).
pub const FORMAT: &str = "10";

/// Iteration caps of the passes, the scaling and the income fixed point.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_passes: usize,
    pub max_sweeps: usize,
    pub max_local: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_passes: MAX_PASSES,
            max_sweeps: MAX_SWEEPS,
            max_local: MAX_LOCAL,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CalibrationError {
    /// The passes ran out with the fixed point still moving: how many ran and the last change.
    NotSettled {
        passes: usize,
        change: f64,
    },
    Failed(String),
}

impl fmt::Display for CalibrationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CalibrationError::NotSettled { passes, change } => write!(
                f,
                "passes did not settle in {passes}, last change {}",
                sci(*change)
            ),
            CalibrationError::Failed(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for CalibrationError {}

impl From<String> for CalibrationError {
    fn from(s: String) -> Self {
        CalibrationError::Failed(s)
    }
}

impl From<&str> for CalibrationError {
    fn from(s: &str) -> Self {
        CalibrationError::Failed(s.into())
    }
}

/// Python's `f"{x:.3e}"`.
pub fn sci(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return format!("{x}");
    }
    let s = format!("{x:.3e}");
    let (mantissa, exp) = s.split_once('e').expect("exponent notation");
    let (sign, digits) = exp.strip_prefix('-').map_or(("+", exp), |d| ("-", d));
    format!("{mantissa}e{sign}{digits:0>2}")
}

/// The scaling of stage 2.
#[derive(Clone, Debug)]
pub struct Scaling {
    /// (P, G) ln w of active traded origins, 0 elsewhere.
    pub log_weight: Mat,
    /// (G,) rescale of each traded good's output value, 1 for local goods.
    pub r: Vec<f64>,
    pub sweeps: usize,
    /// max |ln(received / (r V))|.
    pub residual: f64,
}

/// One pass of the fixed point.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    pub pass: usize,
    pub change: f64,
    pub sweeps: usize,
    pub sinkhorn_residual: f64,
}

#[derive(Clone, Debug)]
pub struct Info {
    pub r: Vec<f64>,
    pub passes: usize,
    pub pass_history: Vec<Pass>,
    /// Per good, the smallest origin weight relative to the largest, log10; None for local
    /// goods.
    pub min_log10_relative_weight: Vec<Option<f64>>,
    pub seconds: f64,
}

impl Info {
    pub fn sinkhorn_residual(&self) -> f64 {
        self.pass_history
            .last()
            .map_or(f64::NAN, |p| p.sinkhorn_residual)
    }
}

/// The calibrated steady state. G goods, P provinces, C countries, E enterprises, K pops.
#[derive(Clone, Debug)]
pub struct Steady {
    /// (C,)
    pub u_star: Vec<f64>,
    /// (G,)
    pub r: Vec<f64>,
    /// (P, G) posted: origin markets of traded goods, 1 for local goods, 0 elsewhere.
    pub price: Mat,
    /// (P, G) buyer price q of traded goods, the local price of local goods.
    pub consumer: Mat,
    /// (P, G) selection constant Gamma = q / P_d of traded goods at the calibrated prices
    /// (ADR 0200), 1 for local goods and where no origin is reachable.
    pub gamma: Mat,
    /// (P, G) expected months in transit of traded goods.
    pub lead: Mat,
    /// (P, G) the own-price elasticity E and the supply elasticity eps of each market's price
    /// loop (ADR 0263, [`crate::elasticity`]), 0 where no market is active.
    pub demand_elasticity: Mat,
    pub supply_elasticity: Mat,
    /// (P, G) the limit gain of each market's coupled block (ADR 0267, [`crate::block::limits`])
    /// in the default mode and frictionless, infinite for a block within its margin and outside
    /// every block.
    pub block_limit: [Mat; 2],
    /// SLOTS + 1 matrices (P, G): units arriving per unit spent, by arrival slot.
    pub arrive: Vec<Mat>,
    pub weights: Weights,
    /// (C,) tariffs per month.
    pub rebate: Vec<f64>,
    /// (E,) output value V.
    pub value: Vec<f64>,
    /// (E,) Y.
    pub output: Vec<f64>,
    /// (E, G)
    pub coef: Mat,
    pub a_va: Vec<f64>,
    pub a: Vec<f64>,
    pub va0: Vec<f64>,
    pub l0: Vec<f64>,
    /// (P,)
    pub w0: Vec<f64>,
    /// (E, G) traded input spending per month.
    pub ent_spend: Mat,
    /// (K,)
    pub pop_income: Vec<f64>,
    pub pop_wage: Vec<f64>,
    /// (K, G)
    pub pop_spend: Mat,
    /// (P, G) household spending (local goods: units at p = 1).
    pub spend_hh: Mat,
    /// (P, G) traded input spending.
    pub spend_int: Mat,
    /// (P, G) Q of every active cell: D = sales, but (1 + delta s) D at the origins of the
    /// competitively stored goods (ADR 0091).
    pub supply: Mat,
    /// Traded orders at the origin in units (P, G): intermediate, household.
    pub orders: [Mat; 2],
    /// (G,) P02.
    pub coverage: Vec<f64>,
    /// (G,) delta_g s_g (P56, P02) of the competitively stored goods, 0 elsewhere (ADR 0091).
    pub spoilage: Vec<f64>,
    /// Goods in flight at the start of a tick, (K * G) rows of SLOTS, as `ent_pipeline`.
    pub pop_pipeline: Vec<[f64; SLOTS]>,
    /// (E * G) rows of SLOTS: index j due j + 1 ticks ahead, the shipments of every slot j + 1
    /// or later.
    pub ent_pipeline: Vec<[f64; SLOTS]>,
    pub info: Info,
}

impl Steady {
    /// (E, G) input stocks at the start of a tick.
    pub fn stock(&self) -> Mat {
        Mat::from_fn(self.coef.rows, self.coef.cols, |e, k| {
            (1.0 + COVER) * self.coef[(e, k)] * self.output[e]
        })
    }

    /// (P, G) D = sales of every active cell: Q / (1 + delta s).
    pub fn sales(&self) -> Mat {
        Mat::from_fn(self.supply.rows, self.supply.cols, |p, g| {
            self.supply[(p, g)] / (1.0 + self.spoilage[g])
        })
    }

    /// (P, G) merchant stocks: coverage times sales at traded origin markets.
    pub fn inventory(&self) -> Mat {
        let sales = self.sales();
        Mat::from_fn(self.supply.rows, self.supply.cols, |p, g| {
            self.coverage[g] * sales[(p, g)]
        })
    }

    /// (K,) money at the start of a tick: dividends and rebates, wages come at E2.
    pub fn pop_money(&self) -> Vec<f64> {
        self.pop_income
            .iter()
            .zip(&self.pop_wage)
            .map(|(i, w)| i - w)
            .collect()
    }
}

fn bincount(index: &[usize], weights: impl IntoIterator<Item = f64>, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (&i, w) in index.iter().zip(weights) {
        out[i] += w;
    }
    out
}

/// numpy's `maximum`: NaN wins.
fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.max(b)
    }
}

fn np_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else {
        a.min(b)
    }
}

/// Python's builtin `max`: the first argument unless a later one is larger, so a NaN after the
/// first is passed over.
fn py_max(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut it = values.into_iter();
    let first = it.next().expect("py_max of nothing");
    it.fold(first, |m, v| if v > m { v } else { m })
}

fn divide_where(num: f64, den: f64, on: bool, otherwise: f64) -> f64 {
    if on { num / den } else { otherwise }
}

/// Successive differences of the history of Anderson mixing, as columns.
fn diffs(history: &[Vec<f64>]) -> Vec<Vec<f64>> {
    history
        .windows(2)
        .map(|w| w[1].iter().zip(&w[0]).map(|(b, a)| b - a).collect())
        .collect()
}

/// `f - (dx + dg) @ gamma`.
fn mix(f: &[f64], dx: &[Vec<f64>], dg: &[Vec<f64>], gamma: &[f64]) -> Vec<f64> {
    (0..f.len())
        .map(|i| {
            let mut s = 0.0;
            for (j, g) in gamma.iter().enumerate() {
                s += (dx[j][i] + dg[j][i]) * g;
            }
            f[i] - s
        })
        .collect()
}

/// Stage 2 for every traded good at once: ln w of the active origins with
/// w_o sum_d K_od / (1 + t_od) E_d / Phi_d = r_g V_o, from `spend` E (P, G) and `supply` V
/// (P, G). Log domain: the weights of each good are shifted by their largest before each pass
/// over the kernel. The sweep is the Sinkhorn map ln w <- ln(r V) - ln(sum_d K_od / (1 + t_od)
/// E_d / Phi_d), Anderson-accelerated. The map is taken at the shifted weights, so the solution
/// is the one whose largest ln w of each good is 0 (the scaling fixes ln w only up to a level per
/// good). `start` (P, G) is the initial ln w (0 by default).
pub fn sinkhorn(
    m: &Model,
    spend: &Mat,
    supply: &Mat,
    start: Option<&Mat>,
) -> Result<Scaling, CalibrationError> {
    sinkhorn_limited(m, spend, supply, start, MAX_SWEEPS)
}

pub fn sinkhorn_limited(
    m: &Model,
    spend: &Mat,
    supply: &Mat,
    start: Option<&Mat>,
    max_sweeps: usize,
) -> Result<Scaling, CalibrationError> {
    let inp = m.inp;
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let n = n_p * n_g;
    let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
    let good: Vec<usize> = (0..n).map(|i| i / n_p).collect();
    let origin: Vec<bool> = (0..n)
        .map(|i| inp.traded[i / n_p] && inp.active[i % n_p][i / n_p])
        .collect();
    let v = model::good_major(supply);
    let e: Vec<f64> = model::good_major(spend)
        .into_iter()
        .enumerate()
        .map(|(i, s)| if inp.traded[good[i]] { s } else { 0.0 })
        .collect();
    let net: Vec<f64> = cro
        .k
        .iter()
        .zip(&cro.inv_duty)
        .map(|(k, d)| k * d)
        .collect();
    let total = bincount(
        &good,
        (0..n).map(|i| if origin[i] { v[i] } else { 0.0 }),
        n_g,
    );
    let start = start.map(model::good_major);
    let mut lw: Vec<f64> = (0..n)
        .map(|i| match (&start, origin[i]) {
            (Some(s), true) => s[i],
            _ => 0.0,
        })
        .collect();

    let gross = Factors {
        dom: &dom.k,
        acc: &acc.k,
        cro: &cro.k,
    };
    let net_factors = Factors {
        dom: &dom.k,
        acc: &acc.k,
        cro: &net,
    };
    let step = |lw: &[f64]| -> (Vec<f64>, Vec<f64>, f64) {
        let mut top = vec![0.0; n_g];
        for i in (0..n).filter(|&i| origin[i]) {
            top[good[i]] = np_max(top[good[i]], lw[i]);
        }
        let w: Vec<f64> = (0..n)
            .map(|i| {
                if origin[i] {
                    (lw[i] - top[good[i]]).exp()
                } else {
                    0.0
                }
            })
            .collect();
        let phi = m.origin_sums(&w, gross);
        let b: Vec<f64> = (0..n)
            .map(|i| divide_where(e[i], phi[i], phi[i] > 0.0, 0.0))
            .collect();
        let reach = m.destination_sums(&b, net_factors);
        let got: Vec<f64> = w.iter().zip(&reach).map(|(w, r)| w * r).collect();
        let received = bincount(&good, got.iter().copied(), n_g);
        let r: Vec<f64> = (0..n_g)
            .map(|g| divide_where(received[g], total[g], total[g] > 0.0, 1.0))
            .collect();
        let target: Vec<f64> = (0..n).map(|i| (r[good[i]] * v[i]).ln()).collect();
        let new = (0..n)
            .map(|i| {
                // the map at the shifted weights, not shifted back: ln w of a good is free up to
                // a common level, which would otherwise drift with the mixing until a unit in
                // the last place of ln w passes SINKHORN_TOL (532 for coal and mining on the
                // pack at nu 0.3, a residual stuck at 1.208e-13)
                if origin[i] {
                    target[i] - reach[i].ln()
                } else {
                    0.0
                }
            })
            .collect();
        let gap: Vec<f64> = (0..n)
            .map(|i| {
                if origin[i] {
                    got[i].ln() - target[i]
                } else {
                    0.0
                }
            })
            .collect();
        (new, r, max_abs(&gap))
    };

    let (mut xs, mut gs): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (Vec::new(), Vec::new());
    let (mut best, mut since) = (f64::INFINITY, 0);
    let mut err = f64::NAN;
    // the plain sweep from the last finite point, where a mixed step leaves f64
    let mut plain: Option<Vec<f64>> = None;
    for sweep in 1..=max_sweeps {
        let (fx, r, e) = step(&lw);
        err = e;
        if err <= SINKHORN_TOL {
            let r = (0..n_g)
                .map(|g| if inp.traded[g] { r[g] } else { 1.0 })
                .collect();
            // the residual does not see the level, which the mixing may leave off 0
            let mut top = vec![f64::NEG_INFINITY; n_g];
            for i in (0..n).filter(|&i| origin[i]) {
                top[good[i]] = np_max(top[good[i]], lw[i]);
            }
            for i in (0..n).filter(|&i| origin[i]) {
                lw[i] -= top[good[i]];
            }
            return Ok(Scaling {
                log_weight: model::cells(&lw, n_p, n_g),
                r,
                sweeps: sweep,
                residual: err,
            });
        }
        if !err.is_finite() {
            // An extrapolated step overflowed: restart the mixing from the plain sweep it
            // extrapolated; a plain sweep that overflows is an error.
            let Some(p) = plain.take() else {
                return Err(format!("Sinkhorn residual not finite after {sweep} sweeps").into());
            };
            xs.clear();
            gs.clear();
            (best, since) = (f64::INFINITY, 0);
            lw = p;
            continue;
        }
        if err < best {
            (best, since) = (err, 0);
        } else {
            since += 1;
        }
        if since > ANDERSON_STALL {
            // Anderson mixing has stalled: restart it from a plain sweep.
            xs.clear();
            gs.clear();
            (best, since) = (f64::INFINITY, 0);
        }
        let g: Vec<f64> = fx.iter().zip(&lw).map(|(f, x)| f - x).collect();
        if xs.len() > ANDERSON_MEMORY {
            xs.remove(0);
            gs.remove(0);
        }
        xs.push(lw.clone());
        gs.push(g.clone());
        if gs.len() > 1 {
            let (dx, dg) = (diffs(&xs), diffs(&gs));
            let gamma = lstsq(&dg, &g)?;
            let stepped: Vec<f64> = lw.iter().zip(&g).map(|(x, g)| x + g).collect();
            lw = mix(&stepped, &dx, &dg, &gamma);
            plain = Some(stepped);
        } else {
            lw = fx;
            plain = None;
        }
    }
    Err(format!(
        "Sinkhorn did not converge in {max_sweeps} sweeps, residual {}",
        sci(err)
    )
    .into())
}

/// SLOTS + 1 matrices (P, G): units arriving at each destination per unit spent, by arrival
/// slot: 0 home, 1 a domestic leg, 1 + lag a crossing (as `world::delivered_prices` reads
/// `Sourced::arrive`). Every unit of spending arrives as 1 / q_d units whatever its route
/// (ADR 0200), so each slot holds its routes' share of the spending times `src.alpha`.
pub fn arrivals(m: &Model, src: &Sourcing) -> Vec<Mat> {
    let inp = m.inp;
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
    let (wf, phi) = (&src.w, &src.phi);
    let alpha = model::good_major(&src.alpha);
    let zeros = |a: &[f64]| -> Vec<f64> { a.iter().map(|k| 0.0 * k).collect() };
    let (acc_zero, cro_zero) = (zeros(&acc.k), zeros(&cro.k));
    let dom_zero = zeros(&dom.k);

    let mut out = vec![wf.clone()];
    let domestic = m.origin_sums(
        wf,
        Factors {
            dom: &dom.k,
            acc: &acc_zero,
            cro: &cro_zero,
        },
    );
    out.push(domestic.iter().zip(wf).map(|(s, w)| s - w).collect());
    out.extend((1..SLOTS).map(|_| vec![0.0; wf.len()]));
    for lag in 0..SLOTS {
        let k: Vec<f64> = cro
            .k
            .iter()
            .zip(&cro.lag)
            .map(|(&a, &l)| if l == lag { a } else { 0.0 })
            .collect();
        let crossing = m.origin_sums(
            wf,
            Factors {
                dom: &dom_zero,
                acc: &acc.k,
                cro: &k,
            },
        );
        for (i, o) in out[1 + lag].iter_mut().enumerate() {
            *o += crossing[i] - wf[i];
        }
    }
    out.iter()
        .map(|slot| {
            let v: Vec<f64> = slot
                .iter()
                .zip(phi)
                .zip(&alpha)
                .map(|((&a, &f), &x)| divide_where(a, f, f > 0.0, 0.0) * x)
                .collect();
            model::cells(&v, n_p, n_g)
        })
        .collect()
}

/// (E,) output of local enterprises at p = 1, 0 for traded ones: per province the Leontief
/// system y_k = d_k + sum_j a_kj y_j over its local producers, `local_demand` (P, local goods)
/// the demand of households and traded enterprises; `a_e` (E, G) the cost shares.
pub fn local_output(
    inp: &Inputs,
    a_e: &Mat,
    local_demand: &Mat,
) -> Result<Vec<f64>, CalibrationError> {
    let local: Vec<usize> = (0..inp.n_goods()).filter(|&g| !inp.traded[g]).collect();
    let (n_p, n_l) = (inp.n_provinces(), local.len());
    let mut slot = vec![usize::MAX; inp.n_goods()];
    for (j, &g) in local.iter().enumerate() {
        slot[g] = j;
    }
    let mut owner: Vec<Option<usize>> = vec![None; n_p * n_l];
    for (e, (&p, &g)) in inp.ent_province.iter().zip(&inp.ent_good).enumerate() {
        if !inp.traded[g] {
            owner[p * n_l + slot[g]] = Some(e);
        }
    }
    if (0..n_p * n_l).any(|i| owner[i].is_none() && local_demand.data[i] > 0.0) {
        return Err("local demand in a province without its producer".into());
    }
    let mut out = vec![0.0; inp.ent_good.len()];
    let mut a = vec![0.0; n_l * n_l];
    for p in 0..n_p {
        // (I - M) with M[i][j] = a_{local i, by the producer of local j}.
        for i in 0..n_l {
            for j in 0..n_l {
                let used = owner[p * n_l + j].map_or(0.0, |e| a_e[(e, local[i])]);
                a[i * n_l + j] = if i == j { 1.0 } else { 0.0 } - used;
            }
        }
        let mut y = local_demand.row(p).to_vec();
        solve_lu(&mut a, n_l, &mut y).ok_or("Singular matrix")?;
        for j in 0..n_l {
            if let Some(e) = owner[p * n_l + j] {
                out[e] = y[j];
            }
        }
    }
    Ok(out)
}

/// Solves `a x = b` in place by LU with partial pivoting, pivoting as LAPACK's `gesv` does (the
/// first largest pivot; the multipliers divide by the pivot where LAPACK scales by its
/// reciprocal); `None` on a zero pivot.
fn solve_lu(a: &mut [f64], n: usize, b: &mut [f64]) -> Option<()> {
    for k in 0..n {
        let mut piv = k;
        for r in k + 1..n {
            if a[r * n + k].abs() > a[piv * n + k].abs() {
                piv = r;
            }
        }
        if a[piv * n + k] == 0.0 {
            return None;
        }
        if piv != k {
            for c in 0..n {
                a.swap(k * n + c, piv * n + c);
            }
            b.swap(k, piv);
        }
        for r in k + 1..n {
            let l = a[r * n + k] / a[k * n + k];
            a[r * n + k] = l;
            for c in k + 1..n {
                a[r * n + c] -= l * a[k * n + c];
            }
            b[r] -= l * b[k];
        }
    }
    for k in (0..n).rev() {
        let mut s = b[k];
        for c in k + 1..n {
            s -= a[k * n + c] * b[c];
        }
        b[k] = s / a[k * n + k];
    }
    Some(())
}

fn relative(new: &[f64], old: &[f64]) -> f64 {
    let diff: Vec<f64> = new.iter().zip(old).map(|(a, b)| a - b).collect();
    max_abs(&diff) / py_max([max_abs(new), 1e-300])
}

/// What households earn and spend at given output values, rebates, buyer prices and weights.
struct Households {
    /// (P,) household income rebates aside, the weights of the world price index.
    base: Vec<f64>,
    income: Vec<f64>,
    pop_spend: Mat,
    spend_hh: Mat,
}

/// The inputs of the passes, derived once.
struct Setup<'a> {
    inp: &'a Inputs,
    traded_e: Vec<bool>,
    /// (E, G) a_kg of each enterprise's country and sector.
    a_e: Mat,
    a_va: Vec<f64>,
    ent_country: Vec<usize>,
    pop_country: Vec<usize>,
    wage_share: Vec<f64>,
    people_share: Vec<f64>,
    province_share: Vec<f64>,
    s_l: Vec<f64>,
    sigma_k: Vec<f64>,
    v_data: Vec<f64>,
    local: Vec<usize>,
    /// (G,) delta_g s_g of the competitively stored goods (ADR 0091), 0 elsewhere.
    spoilage: Vec<f64>,
}

impl<'a> Setup<'a> {
    fn new(inp: &'a Inputs) -> Result<Setup<'a>, CalibrationError> {
        let va_target = inp
            .ent_va_target
            .as_ref()
            .ok_or("the inputs carry no value-added target")?;
        let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
        let eg = &inp.ent_good;
        let n_e = eg.len();
        let traded = &inp.traded;
        let traded_e: Vec<bool> = eg.iter().map(|&g| traded[g]).collect();
        let ent_country = inp.ent_country();
        let a_e = Mat::from_fn(n_e, n_g, |e, k| inp.io[ent_country[e]][(k, eg[e])]);
        let a_va: Vec<f64> = a_e.row_sums().iter().map(|s| 1.0 - s).collect();
        let c = &inp.province_country;
        let kp = &inp.pop_province;
        let kc: Vec<usize> = kp.iter().map(|&p| c[p]).collect();
        let people = &inp.pop_population;
        let country_people = bincount(&kc, people.iter().copied(), n_c);
        let wage_share = inp
            .pop_effective
            .iter()
            .zip(kp)
            .map(|(e, &p)| e / inp.labour_supply[p])
            .collect();
        let people_share = (0..kp.len())
            .map(|k| {
                let n = country_people[kc[k]];
                divide_where(people[k], n, n > 0.0, 0.0)
            })
            .collect();
        let province_people = bincount(c, inp.population.iter().copied(), n_c);
        let province_share = (0..n_p)
            .map(|p| {
                let n = province_people[c[p]];
                divide_where(inp.population[p], n, n > 0.0, 0.0)
            })
            .collect();
        let sigma_within = inp.within_sigma();
        let v_data: Vec<f64> = (0..n_e)
            .map(|e| {
                if traded_e[e] {
                    va_target[e] / a_va[e]
                } else {
                    0.0
                }
            })
            .collect();
        let spoilage = spoilage(inp)?;
        Ok(Setup {
            inp,
            spoilage,
            ent_country,
            sigma_k: kc.iter().map(|&k| sigma_within[k]).collect(),
            pop_country: kc,
            wage_share,
            people_share,
            province_share,
            s_l: inp.ent_s_l(),
            v_data,
            local: (0..n_g).filter(|&g| !traded[g]).collect(),
            traded_e,
            a_e,
            a_va,
        })
    }

    /// (P, G) output value of the data state, V = VA / a_VA, at traded origins.
    fn supply_data(&self) -> Mat {
        let inp = self.inp;
        let mut out = Mat::zeros(inp.n_provinces(), inp.n_goods());
        for (e, v) in self.v_data.iter().enumerate() {
            out[(inp.ent_province[e], inp.ent_good[e])] += v;
        }
        out
    }

    /// (P, G) sales value of the data state, V / (1 + delta s): what buyers pay each origin, the
    /// rest of its output replacing its stock's spoilage (ADR 0091).
    fn sales_data(&self) -> Mat {
        let mut out = self.supply_data();
        for p in 0..out.rows {
            for g in 0..out.cols {
                out[(p, g)] /= 1.0 + self.spoilage[g];
            }
        }
        out
    }

    /// Local output at the data state's VA, the start of the income fixed point.
    fn local_start(&self) -> Result<Vec<f64>, CalibrationError> {
        let va_target = self
            .inp
            .ent_va_target
            .as_ref()
            .ok_or("the inputs carry no value-added target")?;
        Ok((0..self.v_data.len())
            .map(|e| {
                if self.traded_e[e] {
                    0.0
                } else {
                    va_target[e] / self.a_va[e]
                }
            })
            .collect())
    }

    fn households(&self, v: &[f64], rebate: &[f64], q: &Mat, weights: &Weights) -> Households {
        let inp = self.inp;
        let (n_p, n_c, n_g) = (inp.n_provinces(), inp.n_countries(), inp.n_goods());
        let (ep, kp, kc) = (&inp.ent_province, &inp.pop_province, &self.pop_country);
        let va: Vec<f64> = self.a_va.iter().zip(v).map(|(a, v)| a * v).collect();
        let bill = bincount(ep, self.s_l.iter().zip(&va).map(|(s, va)| s * va), n_p);
        let pool = bincount(
            &self.ent_country,
            self.s_l.iter().zip(&va).map(|(s, va)| (1.0 - s) * va),
            n_c,
        );
        // the markets' spoilage, p delta S = V delta s / (1 + delta s) at stored goods' origins
        // (ADR 0091), which the country's profit pool carries; the index weights (`base`) keep
        // the gross pool, as `Inputs::base_income` and the world's
        let eg = &inp.ent_good;
        let loss = bincount(
            &self.ent_country,
            (0..v.len()).map(|e| {
                if self.traded_e[e] {
                    let sp = self.spoilage[eg[e]];
                    v[e] * sp / (1.0 + sp)
                } else {
                    0.0
                }
            }),
            n_c,
        );
        let owned: Vec<f64> = (0..n_c).map(|c| pool[c] - loss[c] + rebate[c]).collect();
        let income: Vec<f64> = (0..kp.len())
            .map(|k| self.wage_share[k] * bill[kp[k]] + self.people_share[k] * owned[kc[k]])
            .collect();
        let per_person: Vec<f64> = income
            .iter()
            .zip(&inp.pop_population)
            .map(|(&i, &n)| divide_where(i, n, n > 0.0, 0.0))
            .collect();
        let share = demand::shares(
            weights,
            kc,
            &q.select_rows(kp),
            &per_person,
            &self.sigma_k,
            &inp.category,
            inp.sigma_demand,
            &inp.pigl,
        );
        let pop_spend = Mat::from_fn(kp.len(), n_g, |k, g| {
            if income[k] > 0.0 {
                share[(k, g)] * income[k]
            } else {
                0.0
            }
        });
        let mut spend_hh = Mat::zeros(n_p, n_g);
        for (k, &p) in kp.iter().enumerate() {
            for g in 0..n_g {
                spend_hh[(p, g)] += pop_spend[(k, g)];
            }
        }
        let c = &inp.province_country;
        let base = (0..n_p)
            .map(|p| bill[p] + self.province_share[p] * pool[c[p]])
            .collect();
        Households {
            base,
            income,
            pop_spend,
            spend_hh,
        }
    }

    fn values(&self, r: &[f64], y_local: &[f64]) -> Vec<f64> {
        let eg = &self.inp.ent_good;
        (0..eg.len())
            .map(|e| {
                if self.traded_e[e] {
                    r[eg[e]] * self.v_data[e]
                } else {
                    y_local[e]
                }
            })
            .collect()
    }

    /// Income and local output at given r, rebates, buyer prices and weights: output values V.
    fn settle(
        &self,
        r: &[f64],
        rebate: &[f64],
        q: &Mat,
        weights: &Weights,
        y_local: &[f64],
        max_local: usize,
    ) -> Result<Vec<f64>, CalibrationError> {
        let inp = self.inp;
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let mut y_local = y_local.to_vec();
        for _ in 0..max_local {
            let v = self.values(r, &y_local);
            let spend_hh = self.households(&v, rebate, q, weights).spend_hh;
            let mut used = Mat::zeros(n_p, n_g);
            for e in (0..v.len()).filter(|&e| self.traded_e[e]) {
                let p = inp.ent_province[e];
                for k in 0..n_g {
                    used[(p, k)] += self.a_e[(e, k)] * v[e];
                }
            }
            let demand = Mat::from_fn(n_p, self.local.len(), |p, j| {
                let g = self.local[j];
                spend_hh[(p, g)] + used[(p, g)]
            });
            let y_new = local_output(inp, &self.a_e, &demand)?;
            let settled = y_new
                .iter()
                .zip(&y_local)
                .all(|(n, o)| (n - o).abs() <= LOCAL_TOL * n.abs());
            y_local = y_new;
            if settled {
                return Ok(self.values(r, &y_local));
            }
        }
        Err(format!("income and local output did not settle in {max_local} iterations").into())
    }

    /// (P, G) traded input spending of every province's enterprises (all goods).
    fn input_spending(&self, v: &[f64]) -> Mat {
        let inp = self.inp;
        let mut out = Mat::zeros(inp.n_provinces(), inp.n_goods());
        for (e, &p) in inp.ent_province.iter().enumerate() {
            for k in 0..inp.n_goods() {
                out[(p, k)] += self.a_e[(e, k)] * v[e];
            }
        }
        out
    }
}

/// (G,) delta_g s_g of every traded good (ADR 0091; P56 and P02 from the inputs), 0 for local
/// goods.
fn spoilage(inp: &Inputs) -> Result<Vec<f64>, CalibrationError> {
    let coverage = inp
        .coverage
        .as_ref()
        .ok_or("the inputs carry no coverage")?;
    Ok((0..inp.n_goods())
        .map(|g| {
            if inp.traded[g] {
                inp.storage_loss[g] * coverage[g]
            } else {
                0.0
            }
        })
        .collect())
}

/// The fixed point of the passes as one vector, in the units `change` measures it in: ln q of
/// traded goods, rebates over `rebate_unit`, ln r of traded goods, the PIGL scales over
/// `scale_unit`, the PIGL omega and within-good weights. Anderson's least squares weighs every
/// entry alike, so in money units the rebates would decide the mixing alone.
struct State {
    n_p: usize,
    n_c: usize,
    traded: Vec<bool>,
    /// The within-pop dispersion, fixed by the inputs.
    sigma: Vec<f64>,
    /// The largest rebate of the first pass (1 without rebates), fixed for the run.
    rebate_unit: f64,
    /// The largest PIGL scale at the start, fixed for the run.
    scale_unit: f64,
}

/// The largest magnitude in `v`, or 1 where every entry is 0: the unit of a block of the state.
fn block_unit(v: &[f64]) -> f64 {
    let m = max_abs(v);
    if m > 0.0 { m } else { 1.0 }
}

impl State {
    fn pack(&self, q: &Mat, rebate: &[f64], r: &[f64], w: &Weights) -> Vec<f64> {
        let mut z = Vec::new();
        for p in 0..self.n_p {
            z.extend(
                (0..self.traded.len())
                    .filter(|&g| self.traded[g])
                    .map(|g| q[(p, g)].ln()),
            );
        }
        z.extend(rebate.iter().map(|x| x / self.rebate_unit));
        z.extend((0..r.len()).filter(|&g| self.traded[g]).map(|g| r[g].ln()));
        z.extend(w.scale.iter().map(|x| x / self.scale_unit));
        z.extend_from_slice(&w.omega.data);
        z.extend_from_slice(&w.within.data);
        z
    }

    fn unpack(&self, z: &[f64]) -> (Mat, Vec<f64>, Vec<f64>, Weights) {
        let (n_p, n_c, n_g) = (self.n_p, self.n_c, self.traded.len());
        let goods: Vec<usize> = (0..n_g).filter(|&g| self.traded[g]).collect();
        let mut at = 0;
        let mut q = Mat::filled(n_p, n_g, 1.0);
        for p in 0..n_p {
            for &g in &goods {
                q[(p, g)] = z[at].exp();
                at += 1;
            }
        }
        let rebate = z[at..at + n_c]
            .iter()
            .map(|x| x * self.rebate_unit)
            .collect();
        at += n_c;
        let mut r = vec![1.0; n_g];
        for &g in &goods {
            r[g] = z[at].exp();
            at += 1;
        }
        let scale = z[at..at + n_c]
            .iter()
            .map(|x| x * self.scale_unit)
            .collect();
        at += n_c;
        let omega = Mat {
            rows: n_c,
            cols: N_CATEGORIES,
            data: z[at..at + n_c * N_CATEGORIES].to_vec(),
        };
        at += n_c * N_CATEGORIES;
        let within = Mat {
            rows: n_c,
            cols: n_g,
            data: z[at..].to_vec(),
        };
        (
            q,
            rebate,
            r,
            Weights {
                scale,
                omega,
                within,
                sigma: self.sigma.clone(),
            },
        )
    }
}

/// Stage 1 at the data state: traded spending E (P, G) of households and enterprises at r = 1, no
/// rebates, the data state's buyer prices (`Inputs::base_price`) and the inputs' PIGL weights,
/// with income and local output settled; and the sales value V / (1 + delta s) (P, G) of traded
/// origins, their output value less the spoilage it replaces (ADR 0091).
pub fn data_state_spending(inp: &Inputs, limits: Limits) -> Result<(Mat, Mat), CalibrationError> {
    let setup = Setup::new(inp)?;
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    let traded = &inp.traded;
    let q = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            inp.base_price[(p, g)]
        } else {
            1.0
        }
    });
    let (r, rebate) = (vec![1.0; n_g], vec![0.0; n_c]);
    let weights = inp.pigl_weights()?;
    let v = setup.settle(
        &r,
        &rebate,
        &q,
        &weights,
        &setup.local_start()?,
        limits.max_local,
    )?;
    let hh = setup.households(&v, &rebate, &q, &weights);
    let spend_int = setup.input_spending(&v);
    let spend = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            hh.spend_hh[(p, g)] + spend_int[(p, g)]
        } else {
            0.0
        }
    });
    Ok((spend, setup.sales_data()))
}

/// The steady state of ADR 0089 (module documentation) at natural rates `u_star` (C,).
/// `progress` gets one line per pass.
pub fn calibrate(
    inp: &Inputs,
    u_star: &[f64],
    progress: &mut dyn FnMut(&str),
) -> Result<Steady, CalibrationError> {
    calibrate_with(inp, u_star, Limits::default(), progress)
}

/// [`calibrate`] with explicit iteration caps, from the buyer prices of the data state.
pub fn calibrate_with(
    inp: &Inputs,
    u_star: &[f64],
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Steady, CalibrationError> {
    let start = Mat::from_fn(inp.n_provinces(), inp.n_goods(), |p, g| {
        if inp.traded[g] {
            inp.base_price[(p, g)]
        } else {
            1.0
        }
    });
    calibrate_from(inp, u_star, limits, &start, progress)
}

/// [`calibrate_with`] from buyer prices `start` (P, G) in place of the data state's: the passes
/// begin at these q (local goods' are read as given). The fixed point does not depend on the
/// start where the calibration is unique (ADR 0106, `calibration_unique_from_eight_starts`).
pub fn calibrate_from(
    inp: &Inputs,
    u_star: &[f64],
    limits: Limits,
    start: &Mat,
    progress: &mut dyn FnMut(&str),
) -> Result<Steady, CalibrationError> {
    let t0 = Instant::now();
    if inp.base_price.data.iter().any(|p| !p.is_finite()) {
        return Err("a traded good is unreachable at some province".into());
    }
    if (start.rows, start.cols) != (inp.n_provinces(), inp.n_goods())
        || start.data.iter().any(|&p| !(p.is_finite() && p > 0.0))
    {
        return Err("start prices must be positive and finite, one per province and good".into());
    }
    let va_target = inp
        .ent_va_target
        .as_ref()
        .ok_or("the inputs carry no value-added target")?;
    let coverage = inp
        .coverage
        .as_ref()
        .ok_or("the inputs carry no coverage")?;
    // the block map at the inputs' P02 and storage parameters (ADR 0091), the rest at the
    // defaults; the structural digest pins the storage parameters to the calibration's
    let default = TradeParams::default();
    let per_good = |v: &[f64], fallback: &[f64; N_GOODS]| -> [f64; N_GOODS] {
        std::array::from_fn(|g| v.get(g).copied().unwrap_or(fallback[g]))
    };
    let st = &inp.storage;
    let trade = TradeParams {
        coverage: per_good(coverage, &[0.0; N_GOODS]),
        storage_loss: per_good(&inp.storage_loss, &[0.0; N_GOODS]),
        storage_cost: per_good(&st.cost, &default.storage_cost),
        speculation: per_good(&st.speculation, &default.speculation),
        convenience: per_good(&st.convenience, &default.convenience),
        // inputs built without storage parameters keep the defaults
        storage_gain: if st.speculation.is_empty() {
            default.storage_gain
        } else {
            st.gain
        },
        ..default
    };
    trade.check_storage()?;
    let m = Model::physical(inp)?;
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    let (ep, eg) = (&inp.ent_province, &inp.ent_good);
    let n_e = eg.len();
    let traded = &inp.traded;
    let traded_e: Vec<bool> = eg.iter().map(|&g| traded[g]).collect();
    let setup = Setup::new(inp)?;
    let (a_e, a_va) = (setup.a_e.clone(), setup.a_va.clone());
    let c = &inp.province_country;
    let kp = &inp.pop_province;
    let kc = setup.pop_country.clone();
    let people = &inp.pop_population;
    let sigma_within = inp.within_sigma();
    let omega_total = inp.omega.row_sums();
    let om = Mat::from_fn(n_c, n_g, |k, g| inp.omega[(k, g)] / omega_total[k]);
    let traded_va_data: f64 = (0..n_e)
        .filter(|&e| traded_e[e])
        .map(|e| va_target[e])
        .sum();
    let sales_data = setup.sales_data();
    let origin = |p: usize, g: usize| inp.active[p][g] && traded[g];
    let theta: Vec<f64> = (0..n_g)
        .map(|g| if traded[g] { inp.theta[g] } else { 1.0 })
        .collect();

    let mut q = start.clone();
    let mut r = vec![1.0; n_g];
    let mut rebate = vec![0.0; n_c];
    let mut y_local = setup.local_start()?;
    let mut weights = inp.pigl_weights()?;
    let mut state = State {
        n_p,
        n_c,
        traded: traded.clone(),
        sigma: sigma_within.clone(),
        rebate_unit: 0.0,
        scale_unit: block_unit(&weights.scale),
    };
    let mut lw: Option<Mat> = None;
    let mut history: Vec<Pass> = Vec::new();
    let (mut zs, mut fs): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (Vec::new(), Vec::new());
    let ones = Mat::filled(n_p, n_g, 1.0);
    let mut settled = None;
    let mut change = f64::NAN;
    for passes in 1..=limits.max_passes {
        let v = setup.settle(&r, &rebate, &q, &weights, &y_local, limits.max_local)?;
        y_local = (0..n_e)
            .map(|e| if traded_e[e] { 0.0 } else { v[e] })
            .collect();
        let hh = setup.households(&v, &rebate, &q, &weights);
        let spend_int = setup.input_spending(&v);
        let spend = Mat::from_fn(n_p, n_g, |p, g| {
            if traded[g] {
                hh.spend_hh[(p, g)] + spend_int[(p, g)]
            } else {
                0.0
            }
        });

        let mut scaling =
            sinkhorn_limited(&m, &spend, &sales_data, lw.as_ref(), limits.max_sweeps)?;
        // spending scales with r, so the passes fix r only up to one common factor: pin it
        let model_va: f64 = (0..n_e)
            .filter(|&e| traded_e[e])
            .map(|e| scaling.r[eg[e]] * va_target[e])
            .sum();
        let r_level = traded_va_data / model_va;
        for g in (0..n_g).filter(|&g| traded[g]) {
            scaling.r[g] *= r_level;
        }
        let log_weight = scaling.log_weight.clone();
        let mut top = vec![f64::NEG_INFINITY; n_g];
        for p in 0..n_p {
            for (g, t) in top.iter_mut().enumerate() {
                let x = if origin(p, g) {
                    log_weight[(p, g)]
                } else {
                    f64::NEG_INFINITY
                };
                *t = np_max(*t, x);
            }
        }
        let raw = Mat::from_fn(n_p, n_g, |p, g| {
            if !traded[g] {
                1.0
            } else if origin(p, g) {
                (-(log_weight[(p, g)] - top[g]) / theta[g]).exp()
            } else {
                0.0
            }
        });
        let index_weights = Mat::from_fn(n_p, n_g, |p, g| om[(c[p], g)] * hh.base[p]);
        let raw_index = m.sourcing(&raw).index;
        let (mut mass, mut level) = (vec![0.0; n_g], vec![0.0; n_g]);
        for p in 0..n_p {
            for g in 0..n_g {
                let index = if traded[g] { raw_index[(p, g)] } else { 1.0 };
                mass[g] += index_weights[(p, g)];
                level[g] += index_weights[(p, g)] * index;
            }
        }
        // A good without final demand has no weight in the index: its price level stays as
        // solved.
        let level: Vec<f64> = (0..n_g)
            .map(|g| divide_where(level[g], mass[g], mass[g] > 0.0, 1.0))
            .collect();
        let price = Mat::from_fn(n_p, n_g, |p, g| {
            raw[(p, g)] / if traded[g] { level[g] } else { 1.0 }
        });
        let src = m.sourcing(&price);
        let q_new = Mat::from_fn(
            n_p,
            n_g,
            |p, g| {
                if traded[g] { src.index[(p, g)] } else { 1.0 }
            },
        );
        let duty = m.flows(&price, &ones, &src).tariff;
        let duty_bill = Mat::from_fn(n_p, n_g, |p, g| duty[(p, g)] * spend[(p, g)]).row_sums();
        let rebate_new = bincount(c, duty_bill, n_c);
        let per_person: Vec<f64> = hh
            .income
            .iter()
            .zip(people)
            .map(|(&i, &n)| divide_where(i, n, n > 0.0, 0.0))
            .collect();
        let weights_new = demand::invert_pops(
            &inp.omega,
            &demand::Pops {
                country: &kc,
                price: &q.select_rows(kp),
                income: &per_person,
                spend: &hh.income,
            },
            &sigma_within,
            &inp.category,
            inp.sigma_demand,
            &inp.pigl,
        )?;

        change = py_max([
            relative(&rebate_new, &rebate),
            relative(&scaling.r, &r),
            relative(&q_new.data, &q.data),
            relative(&weights_new.scale, &weights.scale),
            relative(&weights_new.omega.data, &weights.omega.data),
        ]);
        history.push(Pass {
            pass: passes,
            change,
            sweeps: scaling.sweeps,
            sinkhorn_residual: scaling.residual,
        });
        progress(&format!(
            "pass {passes}: change {}, sinkhorn {} sweeps, residual {}",
            sci(change),
            scaling.sweeps,
            sci(scaling.residual)
        ));
        lw = Some(log_weight);
        if change <= PASS_TOL {
            (rebate, r, q, weights) = (rebate_new, scaling.r, q_new, weights_new);
            settled = Some((passes, price, src, top));
            break;
        }
        // Anderson mixing of the passes' fixed point (prices, rebates, r and the PIGL weights).
        if state.rebate_unit == 0.0 {
            state.rebate_unit = block_unit(&rebate_new);
        }
        let z = state.pack(&q, &rebate, &r, &weights);
        let f = state.pack(&q_new, &rebate_new, &scaling.r, &weights_new);
        let fz: Vec<f64> = f.iter().zip(&z).map(|(f, z)| f - z).collect();
        if zs.len() > ANDERSON_MEMORY {
            zs.remove(0);
            fs.remove(0);
        }
        zs.push(z);
        fs.push(fz.clone());
        if fs.len() > 1 {
            let (dz, df) = (diffs(&zs), diffs(&fs));
            let gamma = lstsq(&df, &fz)?;
            (q, rebate, r, weights) = state.unpack(&mix(&f, &dz, &df, &gamma));
        } else {
            (rebate, r, q, weights) = (rebate_new, scaling.r, q_new, weights_new);
        }
    }
    let Some((passes, price, src, top)) = settled else {
        return Err(CalibrationError::NotSettled {
            passes: limits.max_passes,
            change,
        });
    };
    let lw = lw.expect("a pass ran");

    let v = setup.settle(&r, &rebate, &q, &weights, &y_local, limits.max_local)?;
    let va0: Vec<f64> = a_va.iter().zip(&v).map(|(a, v)| a * v).collect();
    let hh = setup.households(&v, &rebate, &q, &weights);
    let mut spend_int = setup.input_spending(&v);
    for p in 0..n_p {
        for g in (0..n_g).filter(|&g| !traded[g]) {
            spend_int[(p, g)] = 0.0;
        }
    }

    let own: Vec<f64> = (0..n_e)
        .map(|e| {
            if traded_e[e] {
                price[(ep[e], eg[e])]
            } else {
                1.0
            }
        })
        .collect();
    let output: Vec<f64> = v.iter().zip(&own).map(|(v, o)| v / o).collect();
    let s_l = &setup.s_l;
    let bill = bincount(ep, s_l.iter().zip(&va0).map(|(s, va)| s * va), n_p);
    let w0: Vec<f64> = (0..n_p)
        .map(|p| bill[p] / ((1.0 - u_star[c[p]]) * inp.labour_supply[p]))
        .collect();
    let mask = |g: usize| if traded[g] { 1.0 } else { 0.0 };
    let arrive: Vec<Mat> = arrivals(&m, &src)
        .into_iter()
        .map(|a| Mat::from_fn(n_p, n_g, |p, g| a[(p, g)] * mask(g)))
        .collect();
    let mut alpha = arrive[0].clone();
    let mut timed = Mat::from_fn(n_p, n_g, |p, g| 0.0 * arrive[0][(p, g)]);
    for (slot, a) in arrive.iter().enumerate().skip(1) {
        for i in 0..alpha.data.len() {
            alpha.data[i] += a.data[i];
            timed.data[i] += slot as f64 * a.data[i];
        }
    }
    let lead = Mat::from_fn(n_p, n_g, |p, g| {
        divide_where(timed[(p, g)], alpha[(p, g)], alpha[(p, g)] > 0.0, 0.0)
    });
    let mut supply = Mat::zeros(n_p, n_g);
    for e in 0..n_e {
        supply[(ep[e], eg[e])] += output[e];
    }
    let orders_of = |spend: &Mat| {
        let o = m.flows(&price, spend, &src).orders;
        Mat::from_fn(n_p, n_g, |p, g| o[(p, g)] * mask(g))
    };
    let orders = [orders_of(&spend_int), orders_of(&hh.spend_hh)];
    let spend_traded = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            spend_int[(p, g)] + hh.spend_hh[(p, g)]
        } else {
            0.0
        }
    });
    let demand_elasticity = elasticity::demand(&m, &src, &spend_traded);
    let supply_elasticity = elasticity::supply(inp, s_l, &a_va);
    let ent_spend = Mat::from_fn(
        n_e,
        n_g,
        |e, g| {
            if traded[g] { a_e[(e, g)] * v[e] } else { 0.0 }
        },
    );
    let min_log10_relative_weight = (0..n_g)
        .map(|g| {
            traded[g].then(|| {
                let low = (0..n_p).fold(f64::INFINITY, |lo, p| {
                    let x = if origin(p, g) {
                        lw[(p, g)] - top[g]
                    } else {
                        f64::INFINITY
                    };
                    np_min(lo, x)
                });
                low / LN_10
            })
        })
        .collect();
    let info = Info {
        r: r.clone(),
        passes,
        pass_history: history,
        min_log10_relative_weight,
        seconds: t0.elapsed().as_secs_f64(),
    };
    let coef = Mat::from_fn(n_e, n_g, |e, k| a_e[(e, k)] * own[e] / q[(ep[e], k)]);
    let l0 = (0..n_e).map(|e| s_l[e] * va0[e] / w0[ep[e]]).collect();
    let pop_wage: Vec<f64> = (0..kp.len())
        .map(|k| setup.wage_share[k] * bill[kp[k]])
        .collect();
    let pop_pipeline = in_flight(&arrive, &hh.pop_spend, kp);
    let ent_pipeline = in_flight(&arrive, &ent_spend, ep);
    let mut steady = Steady {
        u_star: u_star.to_vec(),
        r,
        price: Mat::from_fn(n_p, n_g, |p, g| {
            if origin(p, g) || !traded[g] {
                price[(p, g)]
            } else {
                0.0
            }
        }),
        gamma: Mat::from_fn(n_p, n_g, |p, g| {
            let (q, ces) = (src.index[(p, g)], src.ces[(p, g)]);
            if traded[g] && q.is_finite() && ces.is_finite() {
                q / ces
            } else {
                1.0
            }
        }),
        consumer: q,
        lead,
        demand_elasticity,
        supply_elasticity,
        block_limit: [
            Mat::filled(n_p, n_g, f64::INFINITY),
            Mat::filled(n_p, n_g, f64::INFINITY),
        ],
        arrive,
        weights,
        rebate,
        value: v,
        output,
        coef,
        a_va,
        a: own.iter().map(|o| 1.0 / o).collect(),
        va0,
        l0,
        w0,
        ent_spend,
        pop_income: hh.income,
        pop_wage,
        pop_spend: hh.pop_spend,
        spend_hh: hh.spend_hh,
        spend_int,
        supply,
        orders,
        coverage: (0..n_g)
            .map(|g| if traded[g] { coverage[g] } else { 0.0 })
            .collect(),
        spoilage: setup.spoilage.clone(),
        pop_pipeline,
        ent_pipeline,
        info,
    };
    let [default, frictionless] = [EconParams::default(), EconParams::frictionless()]
        .map(|params| block::limits(&m, &src, &steady, s_l, &params, &trade));
    steady.block_limit = [default?, frictionless?];
    Ok(steady)
}

/// (buyers * G) rows of SLOTS: goods in flight at the start of a tick for buyers spending
/// `spend` (buyers, G) per month at `province`: j ticks ahead, one month of the shipments of
/// every arrival slot j or later.
pub fn in_flight(arrive: &[Mat], spend: &Mat, province: &[usize]) -> Vec<[f64; SLOTS]> {
    let mut out = Vec::with_capacity(spend.rows * spend.cols);
    for (b, &p) in province.iter().enumerate() {
        for g in 0..spend.cols {
            let mut row = [0.0; SLOTS];
            let mut acc = 0.0;
            for j in (0..SLOTS).rev() {
                let n = arrive[1 + j][(p, g)] * spend[(b, g)];
                acc = if j == SLOTS - 1 { n } else { acc + n };
                row[j] = acc;
            }
            out.push(row);
        }
    }
    out
}

/// The reference model's inputs at the calibrated point: A, VA0, L0, w0, the coefficients,
/// a_VA, the PIGL weights and the natural rates of `s` (labour clears at (1 - u*) L^s).
pub fn reference_inputs(inp: &Inputs, s: &Steady) -> Inputs {
    let mut out = inp.clone();
    out.ent_a.clone_from(&s.a);
    out.ent_va0.clone_from(&s.va0);
    out.ent_l0.clone_from(&s.l0);
    out.w0.clone_from(&s.w0);
    out.u_star.clone_from(&s.u_star);
    out.calibrated_coef = Some(s.coef.clone());
    out.calibrated_a_va = Some(s.a_va.clone());
    out.calibrated_weights = Some(s.weights.clone());
    out.gamma = Some(s.gamma.clone());
    out
}

/// x of the reference model: ln p of the active cells, ln w0.
pub fn reference_point(inp: &Inputs, s: &Steady) -> Vec<f64> {
    let mut x: Vec<f64> = inp
        .active_cells()
        .iter()
        .map(|&i| s.price.data[i].ln())
        .collect();
    x.extend(s.w0.iter().map(|w| w.ln()));
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sci_matches_python_format() {
        for (x, want) in [
            (1.0, "1.000e+00"),
            (0.38041, "3.804e-01"),
            (9.5917e-14, "9.592e-14"),
            (1234.5, "1.234e+03"),
            (1e100, "1.000e+100"),
            (0.0, "0.000e+00"),
            (f64::NAN, "nan"),
            (f64::INFINITY, "inf"),
        ] {
            assert_eq!(sci(x), want, "{x}");
        }
    }

    /// The mixing state holds rebates and PIGL scales relative to their block's unit, the other
    /// blocks as they are, and unpacks back to the same values.
    #[test]
    fn state_packs_rebates_and_scales_relative_to_their_unit() {
        let state = State {
            n_p: 2,
            n_c: 2,
            traded: vec![true, false],
            sigma: vec![0.1, 0.2],
            rebate_unit: 4e9,
            scale_unit: 2.5,
        };
        let q = Mat::from_fn(2, 2, |p, g| if g == 0 { 1.5 + p as f64 } else { 1.0 });
        let (rebate, r) = (vec![1e9, 4e9], vec![1.1, 1.0]);
        let w = Weights {
            scale: vec![2.5, 0.5],
            omega: Mat::from_fn(2, N_CATEGORIES, |c, j| 0.1 * (c + j) as f64),
            within: Mat::from_fn(2, 2, |c, g| 0.3 + 0.1 * (c + g) as f64),
            sigma: state.sigma.clone(),
        };
        let z = state.pack(&q, &rebate, &r, &w);
        assert_eq!(&z[..2], &[1.5f64.ln(), 2.5f64.ln()]);
        assert_eq!(&z[2..4], &[0.25, 1.0]);
        assert_eq!(z[4], 1.1f64.ln());
        assert_eq!(&z[5..7], &[1.0, 0.2]);
        let (q2, rebate2, r2, w2) = state.unpack(&z);
        let close = |a: &[f64], b: &[f64]| {
            a.iter()
                .zip(b)
                .all(|(x, y)| (x - y).abs() <= 1e-15 * y.abs())
        };
        assert!(close(&q2.data, &q.data) && close(&rebate2, &rebate) && close(&r2, &r));
        assert!(close(&w2.scale, &w.scale));
        assert_eq!(
            (w2.omega.data, w2.within.data),
            (w.omega.data, w.within.data)
        );
        assert_eq!(block_unit(&[0.0, 0.0]), 1.0);
        assert_eq!(block_unit(&[-3.0, 2.0]), 3.0);
    }

    #[test]
    fn solve_lu_pivots() {
        let mut a = vec![0.0, 2.0, 1.0, 1.0, 1.0, 0.0, 3.0, 0.0, 1.0];
        let mut b = vec![3.0, 2.0, 4.0];
        solve_lu(&mut a, 3, &mut b).unwrap();
        for (got, want) in b.iter().zip([1.0, 1.0, 1.0]) {
            assert!((got - want).abs() < 1e-15, "{b:?}");
        }
        let mut singular = vec![1.0, 2.0, 2.0, 4.0];
        assert!(solve_lu(&mut singular, 2, &mut [1.0, 1.0]).is_none());
    }
}
