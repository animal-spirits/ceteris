//! Uniqueness check before V1 (ADR 0022; `specs/econ/SPEC.md`, Equilibrium nesting): Theorem 1 of
//! Allen, Arkolakis and Li (2024, "On the Equilibrium Properties of Spatial Models", AER: Insights
//! 6(4)). A system x_ih = sum_j f_ijh(x_j) whose elasticities are bounded by an H x H matrix A has
//! a unique solution if rho(A) < 1, at most one up to scale if rho(A) = 1 with constant
//! elasticities, and for rho(A) > 1 some kernel gives several.
//!
//! P2 maps onto the paper's Online Appendix A.3 (Eaton-Kortum with intermediate goods and
//! quasi-symmetric trade costs), applied good by good; the SPEC states the special case and why.

use crate::io::{Inputs, Mat};

pub const MAX_POWER_ITER: usize = 100_000;

/// rho of the elasticity-bound matrix |a| (Theorem 1 bounds absolute elasticities): its Perron
/// root, the largest over the irreducible diagonal blocks of |a| (its strongly connected
/// components), each by [`perron_root`].
pub fn spectral_radius(a: &Mat) -> f64 {
    assert_eq!(a.rows, a.cols, "spectral radius of a non-square matrix");
    let n = a.rows;
    let a = Mat::from_fn(n, n, |r, c| a[(r, c)].abs());
    let mut reach: Vec<Vec<bool>> = (0..n)
        .map(|i| (0..n).map(|j| i == j || a[(i, j)] > 0.0).collect())
        .collect();
    for k in 0..n {
        let via = reach[k].clone();
        for row in &mut reach {
            if row[k] {
                for (r, v) in row.iter_mut().zip(&via) {
                    *r |= v;
                }
            }
        }
    }
    let mut seen = vec![false; n];
    let mut rho = 0.0_f64;
    for i in 0..n {
        if seen[i] {
            continue;
        }
        let block: Vec<usize> = (0..n).filter(|&j| reach[i][j] && reach[j][i]).collect();
        for &j in &block {
            seen[j] = true;
        }
        let sub = Mat::from_fn(block.len(), block.len(), |r, c| a[(block[r], block[c])]);
        rho = rho.max(perron_root(&sub));
    }
    rho
}

/// Perron root of an irreducible nonnegative matrix by power iteration on a + s I, s its largest
/// row sum: the shift makes the matrix primitive and keeps every iterate positive, so the
/// smallest and largest of (a x)_i / x_i bracket the root (Collatz-Wielandt). Stops when the
/// bracket closes or its upper end stops falling, and returns the upper end.
fn perron_root(a: &Mat) -> f64 {
    let shift = a.row_sums().into_iter().fold(0.0, f64::max);
    if shift == 0.0 {
        return 0.0;
    }
    let mut x = vec![1.0; a.rows];
    let mut upper = f64::INFINITY;
    for _ in 0..MAX_POWER_ITER {
        let ax = a.matvec(&x);
        let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
        for (v, &xi) in ax.iter().zip(&x) {
            lo = lo.min(v / xi);
            hi = hi.max(v / xi);
        }
        if hi - lo <= 4.0 * f64::EPSILON * hi || hi >= upper {
            return hi.min(upper);
        }
        upper = hi;
        for (xi, v) in x.iter_mut().zip(&ax) {
            *xi = v + shift * *xi;
        }
        let top = x.iter().copied().fold(0.0, f64::max);
        for xi in &mut x {
            *xi /= top;
        }
    }
    upper
}

/// `m` X = `rhs` by Gaussian elimination with partial pivoting.
fn solve(m: &Mat, rhs: &Mat) -> Mat {
    assert!(m.rows == m.cols && rhs.rows == m.rows, "solve shapes");
    let n = m.rows;
    let (mut m, mut x) = (m.clone(), rhs.clone());
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&i, &j| m[(i, col)].abs().total_cmp(&m[(j, col)].abs()))
            .unwrap_or(col);
        if pivot != col {
            for c in 0..n {
                m.data.swap(col * n + c, pivot * n + c);
            }
            for c in 0..x.cols {
                x.data.swap(col * x.cols + c, pivot * x.cols + c);
            }
        }
        for r in col + 1..n {
            let l = m[(r, col)] / m[(col, col)];
            for c in col..n {
                m[(r, c)] -= l * m[(col, c)];
            }
            for c in 0..x.cols {
                x[(r, c)] -= l * x[(col, c)];
            }
        }
    }
    for col in (0..n).rev() {
        for c in 0..x.cols {
            let mut v = x[(col, c)];
            for k in col + 1..n {
                v -= m[(col, k)] * x[(k, c)];
            }
            x[(col, c)] = v / m[(col, col)];
        }
    }
    x
}

/// Remark 5: the constant-elasticity system prod x_ih'^gamma_hh' = sum_j K_ijh prod
/// x_ih'^r_hh' x_jh'^b_hh' (equation 8) has elasticity bounds |B (Gamma - R)^-1|.
pub fn change_of_variables(gamma: &Mat, b: &Mat, r: Option<&Mat>) -> Mat {
    let m = match r {
        Some(r) => Mat::from_fn(gamma.rows, gamma.cols, |i, j| gamma[(i, j)] - r[(i, j)]),
        None => gamma.clone(),
    };
    let x = solve(&m.transpose(), &b.transpose()).transpose();
    Mat::from_fn(x.rows, x.cols, |i, j| x[(i, j)].abs())
}

/// Online Appendix A.3 (pp. 8-9): equations (29) and (30) in tradeables price index and wage,
/// labour share beta of tradeables production, the rest the tradeables aggregate. The paper's
/// theta is the inverse of the trade elasticity (Alvarez and Lucas 2007), so its 1/theta in
/// Gamma and B is the trade elasticity.
pub fn trade_with_intermediates(elasticity: f64, labour_share: f64) -> Mat {
    let (t, beta) = (elasticity, labour_share);
    let gamma = Mat::from_rows(&[vec![-t, 0.0], vec![(1.0 - beta) * t, 1.0 + beta * t]]);
    let b = Mat::from_rows(&[
        vec![-(1.0 - beta) * t, -beta * t],
        vec![-(1.0 - beta) * t, -beta * t],
    ]);
    change_of_variables(&gamma, &b, None)
}

/// Per good, the smallest value-added share of gross output over countries, 1 - sum_k a_kg (the
/// largest bound, since A.3's radius falls with the labour share).
pub fn value_added_share(inp: &Inputs) -> Vec<f64> {
    (0..inp.n_goods())
        .map(|g| {
            inp.io
                .iter()
                .map(|a| 1.0 - (0..a.rows).map(|k| a[(k, g)]).sum::<f64>())
                .fold(f64::INFINITY, f64::min)
        })
        .collect()
}

/// Largest A.3 radius over traded goods (theta > 0) at each good's trade elasticity and
/// value-added share (1 everywhere: labour the only input). Agglomeration and migration have no
/// mapping yet: the paper treats them only without trade costs (A.6) or without intermediates
/// and with spatial spillovers (A.2, where any spillover gives a radius above 1), so they are
/// derived when A5 or migration lands.
pub fn spectral_bound(
    theta: &[f64],
    agglomeration: f64,
    migration: Option<f64>,
    value_added_share: &[f64],
) -> Result<f64, String> {
    if agglomeration != 0.0 || migration.is_some() {
        return Err(
            "uniqueness bound with agglomeration or migration: no mapping onto Theorem 1 yet (ADR 0022)"
                .into(),
        );
    }
    (0..theta.len())
        .filter(|&g| theta[g] > 0.0)
        .map(|g| spectral_radius(&trade_with_intermediates(theta[g], value_added_share[g])))
        .reduce(f64::max)
        .ok_or_else(|| "uniqueness bound without a traded good".into())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum V1Mode {
    /// The 1% tolerances of V1.
    Tolerance,
    /// Which equilibrium each model reaches.
    Report,
}

/// V1 compares with the 1% tolerances where the equilibrium is unique (bound at most 1, Theorem
/// 1 parts i and ii.b); otherwise it reports which equilibrium each model reaches.
pub fn v1_mode(bound: f64) -> V1Mode {
    if bound <= 1.0 {
        V1Mode::Tolerance
    } else {
        V1Mode::Report
    }
}
