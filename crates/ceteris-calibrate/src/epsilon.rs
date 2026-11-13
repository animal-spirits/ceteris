//! The dark distance cost on the crossings (ADR 0272, P158 epsilon): every crossing of good g
//! between countries A and B takes an iceberg factor `ln D = (epsilon / theta_g) (ln d_AB - ln
//! d_0c)` for its theta class c, and epsilon is found by Brent's method so that the model's
//! pooled PPML distance coefficient on distw equals the data's, each trial with the border
//! friction nu refitted to the ICIO shares ([`border::fit_border_nu`]).
//!
//! The model's coefficient is read at the data state: stage 2 at the stage 1 spending of
//! [`steady::data_state_spending`], where the border fit itself is evaluated, not at the
//! default run's last ticks that H2 reads.

use ceteris_reference::io::Inputs;
use ceteris_reference::ppml;

use crate::border::{self, Brent, Fit};
use crate::steady::{self, CalibrationError, Limits};

/// The bracket of epsilon searched; ADR 0272 expects about 0.35.
pub const EPSILON_MAX: f64 = 2.0;
/// Brent ends when the bracket is this narrow in epsilon.
pub const EPSILON_TOL: f64 = 1e-3;
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
/// `ln_d0` per theta class, `ln_d` the country-pair distance row-major. K scales by `D^-theta`,
/// the share arriving by `1 / D`, and the base prices and selection constants are recomputed. The
/// inputs must not carry a distance cost already.
pub fn at_epsilon(
    inp: &Inputs,
    theta_class: &[usize],
    ln_d: &[f64],
    ln_d0: &[f64],
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
        let ln_dd = ln_factor(inp, epsilon, ln_d[a * n_c + b] - ln_d0[theta_class[g]], g);
        cro.k[j] *= libm::exp(-inp.theta[g] * ln_dd);
        cro.inv_loss[j] *= libm::exp(-ln_dd);
    }
    border::reprice(&mut out)?;
    Ok(out)
}

/// The classes where some crossing's `(1 + nu) D` is below 1 (ADR 0272, Decision 2), for the
/// friction `nu` per good of `at` as it was fitted.
fn product_misses(
    inp: &Inputs,
    theta_class: &[usize],
    ln_d: &[f64],
    ln_d0: &[f64],
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
        let ln_dd = ln_factor(inp, epsilon, ln_d[a * n_c + b] - ln_d0[c], g);
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
    pub fit: Fit,
    /// `ln d_0` per theta class as used.
    pub ln_d0: Vec<f64>,
    /// Classes whose `d_0` went to the smallest linked distance for `(1 + nu) D >= 1`.
    pub floored: Vec<usize>,
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

fn sums(values: &[f64], n_pairs: usize, stride: usize, goods: &[usize]) -> Vec<f64> {
    (0..n_pairs)
        .map(|k| goods.iter().map(|&g| values[k * stride + g]).sum())
        .collect()
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
    let mut ln_d0 = mean;
    let mut floored: Vec<usize> = vec![];
    loop {
        let at = at_epsilon(inp, t.theta_class, t.ln_d, &ln_d0, epsilon)?;
        let fit = border::fit_border_nu(&at, &groups, t.nu_target, t.icio, limits, progress)?;
        let nu = fit.nu_of_goods(&at.border_nu);
        let bad = product_misses(inp, t.theta_class, t.ln_d, &ln_d0, epsilon, &nu);
        if bad.is_empty() {
            let (spend, supply) = steady::data_state_spending(&at, limits)?;
            let pairs = border::data_state_pair_values(&at, &nu, &spend, &supply, limits)?;
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
                fit,
                ln_d0,
                floored,
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
    let mut run = |eps: f64| -> Result<Option<Trial>, CalibrationError> {
        evaluations += 1;
        match trial(inp, t, eps, limits, progress) {
            Ok(r) => {
                progress(&format!(
                    "epsilon {eps}: pooled distance coefficient {} against the data's {data_pooled}",
                    r.pooled
                ));
                Ok(Some(r))
            }
            Err(CalibrationError::Failed(e)) if e.contains("outside the model's") => {
                progress(&format!(
                    "epsilon {eps}: no nu reaches the target shares: {e}"
                ));
                Ok(None)
            }
            Err(e) => Err(e),
        }
    };
    let f_of = |r: &Option<Trial>| {
        r.as_ref()
            .map_or(data_pooled.abs(), |r| r.pooled - data_pooled)
    };
    let zero = run(0.0)?;
    let f_lo = f_of(&zero);
    let (best, at_zero) = if f_lo <= 0.0 {
        (zero.expect("a trial at f <= 0"), true)
    } else {
        let top = run(EPSILON_MAX)?;
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
            let r = run(x)?;
            brent.update(f_of(&r));
            if r.is_some() {
                last = r;
            }
        }
        let root = brent.root();
        if last.as_ref().map(|r| r.epsilon) != Some(root) {
            last = run(root)?;
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
