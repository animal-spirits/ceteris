//! PPML gravity estimation for the P2 gate's H2 tests (`specs/econ/SPEC.md`, P2 gate): Poisson
//! pseudo-maximum likelihood on country-pair trade with exporter and importer fixed effects, ln
//! population-weighted distance and contiguity as regressors, domestic pairs excluded; and per
//! theta class, ln(1 + tariff) and ln distance (ADR 0018). [`hdfe`] absorbs any number of fixed
//! effects and clusters standard errors, for direct estimation on BACI (ADR 0022).
//!
//! Least squares follow numpy's `lstsq` (minimum norm, singular values at or below
//! eps max(n, k) of the largest dropped) and the pseudo-inverse numpy's `pinv` (1e-15 relative),
//! by Householder QR and one-sided Jacobi SVD with every sum in a fixed order.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::io::{Mat, pairwise_sum};

/// Relative change of the deviance.
pub const TOL: f64 = 1e-10;
pub const MAX_ITER: usize = 200;
/// Weighted norm of a sweep's change of a demeaned column, relative to the column's.
pub const DEMEAN_TOL: f64 = 1e-13;
pub const DEMEAN_MAX_SWEEPS: usize = 10_000;
/// Two fixed effects with at most this many cells start from their direct solution.
const TWO_WAY_CELLS: usize = 4_000_000;

#[derive(Clone, Debug, PartialEq)]
pub enum PpmlError {
    /// Inputs no fit can use.
    Invalid(String),
    /// IRLS or the fixed effects did not converge.
    NoConvergence(String),
}

impl fmt::Display for PpmlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PpmlError::Invalid(s) | PpmlError::NoConvergence(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for PpmlError {}

fn invalid(what: &str) -> PpmlError {
    PpmlError::Invalid(what.into())
}

/// Named regressors, in order.
pub type Regressors<'a> = [(&'a str, &'a [f64])];

/// `fit` plus its separation counts and fit statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct Robust {
    pub names: Vec<String>,
    pub coef: Vec<f64>,
    /// Eicker-Huber-White (HC0).
    pub se: Vec<f64>,
    pub separation: Separation,
    /// Squared correlation of flows and fitted means.
    pub pseudo_r2: f64,
}

impl Robust {
    pub fn coef(&self, name: &str) -> f64 {
        self.coef[position(&self.names, name)]
    }

    pub fn se(&self, name: &str) -> f64 {
        self.se[position(&self.names, name)]
    }
}

fn position(names: &[String], name: &str) -> usize {
    names
        .iter()
        .position(|n| n == name)
        .unwrap_or_else(|| panic!("no regressor {name}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Separation {
    /// Observations removed because their exporter or importer has no positive flow.
    pub n_rows: usize,
    /// Countries that lost all their exporter rows.
    pub n_exporters: usize,
    /// Countries that lost all their importer rows.
    pub n_importers: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Elasticity {
    /// Coefficient on ln distw.
    pub elasticity: f64,
    pub contiguity: f64,
    /// Eicker-Huber-White.
    pub se_elasticity: f64,
    pub se_contiguity: f64,
    /// Ordered non-domestic pairs actually estimated.
    pub n_pairs: usize,
    /// Pairs without a usable distance or contiguity (before estimation).
    pub n_dropped: usize,
    /// Estimable pairs removed because their exporter or importer has no positive flow.
    pub n_separated_rows: usize,
    /// Countries that lost all their exporter rows (zero exports).
    pub n_separated_exporters: usize,
    /// Countries that lost all their importer rows (zero imports).
    pub n_separated_importers: usize,
    /// Squared correlation of flows and fitted means.
    pub pseudo_r2: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TradeElasticity {
    /// Minus the coefficient on ln(1 + tariff).
    pub theta: f64,
    /// Coefficient on ln distance.
    pub distance: f64,
    /// Eicker-Huber-White.
    pub se_theta: f64,
    pub se_distance: f64,
    /// Observations actually estimated.
    pub n_pairs: usize,
    /// Observations removed because their exporter or importer has no positive flow.
    pub n_separated_rows: usize,
    pub pseudo_r2: f64,
}

/// A PPML fit with absorbed fixed effects ([`hdfe`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    pub names: Vec<String>,
    pub coef: Vec<f64>,
    /// Cluster-robust when clusters were given, else Eicker-Huber-White (HC0).
    pub cov: Mat,
    /// Observations estimated.
    pub n: usize,
    /// Observations removed because some fixed-effect group has no positive flow.
    pub n_separated: usize,
    /// Squared correlation of flows and fitted means.
    pub pseudo_r2: f64,
    /// The estimated observations of the input.
    pub keep: Vec<bool>,
}

impl Fit {
    pub fn coef(&self, name: &str) -> f64 {
        self.coef[position(&self.names, name)]
    }

    pub fn se(&self, name: &str) -> f64 {
        let i = position(&self.names, name);
        self.cov[(i, i)].sqrt()
    }
}

/// numpy's `maximum`: NaN wins.
fn np_max(a: f64, b: f64) -> f64 {
    if a.is_nan() { a } else { a.max(b) }
}

fn mean(a: &[f64]) -> f64 {
    pairwise_sum(a) / a.len() as f64
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

fn bincount(index: &[usize], weights: impl IntoIterator<Item = f64>, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (&i, w) in index.iter().zip(weights) {
        out[i] += w;
    }
    out
}

/// `bincount` over the rows of `keep`.
fn bincount_kept(index: &[usize], y: &[f64], keep: &[bool], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for ((&i, &w), &k) in index.iter().zip(y).zip(keep) {
        if k {
            out[i] += w;
        }
    }
    out
}

fn count(mask: &[bool]) -> usize {
    mask.iter().filter(|&&k| k).count()
}

fn select<T: Copy>(v: &[T], keep: &[bool]) -> Vec<T> {
    v.iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(x, _)| *x)
        .collect()
}

/// numpy's `unique(..., return_inverse=True)[1]` and the number of distinct values.
fn codes<T: Ord + Copy>(values: &[T]) -> (Vec<usize>, usize) {
    let sorted: Vec<T> = values
        .iter()
        .copied()
        .collect::<BTreeSet<T>>()
        .into_iter()
        .collect();
    let inverse = values
        .iter()
        .map(|v| sorted.binary_search(v).unwrap())
        .collect();
    (inverse, sorted.len())
}

fn distinct<T: Ord + Copy>(values: impl IntoIterator<Item = T>) -> usize {
    values.into_iter().collect::<BTreeSet<T>>().len()
}

fn deviance(y: &[f64], mu: &[f64]) -> f64 {
    let terms: Vec<f64> = y
        .iter()
        .zip(mu)
        .map(|(&y, &mu)| {
            let ylogy = if y > 0.0 { y * (y / mu).ln() } else { 0.0 };
            ylogy - (y - mu)
        })
        .collect();
    2.0 * pairwise_sum(&terms)
}

/// numpy's `corrcoef(y, mu)[0, 1] ** 2`.
fn pseudo_r2(y: &[f64], mu: &[f64]) -> f64 {
    let (my, mm) = (mean(y), mean(mu));
    let dy: Vec<f64> = y.iter().map(|v| v - my).collect();
    let dm: Vec<f64> = mu.iter().map(|v| v - mm).collect();
    let fact = 1.0 / (y.len() as f64 - 1.0);
    let (c00, c01, c11) = (
        dot(&dy, &dy) * fact,
        dot(&dy, &dm) * fact,
        dot(&dm, &dm) * fact,
    );
    let r = (c01 / c00.sqrt() / c11.sqrt()).clamp(-1.0, 1.0);
    r * r
}

fn validate(y: &[f64], regressors: &Regressors) -> Result<(), PpmlError> {
    if regressors.is_empty() {
        return Err(invalid("no regressors"));
    }
    if regressors.iter().any(|(_, r)| r.len() != y.len()) {
        return Err(invalid("regressors and flows differ in length"));
    }
    let bad_flow = y.iter().any(|&v| v < 0.0 || !v.is_finite());
    let bad_reg = regressors
        .iter()
        .any(|(_, r)| r.iter().any(|v| !v.is_finite()));
    if bad_flow || bad_reg {
        return Err(invalid(
            "flows must be finite and nonnegative, regressors finite",
        ));
    }
    if y.is_empty() {
        return Err(invalid("no observations"));
    }
    Ok(())
}

// ---------------------------------------------------------------- dense linear algebra

/// Householder QR of the columns `a` (each of length n) applied to `b` too: `a` becomes R in
/// its first min(n, k) rows and `b` becomes Q^T b. Returns the number of reflections.
fn householder(a: &mut [Vec<f64>], b: &mut [f64]) -> usize {
    let n = b.len();
    let steps = n.min(a.len());
    for j in 0..steps {
        let mut v = a[j][j..].to_vec();
        let norm = dot(&v, &v).sqrt();
        if norm == 0.0 {
            continue;
        }
        let alpha = if v[0] > 0.0 { -norm } else { norm };
        v[0] -= alpha;
        let vv = dot(&v, &v);
        if vv == 0.0 {
            continue;
        }
        for col in a.iter_mut().skip(j) {
            let t = 2.0 * dot(&v, &col[j..]) / vv;
            for (c, vi) in col[j..].iter_mut().zip(&v) {
                *c -= t * vi;
            }
        }
        let t = 2.0 * dot(&v, &b[j..]) / vv;
        for (c, vi) in b[j..].iter_mut().zip(&v) {
            *c -= t * vi;
        }
    }
    steps
}

/// One-sided Jacobi on the columns `u`: rotates them until orthogonal, applying each rotation to
/// the columns of `v` (the identity to start). The column norms of `u` are then the singular
/// values, `u` holds U times them and `v` the right singular vectors.
fn jacobi(u: &mut [Vec<f64>]) -> Vec<Vec<f64>> {
    let k = u.len();
    let mut v: Vec<Vec<f64>> = (0..k)
        .map(|i| (0..k).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for _ in 0..100 {
        let mut rotated = false;
        for p in 0..k {
            for q in p + 1..k {
                let (alpha, beta, gamma) =
                    (dot(&u[p], &u[p]), dot(&u[q], &u[q]), dot(&u[p], &u[q]));
                if gamma == 0.0 || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for cols in [&mut *u, &mut v] {
                    let (lo, hi) = cols.split_at_mut(q);
                    for (a, b) in lo[p].iter_mut().zip(hi[0].iter_mut()) {
                        let (x, y) = (*a, *b);
                        *a = c * x - s * y;
                        *b = s * x + c * y;
                    }
                }
            }
        }
        if !rotated {
            break;
        }
    }
    v
}

/// numpy's `lstsq(a, b, rcond=None)[0]` for `a` given by its k columns of length n: the
/// minimum-norm least-squares solution, singular values at or below eps max(n, k) of the largest
/// treated as zero.
fn lstsq(columns: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let (n, k) = (b.len(), columns.len());
    let mut r = columns.to_vec();
    let mut qtb = b.to_vec();
    let rows = householder(&mut r, &mut qtb);
    for col in &mut r {
        col.truncate(rows);
    }
    qtb.truncate(rows);
    let v = jacobi(&mut r);
    let sigma: Vec<f64> = r.iter().map(|c| dot(c, c).sqrt()).collect();
    let cutoff = f64::EPSILON * n.max(k) as f64 * sigma.iter().copied().fold(0.0, f64::max);
    let mut x = vec![0.0; k];
    for i in (0..k).filter(|&i| sigma[i] > cutoff) {
        let coef = dot(&r[i], &qtb) / (sigma[i] * sigma[i]);
        for (xk, vk) in x.iter_mut().zip(&v[i]) {
            *xk += coef * vk;
        }
    }
    x
}

/// numpy's `pinv(a)` of a square matrix: singular values at or below 1e-15 of the largest
/// treated as zero.
fn pinv(a: &Mat) -> Mat {
    let k = a.rows;
    let mut u: Vec<Vec<f64>> = (0..k)
        .map(|j| (0..k).map(|i| a[(i, j)]).collect())
        .collect();
    let v = jacobi(&mut u);
    let sigma: Vec<f64> = u.iter().map(|c| dot(c, c).sqrt()).collect();
    let cutoff = 1e-15 * sigma.iter().copied().fold(0.0, f64::max);
    let mut out = Mat::zeros(k, k);
    for s in (0..k).filter(|&s| sigma[s] > cutoff) {
        let scale = 1.0 / (sigma[s] * sigma[s]);
        for i in 0..k {
            let vi = v[s][i] * scale;
            for j in 0..k {
                out[(i, j)] += vi * u[s][j];
            }
        }
    }
    out
}

/// `x.T @ (x * w[:, None])` of the columns `x`.
fn weighted_gram(x: &[Vec<f64>], w: &[f64]) -> Mat {
    let k = x.len();
    let mut out = Mat::zeros(k, k);
    for j in 0..k {
        let xw: Vec<f64> = x[j].iter().zip(w).map(|(x, w)| x * w).collect();
        for i in 0..k {
            out[(i, j)] = dot(&x[i], &xw);
        }
    }
    out
}

/// `bread @ meat @ bread`.
fn sandwich(bread: &Mat, meat: &Mat) -> Mat {
    bread.matmul(meat).matmul(bread)
}

/// Cholesky solve of the symmetric positive definite `a` (row-major, n x n) for the columns of
/// `b`, in place; `None` if a pivot is not positive.
fn cholesky_solve(a: &mut [f64], n: usize, b: &mut [Vec<f64>]) -> Option<()> {
    for j in 0..n {
        let mut d = a[j * n + j];
        for k in 0..j {
            d -= a[j * n + k] * a[j * n + k];
        }
        if d.is_nan() || d <= 0.0 {
            return None;
        }
        let d = d.sqrt();
        a[j * n + j] = d;
        for i in j + 1..n {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= a[i * n + k] * a[j * n + k];
            }
            a[i * n + j] = s / d;
        }
    }
    for col in b {
        for i in 0..n {
            let mut s = col[i];
            for k in 0..i {
                s -= a[i * n + k] * col[k];
            }
            col[i] = s / a[i * n + i];
        }
        for i in (0..n).rev() {
            let mut s = col[i];
            for k in i + 1..n {
                s -= a[k * n + i] * col[k];
            }
            col[i] = s / a[i * n + i];
        }
    }
    Some(())
}

// ---------------------------------------------------------------- dummy-variable fit

/// Observations kept: those whose exporter and importer have some positive flow. A country with
/// none has a fixed effect at minus infinity (perfect prediction of zeros), so its rows carry no
/// information; dropping one can empty another, hence the loop.
fn active(y: &[f64], exporter: &[usize], importer: &[usize]) -> Vec<bool> {
    let ne = exporter.iter().max().map_or(0, |m| m + 1);
    let ni = importer.iter().max().map_or(0, |m| m + 1);
    let mut keep = vec![true; y.len()];
    loop {
        let out = bincount_kept(exporter, y, &keep, ne);
        let inn = bincount_kept(importer, y, &keep, ni);
        let now: Vec<bool> = (0..y.len())
            .map(|i| keep[i] && out[exporter[i]] > 0.0 && inn[importer[i]] > 0.0)
            .collect();
        if count(&now) == count(&keep) {
            return keep;
        }
        keep = now;
    }
}

/// Regressors first, then exporter dummies, then importer dummies without the first
/// (identification).
fn design(exporter: &[usize], importer: &[usize], regressors: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let (e, ne) = codes(exporter);
    let (m, nm) = codes(importer);
    let n = e.len();
    let mut x = regressors;
    let k = x.len();
    x.extend((0..ne + nm - 1).map(|_| vec![0.0; n]));
    for i in 0..n {
        x[k + e[i]][i] = 1.0;
        if m[i] > 0 {
            x[k + ne - 1 + m[i]][i] = 1.0;
        }
    }
    x
}

fn matvec(x: &[Vec<f64>], beta: &[f64], n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let mut s = 0.0;
            for (col, b) in x.iter().zip(beta) {
                s += col[i] * b;
            }
            s
        })
        .collect()
}

/// Poisson IRLS with step halving: the coefficients and fitted means.
fn irls(y: &[f64], x: &[Vec<f64>]) -> Result<(Vec<f64>, Vec<f64>), PpmlError> {
    let n = y.len();
    let my = mean(y);
    let mut mu: Vec<f64> = y.iter().map(|y| (y + my) / 2.0).collect();
    let mut eta: Vec<f64> = mu.iter().map(|m| m.ln()).collect();
    let mut beta = vec![0.0; x.len()];
    let mut dev = deviance(y, &mu);
    for _ in 0..MAX_ITER {
        let sw: Vec<f64> = mu.iter().map(|m| m.sqrt()).collect();
        let cols: Vec<Vec<f64>> = x
            .iter()
            .map(|c| c.iter().zip(&sw).map(|(c, s)| c * s).collect())
            .collect();
        let rhs: Vec<f64> = (0..n)
            .map(|i| sw[i] * (eta[i] + (y[i] - mu[i]) / mu[i]))
            .collect();
        let step = lstsq(&cols, &rhs);
        let mut t = 1.0;
        let (cand, eta_c, mu_c, dev_c) = loop {
            let cand: Vec<f64> = beta
                .iter()
                .zip(&step)
                .map(|(b, s)| (1.0 - t) * b + t * s)
                .collect();
            let eta_c: Vec<f64> = matvec(x, &cand, n)
                .into_iter()
                .map(|e| e.clamp(-50.0, 50.0))
                .collect();
            let mu_c: Vec<f64> = eta_c.iter().map(|e| e.exp()).collect();
            let dev_c = deviance(y, &mu_c);
            if dev_c <= dev || t < 1e-8 {
                break (cand, eta_c, mu_c, dev_c);
            }
            t /= 2.0;
        };
        let converged = (dev - dev_c).abs() <= TOL * (dev_c.abs() + 0.1);
        (beta, eta, mu, dev) = (cand, eta_c, mu_c, dev_c);
        if converged {
            return Ok((beta, mu));
        }
    }
    Err(PpmlError::NoConvergence(format!(
        "PPML did not converge in {MAX_ITER} iterations"
    )))
}

/// PPML of `regressors` with exporter and importer fixed effects as dummies, with HC0 standard
/// errors, the separation counts and the pseudo-R2. `exporter` and `importer` are country indices
/// per observation; zero flows are kept.
pub fn fit_robust(
    flows: &[f64],
    exporter: &[usize],
    importer: &[usize],
    regressors: &Regressors,
) -> Result<Robust, PpmlError> {
    validate(flows, regressors)?;
    if exporter.len() != flows.len() || importer.len() != flows.len() {
        return Err(invalid("exporter, importer and flows differ in length"));
    }
    let keep = active(flows, exporter, importer);
    if !keep.contains(&true) {
        return Err(invalid(
            "no observation survives: every exporter or importer has zero total flow",
        ));
    }
    let separation = Separation {
        n_rows: keep.len() - count(&keep),
        n_exporters: distinct(exporter.iter().copied()) - distinct(select(exporter, &keep)),
        n_importers: distinct(importer.iter().copied()) - distinct(select(importer, &keep)),
    };
    let kept = select(flows, &keep);
    // Slopes are invariant to the scale; it only shifts the fixed effects.
    let scale = mean(&kept);
    let y: Vec<f64> = kept.iter().map(|v| v / scale).collect();
    let r: Vec<Vec<f64>> = regressors.iter().map(|(_, r)| select(r, &keep)).collect();
    let x = design(&select(exporter, &keep), &select(importer, &keep), r);
    let (beta, mu) = irls(&y, &x)?;
    let bread = pinv(&weighted_gram(&x, &mu));
    let sq: Vec<f64> = y.iter().zip(&mu).map(|(y, m)| (y - m) * (y - m)).collect();
    let cov = sandwich(&bread, &weighted_gram(&x, &sq));
    let k = regressors.len();
    Ok(Robust {
        names: regressors.iter().map(|(n, _)| (*n).to_owned()).collect(),
        coef: beta[..k].to_vec(),
        se: (0..k).map(|i| cov[(i, i)].sqrt()).collect(),
        separation,
        pseudo_r2: pseudo_r2(&y, &mu),
    })
}

/// PPML coefficients of `regressors` with exporter and importer fixed effects ([`fit_robust`]).
pub fn fit(
    flows: &[f64],
    exporter: &[usize],
    importer: &[usize],
    regressors: &Regressors,
) -> Result<Vec<f64>, PpmlError> {
    fit_robust(flows, exporter, importer, regressors).map(|r| r.coef)
}

/// The run's country-pair flows (`run_flows`): sparse, absent pairs are zero flows.
pub struct RunFlows<'a> {
    pub exporter_country: &'a [usize],
    pub importer_country: &'a [usize],
    pub value: &'a [f64],
}

/// The CEPII gravity table.
pub struct GravityTable<'a> {
    pub iso3_o: &'a [&'a str],
    pub iso3_d: &'a [&'a str],
    pub distw: &'a [f64],
    pub contig: &'a [f64],
}

/// H2: PPML of the pair value (summed over goods) on ln `distw` and `contig`; `iso3[c]` is the
/// code of country index c. Domestic pairs are excluded; pairs missing from the gravity table or
/// with a non-finite or nonpositive `distw` or a non-finite `contig` are dropped and counted.
pub fn distance_elasticity(
    run: &RunFlows,
    gravity: &GravityTable,
    iso3: &[&str],
) -> Result<Elasticity, PpmlError> {
    let n = iso3.len();
    if run
        .exporter_country
        .iter()
        .chain(run.importer_country)
        .any(|&c| c >= n)
    {
        return Err(invalid("run flows name a country outside iso3"));
    }
    let mut pair_value = vec![0.0; n * n];
    for ((&o, &d), &v) in run
        .exporter_country
        .iter()
        .zip(run.importer_country)
        .zip(run.value)
    {
        if o != d {
            pair_value[o * n + d] += v;
        }
    }
    let mut lookup = BTreeMap::new();
    for i in 0..gravity.iso3_o.len() {
        lookup.insert(
            (gravity.iso3_o[i], gravity.iso3_d[i]),
            (gravity.distw[i], gravity.contig[i]),
        );
    }
    let (mut o_idx, mut d_idx, mut dist, mut contig) = (vec![], vec![], vec![], vec![]);
    let mut n_dropped = 0;
    for o in 0..n {
        for d in (0..n).filter(|&d| d != o) {
            let (dw, c) = lookup
                .get(&(iso3[o], iso3[d]))
                .copied()
                .unwrap_or((f64::NAN, f64::NAN));
            if dw.is_finite() && dw > 0.0 && c.is_finite() {
                o_idx.push(o);
                d_idx.push(d);
                dist.push(dw.ln());
                contig.push(c);
            } else {
                n_dropped += 1;
            }
        }
    }
    let value: Vec<f64> = o_idx
        .iter()
        .zip(&d_idx)
        .map(|(&o, &d)| pair_value[o * n + d])
        .collect();
    let r = fit_robust(
        &value,
        &o_idx,
        &d_idx,
        &[("ln_distw", &dist), ("contig", &contig)],
    )?;
    Ok(Elasticity {
        elasticity: r.coef[0],
        contiguity: r.coef[1],
        se_elasticity: r.se[0],
        se_contiguity: r.se[1],
        n_pairs: value.len() - r.separation.n_rows,
        n_dropped,
        n_separated_rows: r.separation.n_rows,
        n_separated_exporters: r.separation.n_exporters,
        n_separated_importers: r.separation.n_importers,
        pseudo_r2: r.pseudo_r2,
    })
}

/// H2 per theta class (ADR 0018): PPML of `flows` on ln(1 + `tariff`) and ln `distance` with
/// exporter and importer fixed effects; the coefficient on ln(1 + t) is -theta. One observation
/// per pair (zero flows kept); domestic pairs are the caller's to leave out.
pub fn trade_elasticity(
    flows: &[f64],
    exporter: &[usize],
    importer: &[usize],
    tariff: &[f64],
    distance: &[f64],
) -> Result<TradeElasticity, PpmlError> {
    let ln_tariff: Vec<f64> = tariff.iter().map(|t| t.ln_1p()).collect();
    let ln_dist: Vec<f64> = distance.iter().map(|d| d.ln()).collect();
    let r = fit_robust(
        flows,
        exporter,
        importer,
        &[("ln_tariff", &ln_tariff), ("ln_dist", &ln_dist)],
    )?;
    Ok(TradeElasticity {
        theta: -r.coef[0],
        distance: r.coef[1],
        se_theta: r.se[0],
        se_distance: r.se[1],
        n_pairs: flows.len() - r.separation.n_rows,
        n_separated_rows: r.separation.n_rows,
        pseudo_r2: r.pseudo_r2,
    })
}

// ---------------------------------------------------------------- absorbed fixed effects

/// Observations kept: those in no fixed-effect group whose flows are all zero (such a group's
/// effect is minus infinity and its rows carry no information); dropping rows can empty another
/// group.
fn separated(y: &[f64], groups: &[(Vec<usize>, usize)]) -> Vec<bool> {
    let mut keep = vec![true; y.len()];
    loop {
        let mut now = keep.clone();
        for (g, size) in groups {
            let total = bincount_kept(g, y, &now, *size);
            for (k, &gi) in now.iter_mut().zip(g) {
                *k &= total[gi] > 0.0;
            }
        }
        if count(&now) == count(&keep) {
            return keep;
        }
        keep = now;
    }
}

/// Weighted residuals of the columns `v` on two fixed effects solved directly: the first is
/// eliminated from the normal equations, the second solves the Schur complement, singular along
/// each connected component's constant. Each component is solved by Cholesky with its first
/// group's effect pinned at 0 and then centred, which is the minimum-norm solution numpy's
/// `lstsq` gives. `None` if a pivot is not positive.
fn two_way(
    v: &[Vec<f64>],
    w: &[f64],
    a: &[usize],
    b: &[usize],
    na: usize,
    nb: usize,
) -> Option<Vec<Vec<f64>>> {
    let mut m = vec![0.0; na * nb];
    for ((&ai, &bi), &wi) in a.iter().zip(b).zip(w) {
        m[ai * nb + bi] += wi;
    }
    let da: Vec<f64> = (0..na)
        .map(|r| pairwise_sum(&m[r * nb..(r + 1) * nb]))
        .collect();
    let mut db = vec![0.0; nb];
    for r in 0..na {
        for (d, x) in db.iter_mut().zip(&m[r * nb..(r + 1) * nb]) {
            *d += x;
        }
    }
    let ra: Vec<Vec<f64>> = v
        .iter()
        .map(|c| bincount(a, w.iter().zip(c).map(|(w, c)| w * c), na))
        .collect();
    let rb: Vec<Vec<f64>> = v
        .iter()
        .map(|c| bincount(b, w.iter().zip(c).map(|(w, c)| w * c), nb))
        .collect();
    // The second-effect groups each first-effect group meets, and the reverse.
    let cols_of_a: Vec<Vec<usize>> = (0..na)
        .map(|r| (0..nb).filter(|&c| m[r * nb + c] != 0.0).collect())
        .collect();

    // Connected components of the second effect through the first.
    let mut visited = vec![false; nb];
    let mut members: Vec<Vec<usize>> = Vec::new();
    let mut rows_of_b: Vec<Vec<usize>> = vec![Vec::new(); nb];
    for (r, cols) in cols_of_a.iter().enumerate() {
        for &c in cols {
            rows_of_b[c].push(r);
        }
    }
    let mut seen_a = vec![false; na];
    for start in 0..nb {
        if visited[start] {
            continue;
        }
        let mut stack = vec![start];
        visited[start] = true;
        let mut list = Vec::new();
        while let Some(c) = stack.pop() {
            list.push(c);
            for &r in &rows_of_b[c] {
                if seen_a[r] {
                    continue;
                }
                seen_a[r] = true;
                for &c2 in &cols_of_a[r] {
                    if !visited[c2] {
                        visited[c2] = true;
                        stack.push(c2);
                    }
                }
            }
        }
        list.sort_unstable();
        members.push(list);
    }

    let mut gamma: Vec<Vec<f64>> = vec![vec![0.0; nb]; v.len()];
    let mut local = vec![usize::MAX; nb];
    for list in &members {
        let size = list.len() - 1;
        if size == 0 {
            continue;
        }
        for (i, &c) in list[1..].iter().enumerate() {
            local[c] = i;
        }
        let mut schur = vec![0.0; size * size];
        for (i, &c) in list[1..].iter().enumerate() {
            schur[i * size + i] = db[c];
        }
        let mut rhs: Vec<Vec<f64>> = rb
            .iter()
            .map(|r| list[1..].iter().map(|&c| r[c]).collect())
            .collect();
        let mut rows_of: BTreeSet<usize> = BTreeSet::new();
        for &c in list {
            rows_of.extend(&rows_of_b[c]);
        }
        for r in rows_of {
            for &c1 in cols_of_a[r].iter().filter(|&&c| local[c] != usize::MAX) {
                let scaled1 = m[r * nb + c1] / da[r];
                for (j, rj) in rhs.iter_mut().enumerate() {
                    rj[local[c1]] -= scaled1 * ra[j][r];
                }
                for &c2 in cols_of_a[r].iter().filter(|&&c| local[c] != usize::MAX) {
                    schur[local[c2] * size + local[c1]] -= m[r * nb + c2] * scaled1;
                }
            }
        }
        cholesky_solve(&mut schur, size, &mut rhs)?;
        for (g, sol) in gamma.iter_mut().zip(&rhs) {
            let centre = sol.iter().sum::<f64>() / list.len() as f64;
            g[list[0]] = -centre;
            for (&c, s) in list[1..].iter().zip(sol) {
                g[c] = s - centre;
            }
        }
        for &c in &list[1..] {
            local[c] = usize::MAX;
        }
    }

    let out = v
        .iter()
        .zip(&ra)
        .zip(&gamma)
        .map(|((col, ra), g)| {
            let alpha: Vec<f64> = (0..na)
                .map(|r| {
                    let mut s = ra[r];
                    for &c in &cols_of_a[r] {
                        s -= m[r * nb + c] * g[c];
                    }
                    s / da[r]
                })
                .collect();
            col.iter()
                .zip(a)
                .zip(b)
                .map(|((x, &ai), &bi)| x - alpha[ai] - g[bi])
                .collect()
        })
        .collect();
    Some(out)
}

/// Weighted residuals of the columns `v` on the fixed effects `groups`, by alternating
/// projections (the within transformation of each fixed effect in turn until nothing moves). Two
/// small fixed effects start from their direct solution, which the projections then only polish.
fn demean(
    v: &[Vec<f64>],
    w: &[f64],
    groups: &[(Vec<usize>, usize)],
) -> Result<Vec<Vec<f64>>, PpmlError> {
    let mut v = match groups {
        [(a, na), (b, nb)] if na * nb <= TWO_WAY_CELLS => {
            two_way(v, w, a, b, *na, *nb).unwrap_or_else(|| v.to_vec())
        }
        _ => v.to_vec(),
    };
    let weight: Vec<Vec<f64>> = groups
        .iter()
        .map(|(g, s)| bincount(g, w.iter().copied(), *s))
        .collect();
    for _ in 0..DEMEAN_MAX_SWEEPS {
        let before = v.clone();
        for ((g, s), wg) in groups.iter().zip(&weight) {
            for col in &mut v {
                let sums = bincount(g, w.iter().zip(col.iter()).map(|(w, c)| w * c), *s);
                let means: Vec<f64> = sums.iter().zip(wg).map(|(s, w)| s / w).collect();
                for (c, &gi) in col.iter_mut().zip(g) {
                    *c -= means[gi];
                }
            }
        }
        // Rows of negligible weight converge slowly but do not move the weighted least squares
        // step.
        let settled = v.iter().zip(&before).all(|(col, old)| {
            let mut moved = 0.0;
            let mut size = 0.0;
            for ((c, o), w) in col.iter().zip(old).zip(w) {
                moved += w * ((c - o) * (c - o));
                size += w * (c * c);
            }
            moved.sqrt() <= DEMEAN_TOL * np_max(size.sqrt(), 1e-300)
        });
        if settled {
            return Ok(v);
        }
    }
    Err(PpmlError::NoConvergence(format!(
        "fixed effects did not converge in {DEMEAN_MAX_SWEEPS} sweeps"
    )))
}

/// PPML of `flows` on `regressors` with any number of absorbed fixed effects (integer group codes
/// per observation), as in Correia, Guimaraes and Zylkin (2020): IRLS whose weighted least
/// squares step runs on the regressors and working variable demeaned by the fixed effects.
/// Standard errors are clustered on `cluster` (integer codes) with the G / (G - 1) correction,
/// else HC0. Zero flows are kept; rows of fixed-effect groups without any positive flow are
/// dropped first.
pub fn hdfe(
    flows: &[f64],
    fixed_effects: &[&[i64]],
    regressors: &Regressors,
    cluster: Option<&[i64]>,
) -> Result<Fit, PpmlError> {
    validate(flows, regressors)?;
    let lengths = fixed_effects.iter().copied().chain(cluster);
    if lengths.into_iter().any(|g| g.len() != flows.len()) {
        return Err(invalid(
            "fixed effects, clusters and flows differ in length",
        ));
    }
    let all: Vec<(Vec<usize>, usize)> = fixed_effects.iter().map(|g| codes(g)).collect();
    let keep = separated(flows, &all);
    if !keep.contains(&true) {
        return Err(invalid(
            "no observation survives: every fixed-effect group has zero total flow",
        ));
    }
    let y = select(flows, &keep);
    let x: Vec<Vec<f64>> = regressors.iter().map(|(_, r)| select(r, &keep)).collect();
    let groups: Vec<(Vec<usize>, usize)> =
        all.iter().map(|(g, _)| codes(&select(g, &keep))).collect();
    let my = mean(&y);
    let y: Vec<f64> = y.iter().map(|v| v / my).collect();
    let n = y.len();
    let k = x.len();

    let my = mean(&y);
    let mut mu: Vec<f64> = y.iter().map(|y| (y + my) / 2.0).collect();
    let mut eta: Vec<f64> = mu.iter().map(|m| m.ln()).collect();
    let mut dev = deviance(&y, &mu);
    let mut beta = Vec::new();
    let mut converged = false;
    for _ in 0..MAX_ITER {
        let z: Vec<f64> = (0..n).map(|i| eta[i] + (y[i] - mu[i]) / mu[i]).collect();
        let mut both = x.clone();
        both.push(z.clone());
        let mut both = demean(&both, &mu, &groups)?;
        let zt = both.pop().unwrap();
        let xt = both;
        let sw: Vec<f64> = mu.iter().map(|m| m.sqrt()).collect();
        let cols: Vec<Vec<f64>> = xt
            .iter()
            .map(|c| c.iter().zip(&sw).map(|(c, s)| c * s).collect())
            .collect();
        let rhs: Vec<f64> = zt.iter().zip(&sw).map(|(z, s)| z * s).collect();
        beta = lstsq(&cols, &rhs);
        let fitted = matvec(&xt, &beta, n);
        eta = (0..n)
            .map(|i| (z[i] - (zt[i] - fitted[i])).clamp(-700.0, 700.0))
            .collect();
        mu = eta.iter().map(|e| e.exp()).collect();
        let dev_new = deviance(&y, &mu);
        converged = (dev - dev_new).abs() <= TOL * (dev_new.abs() + 0.1);
        dev = dev_new;
        if converged {
            break;
        }
    }
    if !converged {
        return Err(PpmlError::NoConvergence(format!(
            "PPML did not converge in {MAX_ITER} iterations"
        )));
    }

    let xt = demean(&x, &mu, &groups)?;
    let bread = pinv(&weighted_gram(&xt, &mu));
    let resid: Vec<f64> = y.iter().zip(&mu).map(|(y, m)| y - m).collect();
    let score: Vec<Vec<f64>> = xt
        .iter()
        .map(|c| c.iter().zip(&resid).map(|(x, r)| x * r).collect())
        .collect();
    let meat = match cluster {
        None => weighted_gram(&score, &vec![1.0; n]),
        Some(c) => {
            let (c, g) = codes(&select(c, &keep));
            let sums: Vec<Vec<f64>> = score
                .iter()
                .map(|s| bincount(&c, s.iter().copied(), g))
                .collect();
            let factor = g as f64 / (g as f64 - 1.0);
            let mut meat = Mat::zeros(k, k);
            for i in 0..k {
                for j in 0..k {
                    meat[(i, j)] = dot(&sums[i], &sums[j]) * factor;
                }
            }
            meat
        }
    };
    Ok(Fit {
        names: regressors.iter().map(|(n, _)| (*n).to_owned()).collect(),
        coef: beta,
        cov: sandwich(&bread, &meat),
        n,
        n_separated: keep.len() - n,
        pseudo_r2: pseudo_r2(&y, &mu),
        keep,
    })
}
