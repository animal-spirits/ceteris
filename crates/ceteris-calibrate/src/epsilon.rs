//! The dark distance cost on the crossings (ADR 0272, P158 epsilon): every crossing of good g
//! between countries A and B takes an iceberg factor `ln D = (epsilon / theta_g) (ln d_AB - ln
//! d_0c)` for its theta class c, and epsilon is found by Brent's method so that the model's
//! pooled PPML distance coefficient on distw equals the data's, each trial with the border
//! friction nu refitted to the ICIO shares ([`border::fit_border_nu`]).
//!
//! The model's coefficient is read at the data state: stage 2 at the stage 1 spending of
//! [`steady::data_state_spending`], where the border fit itself is evaluated, not at the
//! default run's last ticks that H2 reads.

use std::collections::BTreeMap;
use std::path::Path;

use ceteris_econ::EconInputs;
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_pack::read::natural_earth::{ADMIN0, IsoCodes, read_codes};
use ceteris_pack::read::tariffs::{GRAVITY, read_distances};
use ceteris_reference::io::Inputs;
use ceteris_reference::ppml;

use crate::fit_cache::TrialCache;
use crate::gravity::baci_flows;
use crate::json::Json;
use crate::run_log::{RunLog, trial_json};
use crate::write::Table;

use crate::border::{self, Brent, Fit};
use crate::steady::{CalibrationError, Limits};

/// The bracket of epsilon searched; ADR 0272 expects about 0.35.
pub const EPSILON_MAX: f64 = 2.0;
/// Brent ends when the bracket is this narrow in epsilon.
pub const EPSILON_TOL: f64 = 1e-3;

/// Brent's bracket on a class's epsilon scale ends this narrow, within this many rounds.
const SCALE_TOL: f64 = 1e-6;
/// Good fitted for its own theta jointly with epsilon (RFC 0355 D3): coal and other mining.
pub const THETA_GOOD: usize = 2;
/// The band theta of [`THETA_GOOD`] is searched in: the Mining estimate 13.53 plus or minus two
/// s.e. (3.67, ADR 0023).
pub const THETA_BAND: (f64, f64) = (6.2, 20.9);
/// theta_2 moves less than this between refits: settled.
const THETA_SETTLE: f64 = 1e-4;
const THETA_TOL: f64 = 1e-6;
const THETA_ROUNDS: usize = 100;
/// The residual read at a theta whose Sinkhorn has no balanced scaling: far below any share miss.
const UNBALANCED_RESIDUAL: f64 = -1.0;
const SCALE_ROUNDS: usize = 100;
/// A residual at scale 1 this small counts as the target met there.
const SCALE_FLAT: f64 = 1e-9;
/// A group of a scaled class whose nu is at most this counts as pinned at 0.
const PIN_NU: f64 = 1e-6;
/// Refits of nu and s_c against each other before the trial gives up.
const SCALE_PASSES: usize = 10;
/// A crossing product (1 + nu) D below 1 by more than this is a miss (ADR 0272, Decision 2).
const PRODUCT_TOL: f64 = 1e-12;

/// The regression sample of the gravity coefficient: ordered country pairs `a != b` with a
/// finite positive distw and finite contiguity, in row-major order of `n_countries` squared.
#[derive(Clone, Debug)]
pub struct Gravity {
    n: usize,
    exporter: Vec<usize>,
    importer: Vec<usize>,
    ln_distw: Vec<f64>,
    contig: Vec<f64>,
}

impl Gravity {
    /// `distw` and `contig` row-major `n` squared, absent pairs NaN.
    pub fn new(n: usize, distw: &[f64], contig: &[f64]) -> Self {
        let mut g = Self {
            n,
            exporter: vec![],
            importer: vec![],
            ln_distw: vec![],
            contig: vec![],
        };
        for k in (0..n * n).filter(|&k| k / n != k % n) {
            if distw[k].is_finite() && distw[k] > 0.0 && contig[k].is_finite() {
                g.exporter.push(k / n);
                g.importer.push(k % n);
                g.ln_distw.push(distw[k].ln());
                g.contig.push(contig[k]);
            }
        }
        g
    }

    /// PPML of the pair values (row-major `n` squared, exporter first) on ln distw and contiguity
    /// with exporter and importer effects, zero flows kept: the distance coefficient and its
    /// Eicker-Huber-White standard error.
    pub fn coefficient(&self, pair_value: &[f64]) -> Result<(f64, f64), String> {
        let value: Vec<f64> = self
            .exporter
            .iter()
            .zip(&self.importer)
            .map(|(&e, &i)| pair_value[e * self.n + i])
            .collect();
        let r = ppml::fit_robust(
            &value,
            &self.exporter,
            &self.importer,
            &[("ln_distw", &self.ln_distw), ("contig", &self.contig)],
        )
        .map_err(|e| format!("distance coefficient: {e:?}"))?;
        Ok((r.coef[0], r.se[0]))
    }
}

/// Where the pair of a crossing's gateways lies: countries of its row and column gateway.
fn pair_of(inp: &Inputs, j: usize) -> (usize, usize) {
    let c = &inp.crossing;
    (inp.gateway_country[c.row[j]], inp.gateway_country[c.col[j]])
}

/// Per theta class, over the country pairs a crossing of one of its goods links: the
/// `weight`-weighted mean of `ln_d` (`weight` row-major, pairs counted in both directions) and
/// the smallest `ln_d`. A class without a linked pair of positive weight takes the smallest for
/// both, and a class with no linked pair NaN.
pub fn class_offsets(
    inp: &Inputs,
    theta_class: &[usize],
    ln_d: &[f64],
    weight: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let n_c = inp.n_countries();
    let n_classes = border::N_CLASSES;
    let mut linked = vec![vec![false; n_c * n_c]; n_classes];
    for j in 0..inp.crossing.good.len() {
        let g = inp.crossing.good[j];
        if g < theta_class.len() && inp.crossing.k[j] > 0.0 {
            let (a, b) = pair_of(inp, j);
            let (a, b) = (a.min(b), a.max(b));
            linked[theta_class[g]][a * n_c + b] = true;
        }
    }
    let (mut mean, mut min) = (vec![f64::NAN; n_classes], vec![f64::NAN; n_classes]);
    for c in 0..n_classes {
        let (mut sum, mut total) = (0.0, 0.0);
        for k in (0..n_c * n_c).filter(|&k| linked[c][k]) {
            let (a, b) = (k / n_c, k % n_c);
            let l = ln_d[a * n_c + b];
            if !l.is_finite() {
                continue;
            }
            let w = weight[a * n_c + b] + weight[b * n_c + a];
            sum += w * l;
            total += w;
            min[c] = if min[c].is_nan() { l } else { min[c].min(l) };
        }
        mean[c] = if total > 0.0 { sum / total } else { min[c] };
    }
    (mean, min)
}

/// `ln D` of a crossing of good `g` between the countries of `pair`.
fn ln_factor(inp: &Inputs, epsilon: f64, ln_rel: f64, g: usize) -> f64 {
    if ln_rel.is_finite() {
        epsilon / inp.theta[g] * ln_rel
    } else {
        0.0
    }
}

/// The inputs with every crossing multiplied by the dark distance cost `D` at `epsilon`:
/// `ln_d0` and the epsilon scale `scale` (s_c, ADR 0375) per theta class, `ln_d` the country-pair
/// distance row-major; a crossing of class c sees epsilon `scale[c] * epsilon`. K scales by `D^-theta`,
/// the share arriving by `1 / D`, and the base prices and selection constants are recomputed. The
/// inputs must not carry a distance cost already.
pub fn at_epsilon(
    inp: &Inputs,
    theta_class: &[usize],
    ln_d: &[f64],
    ln_d0: &[f64],
    scale: &[f64],
    epsilon: f64,
) -> Result<Inputs, String> {
    let mut out = inp.clone();
    if epsilon == 0.0 {
        return Ok(out);
    }
    let n_c = inp.n_countries();
    let cro = &mut out.crossing;
    for j in 0..cro.good.len() {
        let g = cro.good[j];
        if g >= theta_class.len() {
            continue;
        }
        let (a, b) = (
            inp.gateway_country[cro.row[j]],
            inp.gateway_country[cro.col[j]],
        );
        let c = theta_class[g];
        let ln_dd = ln_factor(inp, scale[c] * epsilon, ln_d[a * n_c + b] - ln_d0[c], g);
        cro.k[j] *= libm::exp(-inp.theta[g] * ln_dd);
        cro.inv_loss[j] *= libm::exp(-ln_dd);
    }
    border::reprice(&mut out)?;
    Ok(out)
}

/// The inputs at the dark distance cost a calibration was solved at ([`at_epsilon`]).
pub fn at_distance(inp: &Inputs, d: &crate::write::Distance) -> Result<Inputs, String> {
    let n_traded = inp.traded.iter().filter(|&&t| t).count();
    at_epsilon(
        inp,
        &inp.theta_class[..n_traded],
        &d.ln_d,
        &d.ln_d0,
        &d.scale,
        d.epsilon,
    )
}

/// The classes where some crossing's `(1 + nu) D` is below 1 (ADR 0272, Decision 2), for the
/// friction `nu` per good of `at` as it was fitted.
fn product_misses(
    inp: &Inputs,
    theta_class: &[usize],
    ln_d: &[f64],
    ln_d0: &[f64],
    scale: &[f64],
    epsilon: f64,
    nu: &[f64],
) -> Vec<usize> {
    let n_c = inp.n_countries();
    let mut bad = [false; border::N_CLASSES];
    for j in 0..inp.crossing.good.len() {
        let g = inp.crossing.good[j];
        if g >= theta_class.len() || inp.crossing.k[j] <= 0.0 {
            continue;
        }
        let (a, b) = pair_of(inp, j);
        let c = theta_class[g];
        let ln_dd = ln_factor(inp, scale[c] * epsilon, ln_d[a * n_c + b] - ln_d0[c], g);
        if ln_dd + libm::log1p(nu[g]) < -PRODUCT_TOL {
            bad[c] = true;
        }
    }
    (0..bad.len()).filter(|&c| bad[c]).collect()
}

/// One theta class's held-out distance coefficient, model against data.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassCheck {
    pub class: usize,
    pub model: f64,
    pub data: f64,
    pub data_se: f64,
    /// `|model - data| <= 2 data_se`.
    pub within: bool,
}

/// The fit at one epsilon.
#[derive(Clone, Debug)]
pub struct Trial {
    pub epsilon: f64,
    /// theta of [`THETA_GOOD`] as used (fitted when `Targets::theta_fit`).
    pub theta2: f64,
    pub fit: Fit,
    /// `ln d_0` per theta class as used.
    pub ln_d0: Vec<f64>,
    /// Classes whose `d_0` went to the smallest linked distance for `(1 + nu) D >= 1`.
    pub floored: Vec<usize>,
    /// The epsilon scale s_c per theta class (ADR 0375): 1 unless the class's nu clamped at 0.
    pub scale: Vec<f64>,
    pub pooled: f64,
    pub classes: Vec<f64>,
}

/// The fitted epsilon.
#[derive(Clone, Debug)]
pub struct EpsilonFit {
    pub trial: Trial,
    /// The data's pooled coefficient on distw.
    pub data_pooled: f64,
    pub held_out: Vec<ClassCheck>,
    /// Trials, one nu fit each.
    pub evaluations: usize,
    /// The data's coefficient is already met at epsilon 0, which is kept.
    pub at_zero: bool,
}

/// What the epsilon fit reads besides the inputs.
pub struct Targets<'a> {
    /// Fit theta of [`THETA_GOOD`] at each epsilon trial so its cross-border share at nu 0 meets
    /// its target (RFC 0355 D3).
    pub theta_fit: bool,
    /// The ICIO cross-border share per friction group of [`border::friction_groups`].
    pub nu_target: &'a [f64],
    pub icio: &'a [bool],
    /// The theta class of each traded good.
    pub theta_class: &'a [usize],
    /// The model's country-pair distance, row-major, natural log, NaN where absent.
    pub ln_d: &'a [f64],
    /// Pair trade weights of `d_0`, row-major, exporter first: the data's 2000 trade.
    pub weight: &'a [f64],
    pub gravity: &'a Gravity,
    /// The data's 2000 trade by exporter, importer and traded good,
    /// `[(o * C + d) * theta_class.len() + g]` as `gravity::baci_flows`.
    pub data_flows: &'a [f64],
}

/// `r` with `what` prefixed to its error: a fit that fails after hours says which probe it was on.
fn within<T>(
    r: Result<T, CalibrationError>,
    what: impl FnOnce() -> String,
) -> Result<T, CalibrationError> {
    r.map_err(|e| match e {
        CalibrationError::Failed(m) => format!("{}: {m}", what()).into(),
        CalibrationError::Unbalanced(m) => CalibrationError::Unbalanced(format!("{}: {m}", what())),
        other => other,
    })
}

fn sums(values: &[f64], n_pairs: usize, stride: usize, goods: &[usize]) -> Vec<f64> {
    (0..n_pairs)
        .map(|k| goods.iter().map(|&g| values[k * stride + g]).sum())
        .collect()
}

/// s in [0, 1] such that the groups `ks` of class `c`, held at nu 0 with the other groups at `nu`,
/// buy their target cross-border share pooled (ADR 0376), by Brent's method. The residual is the
/// value bought from abroad less the target share of the total, over the groups' total; the share
/// rises as s falls and the target is above it at s = 1 (the clamp).
#[allow(clippy::too_many_arguments)]
fn solve_scale(
    inp: &Inputs,
    t: &Targets,
    ln_d0: &[f64],
    scale: &[f64],
    epsilon: f64,
    c: usize,
    ks: &[usize],
    nu: &[f64],
    settled: bool,
    limits: Limits,
) -> Result<f64, CalibrationError> {
    let groups = border::friction_groups(t.theta_class);
    let f = |s: f64| -> Result<f64, CalibrationError> {
        let mut sc = scale.to_vec();
        sc[c] = s;
        let at = at_epsilon(inp, t.theta_class, t.ln_d, ln_d0, &sc, epsilon)?;
        let (abroad, total) = within(
            border::data_state_parts(&at, nu, &groups, t.icio, limits),
            || format!("epsilon {epsilon}: s_c of class {c} probed at {s}, scales {sc:?}"),
        )?;
        let (miss, all) = ks.iter().fold((0.0, 0.0), |(m, a), &k| {
            (m + abroad[k] - t.nu_target[k] * total[k], a + total[k])
        });
        Ok(miss / all)
    };
    let (f_lo, f_hi) = (f(0.0)?, f(1.0)?);
    // Short of the target even without distance cost. The first time it can be the nu of the other
    // groups, not yet refitted: s = 0, and the next refit says. Once refitted it is infeasible.
    if f_lo < -SCALE_FLAT {
        if settled {
            return Err(format!(
                "class {c}: target cross-border share outside the model's even at scale 0 (residual {f_lo})"
            )
            .into());
        }
        return Ok(0.0);
    }
    if f_hi > SCALE_FLAT {
        return Err(format!(
            "class {c}: pooled cross-border share above its target at scale 1 (residuals {f_lo} at scale 0, {f_hi} at scale 1)"
        )
        .into());
    }
    if f_hi >= 0.0 {
        return Ok(1.0);
    }
    let mut brent = Brent::new(0.0, f_lo, 1.0, f_hi);
    for _ in 0..SCALE_ROUNDS {
        let Some(s) = brent.next(SCALE_TOL) else {
            break;
        };
        brent.update(f(s)?);
    }
    Ok(brent.root())
}

/// `inp` with the kernel of good `g` rebuilt at theta `theta`: K to the power `theta / theta_g`
/// (K is a power of the leg factor, so the legs are unchanged), the base prices and selection
/// constants recomputed.
pub fn at_theta(inp: &Inputs, g: usize, theta: f64) -> Result<Inputs, String> {
    let mut out = inp.clone();
    let ratio = theta / inp.theta[g];
    for j in 0..out.crossing.good.len() {
        if out.crossing.good[j] == g {
            out.crossing.k[j] = out.crossing.k[j].powf(ratio);
        }
    }
    out.theta[g] = theta;
    border::reprice(&mut out)?;
    Ok(out)
}

/// `inp` at the theta of [`THETA_GOOD`] whose group's cross-border share at nu 0, at `epsilon`
/// with the good's class at scale 0 (no distance cost of its own; the other classes still move its
/// prices at their `scale` and `nu`) and `ln_d0`, meets its target, found by Brent's method in
/// [`THETA_BAND`] (see [`theta_root`]); the share falls as theta rises.
#[allow(clippy::too_many_arguments)]
fn fit_theta(
    inp: &Inputs,
    t: &Targets,
    epsilon: f64,
    ln_d0: &[f64],
    scale: &[f64],
    nu_of: &[f64],
    limits: Limits,
) -> Result<Inputs, CalibrationError> {
    let groups = border::friction_groups(t.theta_class);
    let k = groups
        .iter()
        .position(|goods| goods.contains(&THETA_GOOD))
        .ok_or("no friction group holds the good whose theta is fitted")?;
    let target = t.nu_target[k];
    if target.is_nan() {
        return Ok(inp.clone());
    }
    let mut scale = scale.to_vec();
    scale[t.theta_class[THETA_GOOD]] = 0.0;
    let f = |theta: f64| -> Result<f64, CalibrationError> {
        let at = at_epsilon(
            &at_theta(inp, THETA_GOOD, theta)?,
            t.theta_class,
            t.ln_d,
            ln_d0,
            &scale,
            epsilon,
        )?;
        let mut nu = nu_of.to_vec();
        for &g in &groups[k] {
            nu[g] = 0.0;
        }
        let shares = within(
            border::data_state_shares(&at, &nu, &groups, t.icio, limits),
            || format!("epsilon {epsilon}: theta_2 probed at {theta}, scales {scale:?}"),
        )?;
        Ok(shares[k] - target)
    };
    let theta = theta_root(f, |f_lo, f_hi| {
        format!(
            "epsilon {epsilon}: good {THETA_GOOD}'s share at nu 0 misses its target {target} for every theta in {THETA_BAND:?} (residuals {f_lo} and {f_hi})"
        )
    })?;
    Ok(at_theta(inp, THETA_GOOD, theta)?)
}

/// The root of `f` (share minus target, falling in theta) in [`THETA_BAND`]. A probe whose
/// Sinkhorn has no balanced scaling is past the root and reads as [`UNBALANCED_RESIDUAL`]; the
/// band's lower end must balance. A band that does not bracket the root is an error, not a clamp.
fn theta_root(
    f: impl Fn(f64) -> Result<f64, CalibrationError>,
    misses: impl FnOnce(f64, f64) -> String,
) -> Result<f64, CalibrationError> {
    let probe = |theta: f64| match f(theta) {
        Err(CalibrationError::Unbalanced(_)) if theta > THETA_BAND.0 => Ok(UNBALANCED_RESIDUAL),
        r => r,
    };
    let (f_lo, f_hi) = (probe(THETA_BAND.0)?, probe(THETA_BAND.1)?);
    if f_lo < 0.0 || f_hi > 0.0 {
        return Err(misses(f_lo, f_hi).into());
    }
    let mut brent = Brent::new(THETA_BAND.0, f_lo, THETA_BAND.1, f_hi);
    for _ in 0..THETA_ROUNDS {
        let Some(x) = brent.next(THETA_TOL) else {
            break;
        };
        brent.update(probe(x)?);
    }
    Ok(brent.root())
}

/// The fit at one `epsilon`: nu per group, `d_0` and the epsilon scale per class.
pub fn fit_at_epsilon(
    inp: &Inputs,
    t: &Targets,
    epsilon: f64,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Trial, CalibrationError> {
    trial(inp, t, epsilon, limits, progress)
}

fn trial(
    inp: &Inputs,
    t: &Targets,
    epsilon: f64,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Trial, CalibrationError> {
    let groups = border::friction_groups(t.theta_class);
    let (mean, min) = class_offsets(inp, t.theta_class, t.ln_d, t.weight);
    let refit_theta = t.theta_fit && epsilon > 0.0;
    let mut inp = if refit_theta {
        fit_theta(
            inp,
            t,
            epsilon,
            &mean,
            &[1.0; border::N_CLASSES],
            &inp.border_nu,
            limits,
        )?
    } else {
        inp.clone()
    };
    let inp = &mut inp;
    // class 3 is at scale 0 in the theta fits: its d_0 does not enter, the other classes' (floored
    // ones included) do
    let mut ln_d0 = mean;
    let mut floored: Vec<usize> = vec![];
    let mut scale = vec![1.0; border::N_CLASSES];
    let mut scaled = [false; border::N_CLASSES];
    let mut passes = 0;
    loop {
        let at = at_epsilon(inp, t.theta_class, t.ln_d, &ln_d0, &scale, epsilon)?;
        let fit = within(
            border::fit_border_nu(&at, &groups, t.nu_target, t.icio, limits, progress),
            || {
                format!(
                    "epsilon {epsilon}: border fit at scales {scale:?}, theta_2 {}, pass {passes}",
                    inp.theta[THETA_GOOD]
                )
            },
        )?;
        let nu = fit.nu_of_goods(&at.border_nu);
        let bad = product_misses(inp, t.theta_class, t.ln_d, &ln_d0, &scale, epsilon, &nu);
        if bad.is_empty() {
            // theta_2 was fitted before the other classes' s_c and nu were known; refit it at
            // the current state so its share at nu 0 meets the target there too
            if refit_theta {
                let refit = fit_theta(inp, t, epsilon, &ln_d0, &scale, &nu, limits)?;
                let moved = (refit.theta[THETA_GOOD] - inp.theta[THETA_GOOD]).abs() > THETA_SETTLE;
                progress(&format!(
                    "epsilon {epsilon}: theta_2 {} at the current scales",
                    refit.theta[THETA_GOOD]
                ));
                if moved {
                    *inp = refit;
                    passes += 1;
                    if passes > SCALE_PASSES {
                        return Err("theta_2 and the epsilon scales did not settle".into());
                    }
                    continue;
                }
            }
            // A class with groups pinned at nu 0 takes its own s_c (ADR 0376), solved on the
            // pinned groups pooled, so a per-good class has one s_c too. s_c is re-solved at the
            // refitted nu of the other groups until it settles.
            let mut moved = false;
            // epsilon 0 has no distance cost to scale
            for c in (0..border::N_CLASSES).filter(|_| epsilon > 0.0) {
                let pinned: Vec<usize> = (0..groups.len())
                    .filter(|&k| t.theta_class[groups[k][0]] == c && !fit.target[k].is_nan())
                    .filter(|&k| fit.clamped[k] || (scaled[c] && fit.nu[k] <= PIN_NU))
                    .collect();
                if pinned.is_empty() {
                    continue;
                }
                let mut nu_k = nu.clone();
                for &g in pinned.iter().flat_map(|&k| &groups[k]) {
                    nu_k[g] = 0.0;
                }
                let s = solve_scale(
                    inp, t, &ln_d0, &scale, epsilon, c, &pinned, &nu_k, scaled[c], limits,
                )?;
                moved |= !scaled[c] || (s - scale[c]).abs() > 10.0 * SCALE_TOL;
                scale[c] = s;
                scaled[c] = true;
                progress(&format!(
                    "epsilon {epsilon}: class {c} groups {pinned:?} at nu 0, epsilon scale {s}"
                ));
            }
            if moved {
                passes += 1;
                if passes > SCALE_PASSES {
                    return Err(format!("epsilon scales {scale:?} did not settle").into());
                }
                continue;
            }
            let (spend, supply) = border::spending_at(&at, &nu, limits)?;
            let pairs = within(
                border::data_state_pair_values(&at, &nu, &spend, &supply, limits),
                || format!("epsilon {epsilon}: pair values at the settled state, scales {scale:?}"),
            )?;
            let (n_c, n_g) = (at.n_countries(), at.n_goods());
            let traded: Vec<usize> = (0..t.theta_class.len()).collect();
            let pooled = t
                .gravity
                .coefficient(&sums(&pairs, n_c * n_c, n_g, &traded))?
                .0;
            let classes = (0..border::N_CLASSES)
                .map(|c| {
                    let goods: Vec<usize> = traded
                        .iter()
                        .copied()
                        .filter(|&g| t.theta_class[g] == c)
                        .collect();
                    if goods.is_empty() {
                        return Ok(f64::NAN);
                    }
                    t.gravity
                        .coefficient(&sums(&pairs, n_c * n_c, n_g, &goods))
                        .map(|r| r.0)
                })
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(Trial {
                epsilon,
                theta2: inp.theta[THETA_GOOD],
                fit,
                ln_d0,
                floored,
                scale,
                pooled,
                classes,
            });
        }
        // after the floor D >= 1 everywhere, so a second pass cannot miss
        if bad.iter().all(|c| floored.contains(c)) {
            return Err(
                format!("classes {bad:?} miss (1 + nu) D >= 1 at their smallest d_0").into(),
            );
        }
        for c in bad {
            ln_d0[c] = min[c];
            floored.push(c);
        }
        progress(&format!(
            "epsilon {epsilon}: (1 + nu) D < 1 in classes {floored:?}; d_0 at their smallest linked distance"
        ));
    }
}

/// epsilon in `[0, EPSILON_MAX]` such that the model's pooled distance coefficient equals the
/// data's, by Brent's method; the coefficient falls as epsilon rises. A model already at or
/// below the data's keeps epsilon 0. Held-out: each class's coefficient against the data's.
pub fn fit_epsilon(
    inp: &Inputs,
    t: &Targets,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<EpsilonFit, CalibrationError> {
    fit_epsilon_cached(inp, t, limits, None, progress)
}

/// [`fit_epsilon`] replaying and storing its trials in `cache` ([`TrialCache`]); the result is
/// bit-identical to the uncached fit.
pub fn fit_epsilon_cached(
    inp: &Inputs,
    t: &Targets,
    limits: Limits,
    cache: Option<&Path>,
    progress: &mut dyn FnMut(&str),
) -> Result<EpsilonFit, CalibrationError> {
    fit_epsilon_inner(inp, t, limits, cache, None, None, progress)
}

/// One trial at the given `epsilon`, no search: nu, `d_0` and s_c fitted there, the pooled and
/// held-out coefficients reported as [`fit_epsilon`] does. For checking a known epsilon in
/// minutes rather than hours.
pub fn fit_epsilon_at(
    inp: &Inputs,
    t: &Targets,
    epsilon: f64,
    limits: Limits,
    cache: Option<&Path>,
    progress: &mut dyn FnMut(&str),
) -> Result<EpsilonFit, CalibrationError> {
    fit_epsilon_inner(inp, t, limits, cache, Some(epsilon), None, progress)
}

/// [`fit_epsilon_cached`], or [`fit_epsilon_at`] with `fixed`, writing a checkpoint per trial to
/// `log`; the result is bit-identical.
pub fn fit_epsilon_logged(
    inp: &Inputs,
    t: &Targets,
    limits: Limits,
    cache: Option<&Path>,
    fixed: Option<f64>,
    log: &RunLog,
    progress: &mut dyn FnMut(&str),
) -> Result<EpsilonFit, CalibrationError> {
    fit_epsilon_inner(inp, t, limits, cache, fixed, Some(log), progress)
}

struct Computed {
    lines: Vec<String>,
    result: Result<Option<Trial>, CalibrationError>,
    cached: bool,
}

fn fit_epsilon_inner(
    inp: &Inputs,
    t: &Targets,
    limits: Limits,
    cache: Option<&Path>,
    fixed: Option<f64>,
    log: Option<&RunLog>,
    progress: &mut dyn FnMut(&str),
) -> Result<EpsilonFit, CalibrationError> {
    let cache = cache.map(|dir| TrialCache::new(dir, inp, t, limits));
    let n_c = inp.n_countries();
    let n_t = t.theta_class.len();
    let all: Vec<usize> = (0..n_t).collect();
    let data_pooled = t
        .gravity
        .coefficient(&sums(t.data_flows, n_c * n_c, n_t, &all))?
        .0;
    let mut evaluations = 0;
    // A trial where no nu reaches a group's target share (its share at NU_MAX is still above
    // it) has too little distance cost: None, and f counts as the data's magnitude above.
    // Computing a trial touches nothing shared, so two can run at once; `commit` then applies
    // their progress lines, cache store and log checkpoint in the sequential order.
    let compute = |eps: f64| -> Computed {
        if let Some(hit) = cache.as_ref().and_then(|c| c.load(eps)) {
            return Computed {
                lines: vec![format!("epsilon {eps}: trial from the cache")],
                result: Ok(hit),
                cached: true,
            };
        }
        let mut lines = vec![];
        let result = match trial(inp, t, eps, limits, &mut |l| lines.push(l.to_string())) {
            Ok(r) => {
                lines.push(format!(
                    "epsilon {eps}: pooled distance coefficient {} against the data's {data_pooled}",
                    r.pooled
                ));
                Ok(Some(r))
            }
            Err(CalibrationError::Failed(e)) if e.contains("outside the model's") => {
                lines.push(format!(
                    "epsilon {eps}: no nu reaches the target shares: {e}"
                ));
                Ok(None)
            }
            Err(e) => Err(e),
        };
        Computed {
            lines,
            result,
            cached: false,
        }
    };
    let mut commit = |eps: f64, c: Computed| -> Result<Option<Trial>, CalibrationError> {
        evaluations += 1;
        for l in &c.lines {
            progress(l);
        }
        let result = c.result?;
        if c.cached {
            return Ok(result);
        }
        if let Some(c) = &cache {
            c.store(eps, &result)?;
        }
        if let Some(l) = log {
            l.checkpoint(
                "trial",
                result
                    .as_ref()
                    .map_or(Json::Null, |r| trial_json(r, data_pooled)),
            );
        }
        Ok(result)
    };
    let f_of = |r: &Option<Trial>| {
        r.as_ref()
            .map_or(data_pooled.abs(), |r| r.pooled - data_pooled)
    };
    // The two ends of the bracket are independent; the top is committed only if the search
    // reaches it, so a wasted top trial leaves no trace.
    let (zero, mut top_ahead) = match fixed {
        Some(_) => (None, None),
        None => {
            let (z, h) = rayon::join(|| compute(0.0), || compute(EPSILON_MAX));
            (commit(0.0, z)?, Some(h))
        }
    };
    let f_lo = f_of(&zero);
    let (best, at_zero) = if let Some(eps) = fixed {
        let r = commit(eps, compute(eps))?;
        (
            r.ok_or_else(|| format!("no nu reaches the target shares at epsilon {eps}"))?,
            false,
        )
    } else if f_lo <= 0.0 {
        (zero.expect("a trial at f <= 0"), true)
    } else {
        let top = commit(
            EPSILON_MAX,
            top_ahead.take().expect("computed with the zero"),
        )?;
        let f_hi = f_of(&top);
        if f_hi > 0.0 {
            return Err(format!(
                "the pooled distance coefficient is still above the data's {data_pooled} at epsilon {EPSILON_MAX}"
            )
            .into());
        }
        let mut brent = Brent::new(0.0, f_lo, EPSILON_MAX, f_hi);
        let mut last = top;
        while let Some(x) = brent.next(EPSILON_TOL) {
            let r = commit(x, compute(x))?;
            brent.update(f_of(&r));
            if r.is_some() {
                last = r;
            }
        }
        let root = brent.root();
        if last.as_ref().map(|r| r.epsilon) != Some(root) {
            last = commit(root, compute(root))?;
        }
        (
            last.ok_or_else(|| format!("no nu reaches the target shares at epsilon {root}"))?,
            false,
        )
    };
    let held_out = (0..border::N_CLASSES)
        .filter(|&c| !best.classes[c].is_nan())
        .map(|c| {
            let goods: Vec<usize> = all
                .iter()
                .copied()
                .filter(|&g| t.theta_class[g] == c)
                .collect();
            let (data, data_se) =
                t.gravity
                    .coefficient(&sums(t.data_flows, n_c * n_c, n_t, &goods))?;
            let model = best.classes[c];
            Ok(ClassCheck {
                class: c,
                model,
                data,
                data_se,
                within: (model - data).abs() <= 2.0 * data_se,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(EpsilonFit {
        trial: best,
        data_pooled,
        held_out,
        evaluations,
        at_zero,
    })
}

/// The countries the pack's rest of world absorbed and their GDP, if the pack has them.
#[allow(clippy::type_complexity)]
fn read_members(pack: &Path) -> Result<Option<(Vec<String>, Vec<f64>)>, String> {
    let file = pack.join("row_members.arrow");
    if !file.exists() {
        return Ok(None);
    }
    let t = Table::open(&file)?;
    Ok(Some((t.strings("code")?, t.f64("gdp")?)))
}

/// BACI flows, CEPII distw and contiguity of a pack whose last country is the rest of world:
/// flows into and out of it are the sums over its members; its distance to a country is the
/// members' mean weighted by GDP (members without a CEPII pair skipped; unweighted if the weights
/// vanish). Row-major `n` squared, the rest of world's own pair NaN in distw and contig.
#[allow(clippy::type_complexity)]
fn row_data(
    codes: &[String],
    iso3: &[String],
    iso: &IsoCodes,
    cache: &Path,
    cepii: &BTreeMap<(String, String), (f64, f64)>,
    members: &[String],
    gdp: &[f64],
) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let n = codes.len();
    let r = n - 1;
    let mut ext: Vec<String> = codes[..r].to_vec();
    ext.extend(members.iter().cloned());
    let nc = ext.len();
    let flows = baci_flows(cache, &ext, iso)?;
    let n_g = flows.len() / (nc * nc);
    let at = |i: usize| i.min(r);
    let mut data_flows = vec![0.0; n * n * n_g];
    for o in 0..nc {
        for d in 0..nc {
            let cell = (at(o) * n + at(d)) * n_g;
            for g in 0..n_g {
                data_flows[cell + g] += flows[(o * nc + d) * n_g + g];
            }
        }
    }
    let member_iso: Vec<String> = members
        .iter()
        .map(|c| iso.get(c).cloned().unwrap_or_else(|| c.clone()))
        .collect();
    let (mut distw, mut contig) = (vec![f64::NAN; n * n], vec![f64::NAN; n * n]);
    for o in 0..n {
        for d in (0..n).filter(|&d| d != o) {
            let pair = |k: usize| -> Option<(f64, f64, f64)> {
                let key = match (o == r, d == r) {
                    (false, false) => (iso3[o].clone(), iso3[d].clone()),
                    (false, true) => (iso3[o].clone(), member_iso[k].clone()),
                    (true, false) => (member_iso[k].clone(), iso3[d].clone()),
                    (true, true) => return None,
                };
                let w = if o == r || d == r { gdp[k] } else { 1.0 };
                cepii.get(&key).map(|&(a, b)| (a, b, w))
            };
            let found: Vec<(f64, f64, f64)> = if o == r || d == r {
                (0..members.len()).filter_map(pair).collect()
            } else {
                pair(0).into_iter().collect()
            };
            // each quantity over the members that have it, GDP-weighted (unweighted if the
            // weights vanish)
            let mean = |pick: fn(&(f64, f64, f64)) -> f64| -> f64 {
                let have: Vec<&(f64, f64, f64)> =
                    found.iter().filter(|p| pick(p).is_finite()).collect();
                let total: f64 = have.iter().map(|p| p.2).sum();
                if have.is_empty() {
                    f64::NAN
                } else if total > 0.0 {
                    have.iter().map(|p| p.2 * pick(p)).sum::<f64>() / total
                } else {
                    have.iter().map(|p| pick(p)).sum::<f64>() / have.len() as f64
                }
            };
            distw[o * n + d] = mean(|p| p.0);
            contig[o * n + d] = mean(|p| p.1);
        }
    }
    Ok((data_flows, distw, contig))
}

/// What [`Targets`] borrows besides the inputs, read from the pack and the data cache.
pub struct PackData {
    pub ln_d: Vec<f64>,
    pub weight: Vec<f64>,
    pub gravity: Gravity,
    pub data_flows: Vec<f64>,
}

/// The model's country-pair distance (pop-weighted distw of the pack's provinces), the data's
/// 2000 BACI flows and their pair totals, and the CEPII 2000 distw and contiguity of the pack's
/// countries, from the pack in `pack` and the source files in `cache`.
pub fn pack_data(pack: &Path, cache: &Path) -> Result<PackData, String> {
    let codes = Table::open(&pack.join("countries.arrow"))?.strings("code")?;
    let n = codes.len();
    let (iso, _) = read_codes(&cache.join(ADMIN0))?;
    let iso3: Vec<String> = codes
        .iter()
        .map(|c| iso.get(c).cloned().unwrap_or_else(|| c.clone()))
        .collect();
    let cepii: BTreeMap<(String, String), (f64, f64)> = read_distances(&cache.join(GRAVITY), 2000)?
        .into_iter()
        .map(|r| ((r.iso3_o, r.iso3_d), (r.distw, r.contig)))
        .collect();
    // A small pack's last country is the rest of world, with the countries it absorbed in
    // `row_members` (RFC 0357); a pack without that table is read as before.
    let members = read_members(pack)?;
    let (data_flows, distw, contig) = match &members {
        None => {
            let data_flows = baci_flows(cache, &codes, &iso)?;
            let (mut distw, mut contig) = (vec![f64::NAN; n * n], vec![f64::NAN; n * n]);
            for o in 0..n {
                for d in (0..n).filter(|&d| d != o) {
                    if let Some(&(w, c)) = cepii.get(&(iso3[o].clone(), iso3[d].clone())) {
                        (distw[o * n + d], contig[o * n + d]) = (w, c);
                    }
                }
            }
            (data_flows, distw, contig)
        }
        Some((member_codes, gdp)) => {
            row_data(&codes, &iso3, &iso, cache, &cepii, member_codes, gdp)?
        }
    };
    let n_g = data_flows.len() / (n * n);
    let weight = (0..n * n)
        .map(|k| data_flows[k * n_g..(k + 1) * n_g].iter().sum())
        .collect();

    let graph = Pack::load(pack, &TransportParams::default()).map_err(|e| e.to_string())?;
    let econ = EconInputs::from_pack_data(&graph, pack).map_err(|e| e.to_string())?;
    if econ.n_countries() != n {
        return Err(format!(
            "{} pack countries, {} in the economy",
            n,
            econ.n_countries()
        ));
    }
    let ln_d = econ.country_distw(&graph).iter().map(|d| d.ln()).collect();
    Ok(PackData {
        ln_d,
        weight,
        gravity: Gravity::new(n, &distw, &contig),
        data_flows,
    })
}

/// Epsilon, `ln d_0` per class, the scale s_c per class and nu per friction group of an
/// `epsilon.json` ([`EpsilonFit::json`]); a file without `scale` (written before ADR 0375) has
/// s_c = 1.
#[allow(clippy::type_complexity)]
pub fn read_fit(file: &std::path::Path) -> Result<(f64, Vec<f64>, Vec<f64>, Vec<f64>), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    let list = |k: &str| -> Result<Vec<f64>, String> {
        v[k].as_array()
            .and_then(|a| a.iter().map(serde_json::Value::as_f64).collect())
            .ok_or(format!("{}: no numbers under \"{k}\"", file.display()))
    };
    let epsilon = v["epsilon"]
        .as_f64()
        .ok_or(format!("{}: no epsilon", file.display()))?;
    let ln_d0 = list("ln_d0")?;
    let scale = if v.get("scale").is_some() {
        list("scale")?
    } else {
        vec![1.0; ln_d0.len()]
    };
    Ok((epsilon, ln_d0, scale, list("nu")?))
}

impl EpsilonFit {
    /// The fit as printed and written: epsilon, the pooled coefficient against the data's, the
    /// nu, the floored classes and each class's held-out check.
    pub fn json(&self) -> Json {
        let floats = |v: &[f64]| Json::List(v.iter().map(|&x| Json::Float(x)).collect());
        let t = &self.trial;
        Json::object([
            ("epsilon", Json::Float(t.epsilon)),
            ("at_zero", Json::Int(i64::from(self.at_zero))),
            ("evaluations", Json::Int(self.evaluations as i64)),
            ("theta2", Json::Float(t.theta2)),
            ("model_pooled", Json::Float(t.pooled)),
            ("data_pooled", Json::Float(self.data_pooled)),
            ("nu", floats(&t.fit.nu)),
            ("ln_d0", floats(&t.ln_d0)),
            ("scale", floats(&t.scale)),
            (
                "floored",
                Json::List(t.floored.iter().map(|&c| Json::Int(c as i64)).collect()),
            ),
            (
                "held_out",
                Json::List(
                    self.held_out
                        .iter()
                        .map(|c| {
                            Json::object([
                                ("class", Json::Int(c.class as i64)),
                                ("model", Json::Float(c.model)),
                                ("data", Json::Float(c.data)),
                                ("data_se", Json::Float(c.data_se)),
                                ("within", Json::Int(i64::from(c.within))),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

#[cfg(test)]
mod row_members_tests {
    use super::read_members;

    /// A pack without `row_members.arrow` takes pack_data's unchanged path (RFC 0357); checked
    /// bit-for-bit on the full pack against the previous code when this table was added.
    #[test]
    fn a_pack_without_the_table_has_no_members() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_members(dir.path()).unwrap().is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unbalanced_theta_probe_is_past_the_root() {
        let root = 9.04;
        let f = |theta: f64| {
            if theta > 12.0 {
                Err(CalibrationError::Unbalanced("flat".into()))
            } else {
                Ok(0.01 * (root - theta))
            }
        };
        let got = theta_root(f, |_, _| "miss".into()).unwrap();
        assert!((got - root).abs() < 1e-5, "{got}");
    }

    #[test]
    fn an_unbalanced_lower_end_of_the_band_is_an_error() {
        let f = |_: f64| Err(CalibrationError::Unbalanced("flat".into()));
        assert!(theta_root(f, |_, _| "miss".into()).is_err());
    }
}
