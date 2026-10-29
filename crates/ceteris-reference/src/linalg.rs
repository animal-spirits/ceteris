//! Dense linear algebra of the reference, with every sum in a fixed order: numpy's `lstsq`
//! (minimum norm, singular values at or below eps max(n, k) of the largest dropped) by Householder
//! QR and one-sided Jacobi SVD, numpy's `pinv` (1e-15 relative) of a symmetric positive
//! semidefinite matrix, and Cholesky solves.

use crate::io::Mat;

/// Jacobi sweeps before the SVD counts as not converged.
const MAX_SWEEPS: usize = 100;

pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

pub fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// `dot` in eight running sums over chunks of eight, combined as a tree, then the rest: a fixed
/// order the compiler can vectorize, for the long sums of the decompositions.
fn dot8(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    let (ac, ar) = a[..n].as_chunks::<8>();
    let (bc, br) = b[..n].as_chunks::<8>();
    let mut acc = [0.0; 8];
    for (x, y) in ac.iter().zip(bc) {
        for l in 0..8 {
            acc[l] += x[l] * y[l];
        }
    }
    let mut s = ((acc[0] + acc[1]) + (acc[2] + acc[3])) + ((acc[4] + acc[5]) + (acc[6] + acc[7]));
    for (x, y) in ar.iter().zip(br) {
        s += x * y;
    }
    s
}

fn norm8(a: &[f64]) -> f64 {
    dot8(a, a).sqrt()
}

/// Householder QR of the columns `a` (each of length n) applied to `b` too: `a` becomes R in
/// its first min(n, k) rows and `b` becomes Q^T b. Returns the number of reflections.
fn householder(a: &mut [Vec<f64>], b: &mut [f64]) -> usize {
    let n = b.len();
    let steps = n.min(a.len());
    for j in 0..steps {
        let mut v = a[j][j..].to_vec();
        let size = norm8(&v);
        if size == 0.0 {
            continue;
        }
        let alpha = if v[0] > 0.0 { -size } else { size };
        v[0] -= alpha;
        let vv = dot8(&v, &v);
        if vv == 0.0 {
            continue;
        }
        for col in a.iter_mut().skip(j) {
            let t = 2.0 * dot8(&v, &col[j..]) / vv;
            for (c, vi) in col[j..].iter_mut().zip(&v) {
                *c -= t * vi;
            }
        }
        let t = 2.0 * dot8(&v, &b[j..]) / vv;
        for (c, vi) in b[j..].iter_mut().zip(&v) {
            *c -= t * vi;
        }
    }
    steps
}

/// One-sided Jacobi on the columns `u`: rotates them until orthogonal, applying each rotation to
/// the columns of `v` (the identity to start). The column norms of `u` are then the singular
/// values, `u` holds U times them and `v` the right singular vectors. A pair with a column at or
/// below `drop` times the largest is left alone: such a column is dropped as a zero singular
/// value (a rotation only shrinks the smaller column of a pair), and rounding noise cannot be
/// made orthogonal to eps relative.
fn jacobi(u: &mut [Vec<f64>], drop: f64) -> Result<Vec<Vec<f64>>, String> {
    let k = u.len();
    let mut v: Vec<Vec<f64>> = (0..k)
        .map(|i| (0..k).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for _ in 0..MAX_SWEEPS {
        let cutoff = drop * u.iter().map(|c| norm8(c)).fold(0.0, f64::max);
        let mut rotated = false;
        for p in 0..k {
            for q in p + 1..k {
                let (alpha, beta, gamma) =
                    (dot8(&u[p], &u[p]), dot8(&u[q], &u[q]), dot8(&u[p], &u[q]));
                if gamma == 0.0
                    || gamma.abs() <= f64::EPSILON * (alpha * beta).sqrt()
                    || alpha.sqrt().min(beta.sqrt()) <= cutoff
                {
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
            return Ok(v);
        }
    }
    Err(format!("SVD did not converge in {MAX_SWEEPS} sweeps"))
}

/// The minimum-norm least-squares solution of `a x = b`, `a` given by its k columns, decomposed
/// by one-sided Jacobi without a QR step; singular values at or below eps max(n, k) of the
/// largest are treated as zero, n the number of rows numpy's cutoff counts.
fn jacobi_lstsq(columns: &[Vec<f64>], b: &[f64], n: usize) -> Result<Vec<f64>, String> {
    let k = columns.len();
    let drop = f64::EPSILON * n.max(k) as f64;
    let mut u = columns.to_vec();
    let v = jacobi(&mut u, drop)?;
    let sigma: Vec<f64> = u.iter().map(|c| norm8(c)).collect();
    let cutoff = drop * sigma.iter().copied().fold(0.0, f64::max);
    let mut x = vec![0.0; k];
    for i in (0..k).filter(|&i| sigma[i] > cutoff) {
        let coef = dot8(&u[i], b) / (sigma[i] * sigma[i]);
        for (xk, vk) in x.iter_mut().zip(&v[i]) {
            *xk += coef * vk;
        }
    }
    Ok(x)
}

/// numpy's `lstsq(a, b, rcond=None)[0]` by one-sided Jacobi on the columns of `a` themselves,
/// for few short columns.
pub fn lstsq_small(columns: &[Vec<f64>], b: &[f64]) -> Result<Vec<f64>, String> {
    jacobi_lstsq(columns, b, b.len())
}

/// numpy's `lstsq(a, b, rcond=None)[0]` for `a` given by its k columns of length n: Householder
/// QR, then back-substitution when R's singular values provably all exceed numpy's cutoff
/// (1 / ||R^-1||_F <= s_min and s_max <= ||R||_F), else the minimum-norm solution from the
/// Jacobi SVD of R.
pub fn lstsq(columns: &[Vec<f64>], b: &[f64]) -> Result<Vec<f64>, String> {
    let (n, k) = (b.len(), columns.len());
    let mut r = columns.to_vec();
    let mut qtb = b.to_vec();
    let rows = householder(&mut r, &mut qtb);
    for col in &mut r {
        col.truncate(rows);
    }
    qtb.truncate(rows);
    if rows == k
        && let Some(x) = full_rank_solve(&r, &qtb, f64::EPSILON * n.max(k) as f64)
    {
        return Ok(x);
    }
    jacobi_lstsq(&r, &qtb, n)
}

/// The solution of the upper triangular `r x = y` (`r` by columns) when every singular value of
/// `r` provably exceeds `drop` times the largest, else `None`.
fn full_rank_solve(r: &[Vec<f64>], y: &[f64], drop: f64) -> Option<Vec<f64>> {
    let k = r.len();
    let size = r.iter().map(|c| dot(c, c)).sum::<f64>().sqrt();
    // Columns of R^-1 by back-substitution, for the Frobenius norm of the inverse.
    let mut inverse = 0.0;
    let mut e = vec![0.0; k];
    for j in 0..k {
        e.fill(0.0);
        e[j] = 1.0;
        let col = back_substitute(r, &e[..=j])?;
        inverse += dot(&col, &col);
    }
    let bound = 1.0 / inverse.sqrt();
    if !(bound.is_finite() && bound > drop * size) {
        return None;
    }
    back_substitute(r, y)
}

/// `r[..m, ..m] x = y` for upper triangular `r` (by columns), m = y.len(); `None` on a zero or
/// non-finite result.
fn back_substitute(r: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let m = y.len();
    let mut x = y.to_vec();
    for i in (0..m).rev() {
        let mut s = x[i];
        for (j, col) in r.iter().enumerate().take(m).skip(i + 1) {
            s -= col[i] * x[j];
        }
        x[i] = s / r[i][i];
        if !x[i].is_finite() {
            return None;
        }
    }
    Some(x)
}

/// numpy's `pinv(a)` of a symmetric positive semidefinite matrix: singular values at or below
/// 1e-15 of the largest treated as zero. The inverse by Cholesky when every eigenvalue provably
/// exceeds the cutoff (1 / ||A^-1||_F <= l_min and l_max <= ||A||_F), else one-sided Jacobi.
pub fn pinv(a: &Mat) -> Result<Mat, String> {
    let k = a.rows;
    if let Some(inv) = cholesky_inverse(a) {
        let fro = |m: &Mat| dot(&m.data, &m.data).sqrt();
        if 1.0 / fro(&inv) > 1e-15 * fro(a) {
            return Ok(inv);
        }
    }
    let mut u: Vec<Vec<f64>> = (0..k)
        .map(|j| (0..k).map(|i| a[(i, j)]).collect())
        .collect();
    let v = jacobi(&mut u, 1e-15)?;
    let sigma: Vec<f64> = u.iter().map(|c| norm(c)).collect();
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
    Ok(out)
}

/// The inverse of the symmetric `a` by Cholesky, `None` if a pivot is not positive or an entry
/// not finite.
fn cholesky_inverse(a: &Mat) -> Option<Mat> {
    let k = a.rows;
    let mut l = a.data.clone();
    cholesky(&mut l, k)?;
    // L^-1, lower triangular, row by row of its columns.
    let mut li = Mat::zeros(k, k);
    for j in 0..k {
        li[(j, j)] = 1.0 / l[j * k + j];
        for i in j + 1..k {
            let mut s = 0.0;
            for m in j..i {
                s -= l[i * k + m] * li[(m, j)];
            }
            li[(i, j)] = s / l[i * k + i];
        }
    }
    let mut inv = Mat::zeros(k, k);
    for i in 0..k {
        for j in 0..=i {
            let mut s = 0.0;
            for m in i..k {
                s += li[(m, i)] * li[(m, j)];
            }
            inv[(i, j)] = s;
            inv[(j, i)] = s;
        }
    }
    inv.data.iter().all(|v| v.is_finite()).then_some(inv)
}

/// Cholesky factor of the symmetric `a` (row-major, n x n) in its lower triangle, in place;
/// `None` if a pivot is not positive.
fn cholesky(a: &mut [f64], n: usize) -> Option<()> {
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
    Some(())
}

/// Cholesky solve of the symmetric positive definite `a` (row-major, n x n) for the columns of
/// `b`, in place; `None` if a pivot is not positive.
pub fn cholesky_solve(a: &mut [f64], n: usize, b: &mut [Vec<f64>]) -> Option<()> {
    cholesky(a, n)?;
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

/// `x.T @ (x * w[:, None])` of the columns `x`. A column with few nonzeros (a dummy) is summed
/// over its nonzero rows only: the skipped terms are exact zeros, so the sums are those of the
/// full rows.
pub fn weighted_gram(x: &[Vec<f64>], w: &[f64]) -> Mat {
    let k = x.len();
    let support: Vec<Option<Vec<usize>>> = x
        .iter()
        .map(|c| {
            let nz: Vec<usize> = (0..c.len()).filter(|&i| c[i] != 0.0).collect();
            (nz.len() * 4 < c.len()).then_some(nz)
        })
        .collect();
    let mut out = Mat::zeros(k, k);
    for j in 0..k {
        let xw: Vec<f64> = x[j].iter().zip(w).map(|(x, w)| x * w).collect();
        for i in 0..k {
            let rows = match (&support[i], &support[j]) {
                (Some(a), Some(b)) => Some(if a.len() <= b.len() { a } else { b }),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            };
            out[(i, j)] = match rows {
                Some(rows) => {
                    let mut s = 0.0;
                    for &r in rows {
                        s += x[i][r] * xw[r];
                    }
                    s
                }
                None => dot8(&x[i], &xw),
            };
        }
    }
    out
}

/// `bread @ meat @ bread`.
pub fn sandwich(bread: &Mat, meat: &Mat) -> Mat {
    bread.matmul(meat).matmul(bread)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lstsq_gives_the_minimum_norm_solution() {
        let c0 = vec![1.0, 2.0, 0.5, -1.0, 3.0, 0.25];
        let c1 = vec![0.3, -1.0, 2.0, 1.0, 0.0, 1.5];
        let b = vec![1.0, 0.0, -2.0, 0.5, 1.0, 2.0];
        let (g00, g01, g11) = (dot(&c0, &c0), dot(&c0, &c1), dot(&c1, &c1));
        let (r0, r1) = (dot(&c0, &b), dot(&c1, &b));
        let det = g00 * g11 - g01 * g01;
        let want = [(g11 * r0 - g01 * r1) / det, (g00 * r1 - g01 * r0) / det];
        let half = dot(&c0, &b) / dot(&c0, &c0) / 2.0;
        for solve in [lstsq, lstsq_small] {
            // Full rank: the normal equations' solution.
            let x = solve(&[c0.clone(), c1.clone()], &b).unwrap();
            for (got, want) in x.iter().zip(want) {
                assert!((got - want).abs() <= 1e-13 * want.abs(), "{got} vs {want}");
            }
            // A repeated column: the projection split evenly, the smallest-norm solution.
            let x = solve(&[c0.clone(), c0.clone()], &b).unwrap();
            for got in x {
                assert!((got - half).abs() <= 1e-13 * half.abs(), "{got} vs {half}");
            }
        }
    }

    /// Three rows, five columns: `np.linalg.lstsq(a, b, rcond=None)[0]`.
    #[test]
    fn lstsq_of_a_wide_system_matches_numpy() {
        let columns = vec![
            vec![-0.3269748815891447, 0.08799321179027253, 0.3194251558635596],
            vec![0.43570024819929476, 0.1696447133068557, -0.3088960826387215],
            vec![0.1224352351986252, 0.39013899527890616, -0.284211350018602],
            vec![-0.37906569146549307, 0.1412182947606775, 0.3719307996494502],
            vec![
                0.19516206091734212,
                0.22460410137426345,
                -0.31098781060511194,
            ],
        ];
        let b = [
            0.39880592602601617,
            -0.0716806583408729,
            -0.28799560978641037,
        ];
        let want = [
            -0.26907373137073715,
            0.5254699262194698,
            -0.20410019053089978,
            -0.3125860245051302,
            -0.059557607343788385,
        ];
        for solve in [lstsq, lstsq_small] {
            let x = solve(&columns, &b).unwrap();
            for (got, want) in x.iter().zip(want) {
                assert!((got - want).abs() <= 1e-14, "{got} vs {want}");
            }
        }
    }

    /// Five columns of length three: two are rounding noise once the others are orthogonal and
    /// cannot be rotated orthogonal to eps relative. Without the drop the sweep cap reports it.
    #[test]
    fn jacobi_errs_at_its_sweep_cap() {
        let wide = vec![
            vec![-0.3269748815891447, 0.08799321179027253, 0.3194251558635596],
            vec![0.43570024819929476, 0.1696447133068557, -0.3088960826387215],
            vec![0.1224352351986252, 0.39013899527890616, -0.284211350018602],
            vec![-0.37906569146549307, 0.1412182947606775, 0.3719307996494502],
            vec![
                0.19516206091734212,
                0.22460410137426345,
                -0.31098781060511194,
            ],
        ];
        let mut u = wide.clone();
        assert_eq!(
            jacobi(&mut u, 0.0),
            Err(format!("SVD did not converge in {MAX_SWEEPS} sweeps"))
        );
        let mut u = wide;
        assert!(jacobi(&mut u, f64::EPSILON * 5.0).is_ok());
    }

    #[test]
    fn pinv_matches_the_inverse_and_the_minimum_norm_pseudo_inverse() {
        let a = Mat::from_rows(&[
            vec![4.0, 1.0, 0.5],
            vec![1.0, 3.0, 0.2],
            vec![0.5, 0.2, 2.0],
        ]);
        let p = pinv(&a).unwrap();
        let id = a.matmul(&p);
        for i in 0..3 {
            for j in 0..3 {
                let want = f64::from(u8::from(i == j));
                assert!(
                    (id[(i, j)] - want).abs() <= 1e-15,
                    "{i},{j}: {}",
                    id[(i, j)]
                );
            }
        }
        // Rank one: v v^T / |v|^4.
        let v = [1.0, 2.0, 2.0];
        let a = Mat::from_fn(3, 3, |i, j| v[i] * v[j]);
        let p = pinv(&a).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let want = v[i] * v[j] / 81.0;
                assert!((p[(i, j)] - want).abs() <= 1e-15, "{i},{j}: {}", p[(i, j)]);
            }
        }
    }
}
