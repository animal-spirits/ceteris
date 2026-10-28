//! Equilibrium of the reference model: Newton-Krylov from the data state (restarted GMRES on
//! finite-difference Jacobian products, steps capped), Anderson-accelerated tatonnement as the
//! fallback.

use std::fmt;
use std::time::Instant;

use crate::io::Mat;
use crate::model::{self, Model, max_abs};

/// Max-norm residual of the reference solve.
pub const TOL: f64 = 1e-10;
pub const MAX_NEWTON: usize = 200;
/// Krylov vectors of each GMRES cycle.
pub const RESTART: usize = 100;
/// GMRES cycles of one Newton step.
pub const MAX_CYCLES: usize = 1;
/// Largest change of any ln p or ln w in one Newton step. Full steps from the data state overshoot
/// into regions where whole sectors shut down and the Jacobian loses rank.
pub const STEP_CAP: f64 = 0.3;
pub const MAX_TATONNEMENT: usize = 5000;
pub const TATONNEMENT_STEP: f64 = 0.2;
/// As the price rule's c_max.
pub const TATONNEMENT_CLIP: f64 = 0.1;
pub const TATONNEMENT_TOL: f64 = 1e-6;
pub const ANDERSON_MEMORY: usize = 5;

#[derive(Clone, Debug, PartialEq)]
pub enum SolveError {
    /// The solve did not reach its tolerance (also past the deadline).
    NoConvergence(String),
    /// A root that is not a Rust steady state.
    NotSteady(String),
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SolveError::NoConvergence(s) | SolveError::NotSteady(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for SolveError {}

fn no_convergence(what: impl Into<String>) -> SolveError {
    SolveError::NoConvergence(what.into())
}

/// Iteration caps of the two solvers.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_newton: usize,
    pub max_tatonnement: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_newton: MAX_NEWTON,
            max_tatonnement: MAX_TATONNEMENT,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    /// "newton_krylov", or "tatonnement, newton_krylov" after the fallback.
    pub method: String,
    /// Newton steps of the final Newton run.
    pub iterations: usize,
    pub residual: f64,
    pub dropped_province: usize,
    /// The dropped labour equation's excess demand.
    pub dropped_residual: f64,
    pub seconds: f64,
}

/// The reference steady state. (P, G) matrices; country pairs flat (C, C, G).
#[derive(Clone, Debug)]
pub struct Equilibrium {
    pub x: Vec<f64>,
    /// Posted price of active cells, 0 elsewhere.
    pub price: Mat,
    pub wage: Vec<f64>,
    /// (E,)
    pub employment: Vec<f64>,
    /// (E,)
    pub output: Vec<f64>,
    /// Traded output of each origin cell.
    pub supply: Mat,
    /// q_d of traded goods, the local price of local goods.
    pub consumer_price: Mat,
    /// Destination spending on traded goods, tariff included.
    pub expenditure: Mat,
    /// Home purchases at the province's price.
    pub home_value: Mat,
    pub home_units: Mat,
    /// Origin value of shipped units, exporter x importer.
    pub flows_by_country_pair_good: Vec<f64>,
    /// Origin units at dispatch.
    pub units_by_country_pair_good: Vec<f64>,
    /// (P,) household income.
    pub income: Vec<f64>,
    pub price_index: f64,
    pub info: Info,
}

/// The equilibrium quantities at `x`.
pub fn unpack(x: &[f64], m: &Model, info: Info) -> Equilibrium {
    let s = model::evaluate(x, m, &m.inp.ent_a);
    let f = s.flows;
    let home_value = Mat::from_fn(s.price.rows, s.price.cols, |p, g| {
        s.price[(p, g)] * f.home[(p, g)]
    });
    Equilibrium {
        x: x.to_vec(),
        price: s.price,
        wage: s.wage,
        employment: s.employment,
        output: s.output,
        supply: s.supply,
        consumer_price: s.consumer_price,
        expenditure: s.spend,
        home_value,
        home_units: f.home,
        flows_by_country_pair_good: f.pair_value,
        units_by_country_pair_good: f.pair_units,
        income: s.income,
        price_index: s.index,
        info,
    }
}

fn check(deadline: Option<Instant>, what: impl FnOnce() -> String) -> Result<(), SolveError> {
    match deadline {
        Some(d) if Instant::now() > d => Err(no_convergence(format!(
            "{}: wall-clock budget spent",
            what()
        ))),
        _ => Ok(()),
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Jacobian of the residual at `x0` by forward differences along each direction, the step
/// relative to the size of x and of the residual (as scipy's `KrylovJacobian`).
struct Jacobian<'m, 'a> {
    m: &'m Model<'a>,
    x0: Vec<f64>,
    f0: Vec<f64>,
    omega: f64,
}

impl<'m, 'a> Jacobian<'m, 'a> {
    fn new(m: &'m Model<'a>, x0: Vec<f64>, f0: Vec<f64>) -> Self {
        let omega = f64::EPSILON.sqrt() * max_abs(&x0).max(1.0) / max_abs(&f0).max(1.0);
        Jacobian { m, x0, f0, omega }
    }

    fn apply(&self, v: &[f64]) -> Result<Vec<f64>, String> {
        let nv = norm(v);
        if nv == 0.0 {
            return Ok(vec![0.0; v.len()]);
        }
        let sc = self.omega / nv;
        let x: Vec<f64> = self.x0.iter().zip(v).map(|(x, v)| x + sc * v).collect();
        let r: Vec<f64> = model::residual(&x, self.m)
            .iter()
            .zip(&self.f0)
            .map(|(f, f0)| (f - f0) / sc)
            .collect();
        if r.iter().all(|r| r.is_finite()) {
            Ok(r)
        } else {
            Err("Jacobian product not finite".into())
        }
    }
}

/// A linear map applied to a vector, or why it could not be.
type Operator<'o> = dyn FnMut(&[f64]) -> Result<Vec<f64>, String> + 'o;

/// Restarted GMRES from 0 for `a x = b`, to `||b - a x|| <= rtol ||b||` or [`MAX_CYCLES`] cycles
/// of [`RESTART`] Arnoldi vectors (modified Gram-Schmidt, Givens rotations); sums in a fixed
/// order.
fn gmres(
    a: &mut Operator,
    b: &[f64],
    rtol: f64,
) -> Result<Vec<f64>, String> {
    let n = b.len();
    let mut x = vec![0.0; n];
    let target = rtol * norm(b);
    let mut r = b.to_vec();
    let mut beta = norm(&r);
    for cycle in 0..MAX_CYCLES {
        if beta == 0.0 || beta <= target {
            break;
        }
        let mut basis = vec![r.iter().map(|v| v / beta).collect::<Vec<f64>>()];
        let mut hess: Vec<Vec<f64>> = Vec::new();
        let (mut cs, mut sn): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        let mut g = vec![beta];
        for j in 0..RESTART {
            let mut w = a(&basis[j])?;
            let mut h = vec![0.0; j + 2];
            for (i, v) in basis.iter().enumerate() {
                h[i] = dot(&w, v);
                for (wk, vk) in w.iter_mut().zip(v) {
                    *wk -= h[i] * vk;
                }
            }
            let next = norm(&w);
            h[j + 1] = next;
            for i in 0..j {
                let t = cs[i] * h[i] + sn[i] * h[i + 1];
                h[i + 1] = -sn[i] * h[i] + cs[i] * h[i + 1];
                h[i] = t;
            }
            let rr = h[j].hypot(h[j + 1]);
            let (c, s) = if rr == 0.0 {
                (1.0, 0.0)
            } else {
                (h[j] / rr, h[j + 1] / rr)
            };
            h[j] = rr;
            h[j + 1] = 0.0;
            cs.push(c);
            sn.push(s);
            g.push(-s * g[j]);
            g[j] *= c;
            hess.push(h);
            if g[j + 1].abs() <= target || next == 0.0 {
                break;
            }
            basis.push(w.iter().map(|v| v / next).collect());
        }
        let k = hess.len();
        let mut y = vec![0.0; k];
        for i in (0..k).rev() {
            let mut v = g[i];
            for (l, yl) in y.iter().enumerate().skip(i + 1) {
                v -= hess[l][i] * yl;
            }
            y[i] = v / hess[i][i];
        }
        for (v, yi) in basis.iter().zip(&y) {
            for (xk, vk) in x.iter_mut().zip(v) {
                *xk += yi * vk;
            }
        }
        if g[k].abs() <= target || cycle + 1 == MAX_CYCLES {
            break;
        }
        r = b.iter().zip(a(&x)?).map(|(b, ax)| b - ax).collect();
        beta = norm(&r);
    }
    Ok(x)
}

/// Newton-Krylov with steps capped at [`STEP_CAP`]: the number of steps to `tol` in the max
/// norm. `progress` gets one line per step.
pub fn newton(
    m: &Model,
    x0: &[f64],
    tol: f64,
    limits: Limits,
    deadline: Option<Instant>,
    progress: &mut dyn FnMut(&str),
) -> Result<(Vec<f64>, usize), SolveError> {
    let mut x = x0.to_vec();
    let mut fx = model::residual(&x, m);
    for steps in 0..limits.max_newton {
        let norm = max_abs(&fx);
        if !norm.is_finite() {
            return Err(no_convergence(format!(
                "residual not finite after {steps} Newton steps"
            )));
        }
        progress(&format!("newton {steps} residual {norm:.3e}"));
        if norm <= tol {
            return Ok((x, steps));
        }
        check(deadline, || format!("newton at residual {norm:.3e}"))?;
        let jac = Jacobian::new(m, x.clone(), fx.clone());
        let mut dx = gmres(&mut |v| jac.apply(v), &fx, norm.min(0.1))
            .map_err(|e| no_convergence(format!("{e} at Newton step {steps}")))?;
        let scale = (STEP_CAP / max_abs(&dx)).min(1.0);
        for (xi, d) in x.iter_mut().zip(&mut dx) {
            *d *= -scale;
            *xi += *d;
        }
        fx = model::residual(&x, m);
    }
    Err(no_convergence(format!(
        "residual {} after {} Newton steps",
        max_abs(&fx),
        limits.max_newton
    )))
}

/// `a x = b` in the least-squares sense with the minimum norm (as numpy's `lstsq` with the
/// default cutoff, singular values below eps max(n, m) of the largest dropped): `a` given by its
/// m columns of length n, decomposed by one-sided Jacobi.
fn lstsq(columns: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
    let m = columns.len();
    let n = b.len();
    let mut u = columns.to_vec();
    let mut v: Vec<Vec<f64>> = (0..m)
        .map(|i| (0..m).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for _ in 0..100 {
        let mut rotated = false;
        for p in 0..m {
            for q in p + 1..m {
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
                for cols in [&mut u, &mut v] {
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
    let sigma: Vec<f64> = u.iter().map(|c| norm(c)).collect();
    let cutoff = f64::EPSILON * n.max(m) as f64 * sigma.iter().copied().fold(0.0, f64::max);
    let mut x = vec![0.0; m];
    for i in (0..m).filter(|&i| sigma[i] > cutoff) {
        let coef = dot(&u[i], b) / (sigma[i] * sigma[i]);
        for (xk, vk) in x.iter_mut().zip(&v[i]) {
            *xk += coef * vk;
        }
    }
    x
}

/// ln p and ln w move with their market's scaled excess demand (step clipped like the price
/// rule), shifted together after each step so the world price index is 1. The fixed point of
/// that map is accelerated by Anderson mixing, restarted from the best iterate whenever the
/// step grows tenfold.
pub fn tatonnement(
    m: &Model,
    x0: &[f64],
    tol: f64,
    limits: Limits,
    deadline: Option<Instant>,
    progress: &mut dyn FnMut(&str),
) -> Result<Vec<f64>, SolveError> {
    let step = |x: &[f64]| -> Vec<f64> {
        let y: Vec<f64> = x
            .iter()
            .zip(model::excess(x, m))
            .map(|(x, e)| x + (TATONNEMENT_STEP * e).clamp(-TATONNEMENT_CLIP, TATONNEMENT_CLIP))
            .collect();
        let shift = model::evaluate(&y, m, &m.inp.ent_a).index.ln();
        y.iter().map(|y| y - shift).collect()
    };
    let (mut xs, mut rs): (Vec<Vec<f64>>, Vec<Vec<f64>>) = (Vec::new(), Vec::new());
    let (mut x, mut best, mut best_x) = (x0.to_vec(), f64::INFINITY, x0.to_vec());
    for it in 0..limits.max_tatonnement {
        let r: Vec<f64> = step(&x).iter().zip(&x).map(|(s, x)| s - x).collect();
        let norm = max_abs(&r);
        if it % 50 == 0 {
            progress(&format!("tatonnement {it} step {norm:.3e} best {best:.3e}"));
        }
        if norm <= tol {
            return Ok(x);
        }
        check(deadline, || format!("tatonnement at best step {best:.3e}"))?;
        if norm.is_nan() || norm >= 10.0 * best {
            xs.clear();
            rs.clear();
            x.clone_from(&best_x);
            let s = step(&x);
            x = x.iter().zip(&s).map(|(x, s)| x + s - x).collect();
            continue;
        }
        if norm < best {
            best = norm;
            best_x.clone_from(&x);
        }
        if xs.len() > ANDERSON_MEMORY {
            xs.remove(0);
            rs.remove(0);
        }
        xs.push(x.clone());
        rs.push(r.clone());
        if rs.len() > 1 {
            let diff = |a: &[Vec<f64>]| -> Vec<Vec<f64>> {
                a.windows(2)
                    .map(|w| w[1].iter().zip(&w[0]).map(|(b, a)| b - a).collect())
                    .collect()
            };
            let (dx, dr) = (diff(&xs), diff(&rs));
            let gamma = lstsq(&dr, &r);
            for (k, xk) in x.iter_mut().enumerate() {
                let mut mix = 0.0;
                for (j, g) in gamma.iter().enumerate() {
                    mix += (dx[j][k] + dr[j][k]) * g;
                }
                *xk = *xk + r[k] - mix;
            }
        } else {
            for (xk, rk) in x.iter_mut().zip(&r) {
                *xk += rk;
            }
        }
    }
    Err(no_convergence(format!(
        "tatonnement step {best} after {} iterations",
        limits.max_tatonnement
    )))
}

/// Solves `model::residual(x) = 0` to `tol` in the max norm, from the data state unless `x0` is
/// given: Newton-Krylov, and where it fails (step cap, non-finite residual), tatonnement and
/// Newton-Krylov from there. Errs without convergence (also past `deadline`), or for a root that
/// is not a Rust steady state.
pub fn equilibrium(
    m: &Model,
    x0: Option<&[f64]>,
    tol: f64,
    deadline: Option<Instant>,
) -> Result<Equilibrium, SolveError> {
    equilibrium_with(m, x0, tol, deadline, Limits::default(), &mut |_| {})
}

/// [`equilibrium`] with explicit iteration caps; `progress` follows the solve.
pub fn equilibrium_with(
    m: &Model,
    x0: Option<&[f64]>,
    tol: f64,
    deadline: Option<Instant>,
    limits: Limits,
    progress: &mut dyn FnMut(&str),
) -> Result<Equilibrium, SolveError> {
    let start = Instant::now();
    let x0 = x0.map_or_else(|| m.data_state(), <[f64]>::to_vec);
    let (x, steps, method) = match newton(m, &x0, tol, limits, deadline, progress) {
        Ok((x, steps)) => (x, steps, "newton_krylov"),
        Err(SolveError::NoConvergence(why)) => {
            progress(&format!("newton_krylov failed ({why}): tatonnement"));
            check(deadline, || "equilibrium".into())?;
            let start = tatonnement(m, &x0, TATONNEMENT_TOL, limits, deadline, progress)?;
            let (x, steps) = newton(m, &start, tol, limits, deadline, progress)?;
            (x, steps, "tatonnement, newton_krylov")
        }
        Err(e) => return Err(e),
    };
    let info = Info {
        method: method.into(),
        iterations: steps,
        residual: max_abs(&model::residual(&x, m)),
        dropped_province: m.dropped_province(),
        dropped_residual: model::excess(&x, m)[m.index_row()],
        seconds: start.elapsed().as_secs_f64(),
    };
    let eq = unpack(&x, m, info);
    let poor: Vec<usize> = (0..eq.income.len())
        .filter(|&p| eq.income[p] <= 0.0)
        .collect();
    if !poor.is_empty() {
        // Rust pops with no money buy nothing, and with negative income their balance never
        // recovers.
        return Err(SolveError::NotSteady(format!(
            "household income not positive in provinces {poor:?}"
        )));
    }
    Ok(eq)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn residual_norm(a: &[Vec<f64>], x: &[f64], b: &[f64]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(row, b)| (dot(row, x) - b).abs())
            .fold(0.0, f64::max)
    }

    /// A linear system that needs more than 30 Krylov vectors converges within one cycle of
    /// RESTART.
    #[test]
    fn gmres_uses_the_restart_size() {
        let n = 80;
        let mut seed = 5_u64;
        let mut noise = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        };
        let a: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        let diag = if i == j {
                            10f64.powf(4.0 * i as f64 / (n - 1) as f64)
                        } else {
                            0.0
                        };
                        diag + 0.01 * noise()
                    })
                    .collect()
            })
            .collect();
        let b: Vec<f64> = (0..n).map(|_| noise()).collect();
        let mut calls = 0;
        let x = gmres(
            &mut |v| {
                calls += 1;
                Ok(a.iter().map(|row| dot(row, v)).collect())
            },
            &b,
            1e-10,
        )
        .unwrap();
        assert!(
            30 < calls && calls <= RESTART * MAX_CYCLES,
            "{calls} products"
        );
        assert!(residual_norm(&a, &x, &b) <= 1e-6 * max_abs(&b));
    }

    #[test]
    fn lstsq_gives_the_minimum_norm_solution() {
        let c0 = vec![1.0, 2.0, 0.5, -1.0, 3.0, 0.25];
        let c1 = vec![0.3, -1.0, 2.0, 1.0, 0.0, 1.5];
        let b = vec![1.0, 0.0, -2.0, 0.5, 1.0, 2.0];
        // Full rank: the normal equations' solution.
        let x = lstsq(&[c0.clone(), c1.clone()], &b);
        let (g00, g01, g11) = (dot(&c0, &c0), dot(&c0, &c1), dot(&c1, &c1));
        let (r0, r1) = (dot(&c0, &b), dot(&c1, &b));
        let det = g00 * g11 - g01 * g01;
        let want = [(g11 * r0 - g01 * r1) / det, (g00 * r1 - g01 * r0) / det];
        for (got, want) in x.iter().zip(want) {
            assert!((got - want).abs() <= 1e-13 * want.abs(), "{got} vs {want}");
        }
        // A repeated column: the projection split evenly, the smallest-norm of the solutions.
        let x = lstsq(&[c0.clone(), c0.clone()], &b);
        let half = dot(&c0, &b) / dot(&c0, &c0) / 2.0;
        for got in x {
            assert!((got - half).abs() <= 1e-13 * half.abs(), "{got} vs {half}");
        }
    }
}
