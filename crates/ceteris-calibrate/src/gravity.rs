//! Direct estimation of trade costs by PPML structural gravity (ADR 0022).
//!
//! The estimator ([`ceteris_reference::ppml::hdfe`]) first reproduces the published PPML
//! estimates of Yotov, Piermartini, Monteiro and Larch (2016) on their sample ([`yotov`]). On 2000 BACI it then
//! estimates, with least-cost (S05) costs between countries from the pack's transport graph
//! (route choice is deferred past P2, ADR 0017):
//!
//! - theta per class (P01, ADR 0023): minus the coefficient on ln(1 + tariff), international
//!   flows;
//! - the border friction nu (P05): a border dummy on a sample with domestic flows (2000 ICIO
//!   output less exports), as an ad valorem equivalent at the class thetas of the same fit;
//! - mode cost ratios (P04a-d): coefficients on tonne-km by mode along the S05 paths, relative to
//!   road.
//!
//! Outputs carry provenance "data". `ceteris gravity --write` puts them in `gravity_2000.toml`
//! only when no stop rule fires ([`stop_rules`]); the econ pack build does not read that file,
//! since whether the model switches to the estimates is the owner's decision.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ceteris_pack::econ::goods::hs_goods;
use ceteris_pack::read::baci::{country_codes, for_each_flow, read_hs_cpa};
use ceteris_pack::read::icio::{aggregate, exports};
use ceteris_pack::read::natural_earth::{ADMIN0, IsoCodes, read_codes};
use ceteris_pack::read::people::parts;
use ceteris_reference::io::pairwise_sum;
use ceteris_reference::ppml::Fit;

pub use crate::costs::*;
use crate::dta::select;
use crate::json::Json;
use crate::pyfmt::{fixed, int_list, num, py_g};
use crate::write::Table;
use crate::yotov::{Columns, hdfe};
pub use crate::yotov::{GRAVITY_TERMS, Published, YOTOV, YOTOV_PUBLISHED, YOTOV_YEARS, yotov};

pub const BACI: &str = "BACI_HS92_V202601.zip";
const ICIO: &str = "1995-2000_SML.zip";
const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
const YEAR: i64 = 2000;

/// P43 time cost of transit lambda_g per day, the starts of `TradeParams::default()` (trade
/// SPEC), traded goods.
pub const TIME_COST: [f64; N_TRADED] = [0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010];
/// P01 classes of `TradeParams::default()` (ADRs 0023 and 0101) and their current values.
pub const CLASSES: [&[usize]; 4] = [&[3, 5], &[6], &[0, 4], &[1, 2]];
pub const THETA_DEFAULT: [f64; 4] = [2.91, 4.14, 7.68, 16.91];
/// P05 start.
pub const BORDER_DEFAULT: f64 = 0.3;
/// `ceteris_trade::goods::tariff_group`: primary, manufactured.
pub const TARIFF_GROUP: [usize; N_TRADED] = [0, 0, 0, 0, 1, 1, 1];

// ---------------------------------------------------------------- flows

/// The pack country index of every source economy code: each country's parts (or ISO code)
/// and its own code, the first country to claim a code keeping it; BACI's Taiwan and Serbia and
/// Montenegro codes follow their countries.
fn pack_index(codes: &[String], iso: &IsoCodes) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for (i, c) in codes.iter().enumerate() {
        for s in parts(c, iso).into_iter().chain([c.clone()]) {
            index.entry(s).or_insert(i);
        }
    }
    for (s, c) in [("S19", "TWN"), ("SCG", "YUG")] {
        if let Some(&i) = index.get(c) {
            index.entry(s.to_owned()).or_insert(i);
        }
    }
    index
}

/// 2000 BACI trade by (exporter, importer, traded good) of the pack countries, USD, as
/// `flows[(o * C + d) * N_TRADED + g]`; HS lines assigned to goods by the OECD HS to CPA key,
/// which puts each line wholly in one good. Key lines that are not numbers (the EU aggregates)
/// match no BACI line.
pub fn baci_flows(cache: &Path, codes: &[String], iso: &IsoCodes) -> Result<Vec<f64>, String> {
    let hs_cpa = read_hs_cpa(&cache.join(HS_KEY))?;
    let good_of: BTreeMap<i64, usize> = hs_goods(&hs_cpa)
        .into_iter()
        .filter(|(k, _)| !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()))
        .map(|(k, g)| (k.parse().expect("digits"), g))
        .collect();
    let index = pack_index(codes, iso);
    let baci = cache.join(BACI);
    let lookup: BTreeMap<i64, usize> = country_codes(&baci)?
        .into_iter()
        .filter_map(|(k, v)| Some((k, *index.get(&v)?)))
        .collect();
    let nc = codes.len();
    let mut flows = vec![0.0; nc * nc * N_TRADED];
    // Sums run per batch and are then added, as a bincount per batch would.
    let mut batch = vec![0.0; nc * nc * N_TRADED];
    let (mut touched, mut seen) = (Vec::new(), vec![false; nc * nc]);
    for_each_flow(&baci, |row| {
        let Some(f) = row else {
            for &cell in &touched {
                for g in 0..N_TRADED {
                    flows[cell * N_TRADED + g] += batch[cell * N_TRADED + g];
                    batch[cell * N_TRADED + g] = 0.0;
                }
                seen[cell] = false;
            }
            touched.clear();
            return;
        };
        let (Some(&i), Some(&j)) = (lookup.get(&f.i), lookup.get(&f.j)) else {
            return;
        };
        if i == j {
            return;
        }
        let v = f.v * 1000.0;
        let good = good_of.get(&f.k).copied().filter(|&g| g < N_TRADED);
        let cell = i * nc + j;
        if !seen[cell] {
            seen[cell] = true;
            touched.push(cell);
        }
        for g in 0..N_TRADED {
            let share = if good == Some(g) { 1.0 } else { 0.0 };
            batch[cell * N_TRADED + g] += v * share;
        }
    })?;
    Ok(flows)
}

/// Domestic flows by (pack country, traded good), USD: 2000 ICIO output less exports, for the
/// countries that are ICIO economies (`va_basis` icio); NaN elsewhere and where exports exceed
/// output.
pub fn icio_domestic(
    cache: &Path,
    codes: &[String],
    iso: &IsoCodes,
    va_basis: &[String],
) -> Result<Vec<[f64; N_TRADED]>, String> {
    let icio = cache.join(ICIO);
    let output = aggregate(&icio, YEAR)?;
    let sold = exports(&icio, YEAR)?;
    codes
        .iter()
        .zip(va_basis)
        .map(|(c, basis)| {
            let code = iso.get(c).unwrap_or(c);
            let mut out = [f64::NAN; N_TRADED];
            if basis == "icio"
                && code != "ROW"
                && let Some(o) = output.get(code)
            {
                let s = sold
                    .get(code)
                    .ok_or_else(|| format!("no ICIO exports of {code}"))?;
                for g in 0..N_TRADED {
                    let d = (o.output[g] - s[g]) * 1e6;
                    out[g] = if d >= 0.0 { d } else { f64::NAN };
                }
            }
            Ok(out)
        })
        .collect()
}

// ---------------------------------------------------------------- estimators

/// One observation per (exporter, importer, traded good); `q` the S05 path quantities and `tau`
/// their iceberg factor at the good's weight and time cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub exporter: Vec<usize>,
    pub importer: Vec<usize>,
    pub good: Vec<usize>,
    /// USD.
    pub flow: Vec<f64>,
    /// Ad valorem.
    pub tariff: Vec<f64>,
    pub tau: Vec<f64>,
    pub q: Vec<Quantities>,
    /// Tonnes per USD of the good.
    pub weight: Vec<f64>,
    /// P43 lambda of the good.
    pub time_cost: Vec<f64>,
}

impl Sample {
    pub fn rows(&self, mask: &[bool]) -> Sample {
        Sample {
            exporter: select(&self.exporter, mask),
            importer: select(&self.importer, mask),
            good: select(&self.good, mask),
            flow: select(&self.flow, mask),
            tariff: select(&self.tariff, mask),
            tau: select(&self.tau, mask),
            q: select(&self.q, mask),
            weight: select(&self.weight, mask),
            time_cost: select(&self.time_cost, mask),
        }
    }

    pub fn len(&self) -> usize {
        self.flow.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flow.is_empty()
    }

    /// Exporter-good and importer-good effects (outward and inward multilateral resistance).
    pub fn fixed_effects(&self) -> [Vec<i64>; 2] {
        let code = |c: &[usize]| -> Vec<i64> {
            c.iter()
                .zip(&self.good)
                .map(|(&c, &g)| (c * N_TRADED + g) as i64)
                .collect()
        };
        [code(&self.exporter), code(&self.importer)]
    }

    /// Unordered country pair: the cluster of Yotov et al. (2016).
    pub fn pairs(&self) -> Vec<i64> {
        let lo_hi: Vec<(usize, usize)> = self
            .exporter
            .iter()
            .zip(&self.importer)
            .map(|(&e, &i)| (e.min(i), e.max(i)))
            .collect();
        let top = lo_hi.iter().map(|x| x.1).max().unwrap_or(0) + 1;
        lo_hi.iter().map(|(l, h)| (l * top + h) as i64).collect()
    }

    fn fit(&self, regressors: &Columns) -> Result<Fit, String> {
        let [o, d] = self.fixed_effects();
        hdfe(&self.flow, &[&o, &d], regressors, Some(&self.pairs()))
    }

    fn ln_tariff(&self) -> Vec<f64> {
        self.tariff.iter().map(|t| t.ln_1p()).collect()
    }
}

fn masked(v: &[f64], m: &[bool]) -> Vec<f64> {
    v.iter()
        .zip(m)
        .map(|(x, &m)| x * f64::from(u8::from(m)))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theta {
    pub goods: Vec<usize>,
    /// Minus the coefficient on ln(1 + tariff).
    pub theta: f64,
    pub se: f64,
    /// Coefficient on ln tau (S05).
    pub tau: f64,
    pub se_tau: f64,
    pub n: usize,
    pub pseudo_r2: f64,
}

/// P01 per class: PPML of international flows of the class's goods on ln(1 + tariff) and ln tau
/// (S05) with exporter-good and importer-good fixed effects, clustered by country pair.
pub fn theta_by_class(s: &Sample, classes: &[&[usize]]) -> Result<Vec<Theta>, String> {
    classes
        .iter()
        .map(|goods| {
            let mask: Vec<bool> = (0..s.len())
                .map(|i| goods.contains(&s.good[i]) && s.exporter[i] != s.importer[i])
                .collect();
            let sub = s.rows(&mask);
            let fit = sub.fit(&vec![
                ("ln_tariff".into(), sub.ln_tariff()),
                ("ln_tau".into(), sub.tau.iter().map(|t| t.ln()).collect()),
            ])?;
            Ok(Theta {
                goods: goods.to_vec(),
                theta: -fit.coef("ln_tariff"),
                se: fit.se("ln_tariff"),
                tau: fit.coef("ln_tau"),
                se_tau: fit.se("ln_tau"),
                n: fit.n,
                pseudo_r2: fit.pseudo_r2,
            })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Border {
    /// P05, ad valorem: exp(sum_c share_c ln(1 + nu_c)) - 1.
    pub nu: f64,
    pub se: f64,
    pub nu_by_class: Vec<f64>,
    pub se_by_class: Vec<f64>,
    /// From the same fit.
    pub theta_by_class: Vec<f64>,
    /// Coefficients on the border dummy.
    pub border_by_class: Vec<f64>,
    pub n: usize,
    pub pseudo_r2: f64,
}

/// g' cov g.
fn quadratic(g: &[f64], fit: &Fit) -> f64 {
    let k = g.len();
    let gc: Vec<f64> = (0..k)
        .map(|j| {
            let mut s = 0.0;
            for (i, gi) in g.iter().enumerate() {
                s += gi * fit.cov[(i, j)];
            }
            s
        })
        .collect();
    let mut s = 0.0;
    for (a, b) in gc.iter().zip(g) {
        s += a * b;
    }
    s
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

fn position(fit: &Fit, name: &str) -> usize {
    fit.names
        .iter()
        .position(|n| n == name)
        .expect("a regressor of the fit")
}

/// Membership of each observation in each class.
fn members(s: &Sample, classes: &[&[usize]]) -> Vec<Vec<bool>> {
    classes
        .iter()
        .map(|c| s.good.iter().map(|g| c.contains(g)).collect())
        .collect()
}

/// P05: PPML on a sample with domestic flows, per class a border dummy, ln(1 + tariff) and ln
/// tau, with exporter-good and importer-good fixed effects, clustered by country pair. The
/// border coefficient is -theta_c ln(1 + nu) (trade SPEC, two-level kernel), so ln(1 + nu_c) =
/// b_c / beta_c with beta_c the tariff coefficient; nu pools the classes with weights `shares`
/// (default equal). Delta-method errors.
pub fn border_friction(
    s: &Sample,
    classes: &[&[usize]],
    shares: Option<&[f64]>,
) -> Result<Border, String> {
    let member = members(s, classes);
    let border: Vec<f64> = s
        .exporter
        .iter()
        .zip(&s.importer)
        .map(|(e, i)| f64::from(u8::from(e != i)))
        .collect();
    let (ln_tariff, ln_tau) = (
        s.ln_tariff(),
        s.tau.iter().map(|t| t.ln()).collect::<Vec<_>>(),
    );
    let mut reg: Columns = Vec::new();
    for (c, m) in member.iter().enumerate() {
        reg.push((format!("ln_tariff_{c}"), masked(&ln_tariff, m)));
        reg.push((format!("ln_tau_{c}"), masked(&ln_tau, m)));
        reg.push((format!("border_{c}"), masked(&border, m)));
    }
    let fit = s.fit(&reg)?;
    let k = classes.len();
    let w: Vec<f64> = match shares {
        None => vec![1.0 / k as f64; k],
        Some(sh) => {
            let total = sh.iter().fold(0.0, |a, b| a + b);
            sh.iter().map(|x| x / total).collect()
        }
    };
    let it: Vec<usize> = (0..k)
        .map(|c| position(&fit, &format!("ln_tariff_{c}")))
        .collect();
    let ib: Vec<usize> = (0..k)
        .map(|c| position(&fit, &format!("border_{c}")))
        .collect();
    let beta: Vec<f64> = it.iter().map(|&i| fit.coef[i]).collect();
    let b: Vec<f64> = ib.iter().map(|&i| fit.coef[i]).collect();
    let ln_nu: Vec<f64> = b.iter().zip(&beta).map(|(b, beta)| b / beta).collect();
    let (mut nu_c, mut se_c) = (Vec::new(), Vec::new());
    let mut grad = vec![0.0; fit.names.len()];
    for c in 0..k {
        let mut g = vec![0.0; fit.names.len()];
        g[ib[c]] = 1.0 / beta[c];
        g[it[c]] = -b[c] / (beta[c] * beta[c]);
        nu_c.push(ln_nu[c].exp_m1());
        se_c.push(ln_nu[c].exp() * quadratic(&g, &fit).sqrt());
        for (a, x) in grad.iter_mut().zip(&g) {
            *a += w[c] * x;
        }
    }
    let pooled = dot(&w, &ln_nu);
    Ok(Border {
        nu: pooled.exp_m1(),
        se: pooled.exp() * quadratic(&grad, &fit).sqrt(),
        nu_by_class: nu_c,
        se_by_class: se_c,
        theta_by_class: beta.iter().map(|x| -x).collect(),
        border_by_class: b,
        n: fit.n,
        pseudo_r2: fit.pseudo_r2,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modes {
    /// P04b-d over P04a: rail, sea and air cost per tonne-km relative to road; NaN for a mode
    /// that is not identified.
    pub ratio: [f64; 3],
    pub se: [f64; 3],
    /// Country pairs with a positive flow on a path that uses the mode ([`mode_support`]).
    pub support: [usize; 3],
    /// False for a mode with fewer than [`MODE_MIN_PAIRS`] pairs: it stays at its start and is
    /// not fitted.
    pub identified: [bool; 3],
    pub theta_by_class: Vec<f64>,
    /// Coefficient on ln tau(ratio).
    pub tau_by_class: Vec<f64>,
    pub n: usize,
    pub pseudo_r2: f64,
    pub iterations: usize,
    /// The undamped Gauss-Newton step of each iteration.
    pub steps: Vec<[f64; 3]>,
    /// The halvings of each step before it was taken (none for the converged last step).
    pub halvings: Vec<usize>,
    /// Profile deviance (the fit without the step regressors) at the ratios each step starts
    /// from.
    pub deviance: Vec<f64>,
}

pub const MODE_TOL: f64 = 1e-9;
pub const MODE_MAX_ITER: usize = 50;
/// Step halvings before the fit gives up: 2^-30 of a step is about `MODE_TOL` of it.
pub const MODE_MAX_HALVINGS: usize = 30;
/// Fewest country pairs, the clusters of the covariance, with a positive flow on a mode for its
/// ratio to be fitted (ADR 0022, Amendment 2026-10-29).
pub const MODE_MIN_PAIRS: usize = 50;

/// International rows of `s`.
fn international(s: &Sample) -> Sample {
    let mask: Vec<bool> = (0..s.len())
        .map(|i| s.exporter[i] != s.importer[i])
        .collect();
    s.rows(&mask)
}

/// Rail, sea and air: the unordered international country pairs with a positive flow on a path
/// with km on the mode.
pub fn mode_support(s: &Sample) -> [usize; 3] {
    let pairs = s.pairs();
    std::array::from_fn(|m| {
        let used: BTreeSet<i64> = (0..s.len())
            .filter(|&i| s.exporter[i] != s.importer[i] && s.flow[i] > 0.0 && s.q[i][m + 1] > 0.0)
            .map(|i| pairs[i])
            .collect();
        used.len()
    })
}

/// tau(r) of every row of `s` at rail, sea and air ratios `r` to road, road at its P04a value.
pub fn mode_tau(s: &Sample, r: &[f64; 3], p: &Transport) -> Vec<f64> {
    let road = p.per_tkm[0];
    (0..s.len())
        .map(|i| {
            let q = &s.q[i];
            let km = q[1] * r[0] + q[2] * r[1] + q[3] * r[2];
            1.0 + s.weight[i] * (road * (q[0] + km) + q[Q_FEE]) + s.time_cost[i] * q[Q_HOURS] / 24.0
        })
        .collect()
}

/// P04 ratios: nonlinear PPML of international flows on, per class, ln(1 + tariff) and ln
/// tau(r), with tau(r) = 1 + w (f_road (km_road + sum_m r_m km_m) + fees) + lambda hours / 24 the
/// S05 factor along the least-cost paths and road at its P04a value as numeraire. Gauss-Newton:
/// each step is a PPML on the same regressors plus kappa_c w f_road km_m / tau, kappa_c the ln
/// tau coefficients of the profile fit at the current r, whose coefficients update r; at
/// convergence the clustered covariance of those coefficients is that of r. Guarded (ADR 0022,
/// Amendment 2026-10-29): a step is halved until every tau is positive and the profile deviance
/// does not rise; convergence is tested on the undamped step. A mode below [`MODE_MIN_PAIRS`]
/// stays at its start.
pub fn mode_costs(s: &Sample, classes: &[&[usize]], p: &Transport) -> Result<Modes, String> {
    let s = international(s);
    let member = members(&s, classes);
    let support = mode_support(&s);
    let identified: [bool; 3] = std::array::from_fn(|m| support[m] >= MODE_MIN_PAIRS);
    let road = p.per_tkm[0];
    let mut r: [f64; 3] = std::array::from_fn(|m| p.per_tkm[m + 1] / road);
    let ln_tariff = s.ln_tariff();
    let base = |t: &[f64]| -> Columns {
        let ln_t: Vec<f64> = t.iter().map(|t| t.ln()).collect();
        let mut reg = Vec::new();
        for (c, m) in member.iter().enumerate() {
            reg.push((format!("ln_tariff_{c}"), masked(&ln_tariff, m)));
            reg.push((format!("ln_tau_{c}"), masked(&ln_t, m)));
        }
        reg
    };
    let d_name = |m: usize| format!("d_{}", MODE_NAMES[m + 1]);
    let mut profile = s.fit(&base(&mode_tau(&s, &r, p)))?;
    let (mut steps, mut halvings, mut deviance) = (Vec::new(), Vec::new(), Vec::new());
    let mut converged = None;
    for iteration in 1..=MODE_MAX_ITER {
        deviance.push(profile.deviance);
        let t = mode_tau(&s, &r, p);
        let mut kappa = vec![0.0; s.len()];
        for (c, m) in member.iter().enumerate() {
            let coef = profile.coef(&format!("ln_tau_{c}"));
            for (k, &m) in kappa.iter_mut().zip(m) {
                *k += coef * f64::from(u8::from(m));
            }
        }
        let mut reg = base(&t);
        for m in (0..3).filter(|&m| identified[m]) {
            let v = (0..s.len())
                .map(|i| kappa[i] * s.weight[i] * road * s.q[i][m + 1] / t[i])
                .collect();
            reg.push((d_name(m), v));
        }
        let fit = s.fit(&reg)?;
        let step: [f64; 3] = std::array::from_fn(|m| {
            if identified[m] {
                fit.coef(&d_name(m))
            } else {
                0.0
            }
        });
        steps.push(step);
        if (0..3).all(|m| step[m].abs() <= MODE_TOL * r[m].abs().max(1.0)) {
            for m in 0..3 {
                r[m] += step[m];
            }
            halvings.push(0);
            converged = Some(fit);
            break;
        }
        let mut alpha = 1.0;
        let mut taken = None;
        for h in 0..=MODE_MAX_HALVINGS {
            let trial: [f64; 3] = std::array::from_fn(|m| r[m] + alpha * step[m]);
            let t = mode_tau(&s, &trial, p);
            if t.iter().all(|&t| t > 0.0 && t.is_finite()) {
                let at = s.fit(&base(&t))?;
                if at.deviance <= profile.deviance {
                    taken = Some((trial, h, at));
                    break;
                }
            }
            alpha *= 0.5;
        }
        let Some((trial, h, at)) = taken else {
            return Err(format!(
                "step {iteration}: within {MODE_MAX_HALVINGS} halvings, no step from {r:?} along \
                 {step:?} keeps tau positive without raising the deviance"
            ));
        };
        r = trial;
        halvings.push(h);
        profile = at;
    }
    let Some(fit) = converged else {
        return Err(format!(
            "mode costs did not converge in {MODE_MAX_ITER} steps: {r:?}"
        ));
    };
    let k = classes.len();
    Ok(Modes {
        ratio: std::array::from_fn(|m| if identified[m] { r[m] } else { f64::NAN }),
        se: std::array::from_fn(|m| {
            if identified[m] {
                fit.se(&d_name(m))
            } else {
                f64::NAN
            }
        }),
        support,
        identified,
        theta_by_class: (0..k)
            .map(|c| -fit.coef(&format!("ln_tariff_{c}")))
            .collect(),
        tau_by_class: (0..k).map(|c| fit.coef(&format!("ln_tau_{c}"))).collect(),
        n: fit.n,
        pseudo_r2: fit.pseudo_r2,
        iterations: steps.len(),
        steps,
        halvings,
        deviance,
    })
}

// ---------------------------------------------------------------- 2000 BACI on the pack

/// Counts for the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Notes {
    pub countries: usize,
    pub countries_with_baci_trade: usize,
    pub unreached_pairs: usize,
    pub baci_international_usd: f64,
    pub baci_international_usd_in_sample: f64,
    pub domestic_countries: usize,
    pub domestic_cells_dropped: usize,
    pub class_trade_usd: Vec<f64>,
}

impl Notes {
    pub fn json(&self) -> Json {
        let int = |n: usize| Json::Int(n as i64);
        Json::object([
            ("countries", int(self.countries)),
            (
                "countries_with_baci_trade",
                int(self.countries_with_baci_trade),
            ),
            ("unreached_pairs", int(self.unreached_pairs)),
            (
                "baci_international_usd",
                Json::Float(self.baci_international_usd),
            ),
            (
                "baci_international_usd_in_sample",
                Json::Float(self.baci_international_usd_in_sample),
            ),
            ("domestic_countries", int(self.domestic_countries)),
            ("domestic_cells_dropped", int(self.domestic_cells_dropped)),
            (
                "class_trade_usd",
                Json::List(
                    self.class_trade_usd
                        .iter()
                        .map(|&x| Json::Float(x))
                        .collect(),
                ),
            ),
        ])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Estimates {
    pub theta: Vec<Theta>,
    pub border: Border,
    /// A failed P04 fit is a diagnostic, not an error of the estimation (ADR 0022, Amendment
    /// 2026-10-29).
    pub modes: Result<Modes, String>,
    /// 2000 BACI international trade by class, the weights of P05.
    pub class_shares: Vec<f64>,
    pub n_countries: usize,
    pub n_domestic_countries: usize,
    pub notes: Notes,
}

/// The international sample (every pair of different pack countries connected by S05, zeros
/// kept) and the sample with domestic flows (ICIO economies only, both ends), and counts for the
/// report.
pub fn samples(pack: &Path, cache: &Path) -> Result<(Sample, Sample, Notes), String> {
    let read = |name: &str| Table::open(&pack.join(name));
    let codes = read("countries.arrow")?.strings("code")?;
    let (iso, _) = read_codes(&cache.join(ADMIN0))?;
    let nc = codes.len();
    let flows = baci_flows(cache, &codes, &iso)?;
    let domestic = icio_domestic(
        cache,
        &codes,
        &iso,
        &read("econ_countries.arrow")?.strings("va_basis")?,
    )?;
    let t = read("econ_tariffs.arrow")?;
    let mut rate = vec![0.0; nc * nc * 2];
    for (((i, e), g), r) in t
        .usize("importer")?
        .into_iter()
        .zip(t.usize("exporter")?)
        .zip(t.usize("group")?)
        .zip(t.f64("rate")?)
    {
        rate[(i * nc + e) * 2 + g] = r;
    }
    let p = Transport::default();
    let costs = country_costs(pack, &p)?;
    let weight = read("econ_goods.arrow")?.f64("weight")?;

    let build = |mask: &dyn Fn(usize, usize, usize) -> bool| -> Sample {
        let mut s = Sample::default();
        for o in 0..nc {
            for d in 0..nc {
                for g in 0..N_TRADED {
                    if !mask(o, d, g) {
                        continue;
                    }
                    let q = *costs.at(o, d, g);
                    let (w, lam) = (weight[g], TIME_COST[g]);
                    let flow = if o == d {
                        let x = domestic[o][g];
                        if x.is_nan() { 0.0 } else { x }
                    } else {
                        flows[(o * nc + d) * N_TRADED + g]
                    };
                    let tariff = if o == d {
                        0.0
                    } else {
                        rate[(d * nc + o) * 2 + TARIFF_GROUP[g]]
                    };
                    s.exporter.push(o);
                    s.importer.push(d);
                    s.good.push(g);
                    s.flow.push(flow);
                    s.tariff.push(tariff);
                    s.tau.push(tau(&q, w, lam, &p));
                    s.q.push(q);
                    s.weight.push(w);
                    s.time_cost.push(lam);
                }
            }
        }
        s
    };
    let pair = |o: usize, d: usize| costs.reach[o * nc + d] && o != d;
    let international = build(&|o, d, _| pair(o, d));
    let icio_ok = |c: usize, g: usize| domestic[c][g].is_finite();
    let both: Vec<bool> = (0..nc)
        .map(|c| (0..N_TRADED).all(|g| icio_ok(c, g)))
        .collect();
    let with_domestic = build(&|o, d, g| {
        (pair(o, d) && both[o] && both[d]) || (o == d && icio_ok(o, g) && costs.reach[o * nc + d])
    });
    let mut traded = 0;
    for c in 0..nc {
        let mut out = Vec::with_capacity(nc * N_TRADED);
        let mut inn = Vec::with_capacity(nc * N_TRADED);
        for x in 0..nc {
            for g in 0..N_TRADED {
                out.push(flows[(c * nc + x) * N_TRADED + g]);
                inn.push(flows[(x * nc + c) * N_TRADED + g]);
            }
        }
        if pairwise_sum(&out) + pairwise_sum(&inn) > 0.0 {
            traded += 1;
        }
    }
    let class_trade_usd = CLASSES
        .iter()
        .map(|c| {
            let v: Vec<f64> = (0..nc * nc)
                .flat_map(|cell| c.iter().map(move |&g| (cell, g)))
                .map(|(cell, g)| flows[cell * N_TRADED + g])
                .collect();
            pairwise_sum(&v)
        })
        .collect();
    let notes = Notes {
        countries: nc,
        countries_with_baci_trade: traded,
        unreached_pairs: (0..nc * nc)
            .filter(|&i| !costs.reach[i] && i / nc != i % nc)
            .count(),
        baci_international_usd: pairwise_sum(&flows),
        baci_international_usd_in_sample: pairwise_sum(&international.flow),
        domestic_countries: both.iter().filter(|&&b| b).count(),
        domestic_cells_dropped: (0..nc)
            .filter(|&c| both[c])
            .map(|c| (0..N_TRADED).filter(|&g| !icio_ok(c, g)).count())
            .sum::<usize>(),
        class_trade_usd,
    };
    Ok((international, with_domestic, notes))
}

pub fn estimate(pack: &Path, cache: &Path) -> Result<Estimates, String> {
    let (international, with_domestic, notes) = samples(pack, cache)?;
    let trade = &notes.class_trade_usd;
    let total = trade.iter().fold(0.0, |a, b| a + b);
    let shares: Vec<f64> = trade.iter().map(|x| x / total).collect();
    let p = Transport::default();
    Ok(Estimates {
        theta: theta_by_class(&international, &CLASSES)?,
        border: border_friction(&with_domestic, &CLASSES, Some(&shares))?,
        modes: mode_costs(&international, &CLASSES, &p),
        class_shares: shares,
        n_countries: notes.countries,
        n_domestic_countries: notes.domestic_countries,
        notes,
    })
}

// ---------------------------------------------------------------- outputs

/// The P04 rows are ratios to road (P04a), the numeraire of [`mode_costs`].
pub const P04_IDS: [&str; 3] = ["P04b/P04a", "P04c/P04a", "P04d/P04a"];

/// One estimated value with its standard error, observations, pseudo-R2 and current default.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: &'static str,
    pub index: usize,
    pub goods: Vec<usize>,
    pub value: f64,
    pub se: f64,
    pub n: usize,
    pub pseudo_r2: f64,
    pub default: f64,
}

pub fn rows(est: &Estimates, p: &Transport) -> Vec<Row> {
    let mut out: Vec<Row> = est
        .theta
        .iter()
        .enumerate()
        .map(|(c, t)| Row {
            id: "P01",
            index: c,
            goods: t.goods.clone(),
            value: t.theta,
            se: t.se,
            n: t.n,
            pseudo_r2: t.pseudo_r2,
            default: THETA_DEFAULT[c],
        })
        .collect();
    let b = &est.border;
    out.push(Row {
        id: "P05",
        index: 0,
        goods: vec![],
        value: b.nu,
        se: b.se,
        n: b.n,
        pseudo_r2: b.pseudo_r2,
        default: BORDER_DEFAULT,
    });
    let Ok(m) = &est.modes else {
        return out;
    };
    for (k, id) in P04_IDS.iter().enumerate() {
        out.push(Row {
            id,
            index: 0,
            goods: vec![],
            value: m.ratio[k],
            se: m.se[k],
            n: m.n,
            pseudo_r2: m.pseudo_r2,
            default: p.per_tkm[k + 1] / p.per_tkm[0],
        });
    }
    out
}

/// `gravity_2000.toml`.
pub fn to_toml(est: &Estimates) -> String {
    let mut lines: Vec<String> = [
        "# P01 theta by class, P05 border friction and P04 mode cost ratios, estimated by PPML structural gravity on",
        "# 2000 BACI with least-cost (S05) costs between the countries of the pack (ADR 0022; written by",
        "# `ceteris gravity --write`).",
        "# TradeParams::default() keeps its values until the owner decides on the switch.",
        "provenance = \"data\"",
        "",
    ]
    .map(str::to_owned)
    .to_vec();
    for r in rows(est, &Transport::default()) {
        lines.extend([
            "[[estimate]]".to_owned(),
            format!("id = \"{}\"", r.id),
            format!("index = {}", r.index),
            format!("goods = {}", int_list(&r.goods)),
            format!("value = {}", num(r.value)),
            format!("se = {}", num(r.se)),
            format!("n = {}", r.n),
            format!("pseudo_r2 = {}", num(r.pseudo_r2)),
            format!("default = {}", num(r.default)),
            String::new(),
        ]);
    }
    lines.join("\n")
}

/// Write `gravity_2000.toml` to `out` when no stop rule fires; whether it wrote.
pub fn write_toml(est: &Estimates, out: &Path) -> Result<bool, String> {
    if !stop_rules(est).is_empty() {
        return Ok(false);
    }
    std::fs::write(out, to_toml(est)).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(true)
}

/// The stop rules: estimates that go to the owner instead of the pack.
pub fn stop_rules(est: &Estimates) -> Vec<String> {
    let mut fired = Vec::new();
    for (c, t) in est.theta.iter().enumerate() {
        if !(1.0..=25.0).contains(&t.theta) || t.se > t.theta.abs() {
            fired.push(format!(
                "P01 class {c}: theta {} (s.e. {})",
                fixed(t.theta, 3),
                fixed(t.se, 3)
            ));
        }
    }
    let b = &est.border;
    if !(0.05..=3.0).contains(&b.nu) {
        fired.push(format!(
            "P05: nu {} (s.e. {})",
            fixed(b.nu, 3),
            fixed(b.se, 3)
        ));
    }
    let [rail, sea, air] = match &est.modes {
        Ok(m) => m.ratio,
        Err(e) => {
            fired.push(format!("P04 did not converge: {e}"));
            return fired;
        }
    };
    // water < rail < road < air, road = 1; NaN (unidentified) fails.
    if !(sea < rail && rail < 1.0 && 1.0 < air) {
        fired.push(format!(
            "P04: ratios to road rail {}, sea {}, air {} break water < rail < road < air",
            py_g(rail, 4),
            py_g(sea, 4),
            py_g(air, 4)
        ));
    }
    fired
}

fn fixed3(v: &[f64]) -> String {
    v.iter()
        .map(|&x| fixed(x, 3))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn report(est: &Estimates) -> String {
    let mut lines = vec![
        "| ID | Class / mode | Estimate | s.e. | N | pseudo-R2 | Default |".to_owned(),
        "|---|---|---|---|---|---|---|".to_owned(),
    ];
    for r in rows(est, &Transport::default()) {
        let label = if r.id == "P01" {
            format!("class {}, goods {}", r.index, int_list(&r.goods))
        } else {
            String::new()
        };
        lines.push(format!(
            "| {} | {label} | {} | {} | {} | {} | {} |",
            r.id,
            py_g(r.value, 4),
            py_g(r.se, 3),
            r.n,
            fixed(r.pseudo_r2, 3),
            py_g(r.default, 4)
        ));
    }
    let b = &est.border;
    lines.push(String::new());
    lines.push(format!(
        "P05 by class: nu {} (s.e. {}); theta in that fit {}; border coefficients {}; class weights {}",
        fixed3(&b.nu_by_class),
        fixed3(&b.se_by_class),
        fixed3(&b.theta_by_class),
        fixed3(&b.border_by_class),
        fixed3(&est.class_shares)
    ));
    match &est.modes {
        Ok(m) => {
            lines.push(format!(
                "P04 fit: theta by class {}, ln tau coefficients {}, {} Gauss-Newton steps \
                 ({} halved), profile deviance {} to {}",
                fixed3(&m.theta_by_class),
                fixed3(&m.tau_by_class),
                m.iterations,
                m.halvings.iter().filter(|&&h| h > 0).count(),
                fixed(m.deviance[0], 3),
                fixed(m.deviance[m.deviance.len() - 1], 3)
            ));
            let p = Transport::default();
            for k in (0..3).filter(|&k| !m.identified[k]) {
                lines.push(format!(
                    "P04: {} not identified ({} country pairs with a positive flow, fewer than \
                     {MODE_MIN_PAIRS}); held at its start {}",
                    MODE_NAMES[k + 1],
                    m.support[k],
                    py_g(p.per_tkm[k + 1] / p.per_tkm[0], 4)
                ));
            }
        }
        Err(e) => lines.push(format!("P04 did not converge: {e}")),
    }
    let t: Vec<String> = est
        .theta
        .iter()
        .map(|x| format!("{} ({})", fixed(x.tau, 3), fixed(x.se_tau, 3)))
        .collect();
    lines.push(format!(
        "ln tau coefficients in the theta fits: {}",
        t.join(", ")
    ));
    lines.join("\n")
}
