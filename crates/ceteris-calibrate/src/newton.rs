//! The scaling's Newton phase (RFC 0361, D3 amended by RFC 0362): constants, and restarted GMRES
//! (D2) on a matrix-free operator, serial, operations in index order.

/// Sweeps before the Newton phase starts (ADR 0390, chosen by the spike's rule from {20, 50, 100}).
pub const NEWTON_AFTER: usize = 20;
/// Newton steps per good.
pub const NEWTON_MAX: usize = 60;
/// Largest step in ln w; a longer Newton step is scaled to it before the line search.
pub const STEP_CAP: f64 = 10.0;
pub const RESTART: usize = 30;
pub const MAX_RESTARTS: usize = 10;
/// Forcing term bounds, eta = clamp(max |F|, ETA_MIN, ETA_MAX) (Eisenstat and Walker 1996).
pub const ETA_MIN: f64 = 1e-8;
pub const ETA_MAX: f64 = 1e-1;
/// Line search halvings and the sufficient decrease constant.
pub const BACKTRACK: usize = 30;
pub const ARMIJO: f64 = 1e-4;

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

/// Solves A x = b to `||b - A x|| <= rel_tol ||b||`. Returns x and the number of products, or the
/// number of products spent when `restart * max_restarts` iterations do not reach it.
pub fn gmres(
    matvec: &mut dyn FnMut(&[f64]) -> Vec<f64>,
    b: &[f64],
    rel_tol: f64,
    restart: usize,
    max_restarts: usize,
) -> Result<(Vec<f64>, usize), usize> {
    let n = b.len();
    let beta0 = norm(b);
    let mut x = vec![0.0; n];
    let mut products = 0;
    if beta0 == 0.0 || !beta0.is_finite() {
        return if beta0 == 0.0 { Ok((x, 0)) } else { Err(0) };
    }
    let m = restart.min(n);
    for cycle in 0..max_restarts {
        let r: Vec<f64> = if cycle == 0 {
            b.to_vec()
        } else {
            let ax = matvec(&x);
            products += 1;
            b.iter().zip(&ax).map(|(b, a)| b - a).collect()
        };
        let beta = norm(&r);
        if beta <= rel_tol * beta0 {
            return Ok((x, products));
        }
        let mut v: Vec<Vec<f64>> = vec![r.iter().map(|x| x / beta).collect()];
        let mut h = vec![vec![0.0; m]; m + 1];
        let (mut cs, mut sn) = (vec![0.0; m], vec![0.0; m]);
        let mut g = vec![0.0; m + 1];
        g[0] = beta;
        let mut used = 0;
        for j in 0..m {
            let mut w = matvec(&v[j]);
            products += 1;
            for i in 0..=j {
                h[i][j] = dot(&w, &v[i]);
                for (wk, vk) in w.iter_mut().zip(&v[i]) {
                    *wk -= h[i][j] * vk;
                }
            }
            h[j + 1][j] = norm(&w);
            if !h[j + 1][j].is_finite() {
                return Err(products);
            }
            let breakdown = h[j + 1][j] <= 1e-300;
            if !breakdown {
                v.push(w.iter().map(|x| x / h[j + 1][j]).collect());
            }
            for i in 0..j {
                let t = cs[i] * h[i][j] + sn[i] * h[i + 1][j];
                h[i + 1][j] = -sn[i] * h[i][j] + cs[i] * h[i + 1][j];
                h[i][j] = t;
            }
            let d = h[j][j].hypot(h[j + 1][j]);
            (cs[j], sn[j]) = (h[j][j] / d, h[j + 1][j] / d);
            h[j][j] = d;
            h[j + 1][j] = 0.0;
            g[j + 1] = -sn[j] * g[j];
            g[j] *= cs[j];
            used = j + 1;
            if g[j + 1].abs() <= rel_tol * beta0 || breakdown {
                break;
            }
        }
        let mut y = vec![0.0; used];
        for i in (0..used).rev() {
            let mut s = g[i];
            for k in i + 1..used {
                s -= h[i][k] * y[k];
            }
            y[i] = s / h[i][i];
        }
        for (j, yj) in y.iter().enumerate() {
            for (xk, vk) in x.iter_mut().zip(&v[j]) {
                *xk += yj * vk;
            }
        }
        if g[used].abs() <= rel_tol * beta0 {
            return if x.iter().all(|x| x.is_finite()) {
                Ok((x, products))
            } else {
                Err(products)
            };
        }
    }
    Err(products)
}

#[cfg(test)]
mod tests {
    use super::gmres;

    /// A fixed non-symmetric, diagonally dominant test matrix.
    fn matrix(n: usize) -> Vec<Vec<f64>> {
        (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| {
                        let off = ((i * 7 + j * 13) % 11) as f64 / 11.0 - 0.45;
                        if i == j {
                            3.0 + off
                        } else {
                            off / (1.0 + (i as f64 - j as f64).abs())
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Gaussian elimination with partial pivoting, the oracle.
    fn dense_solve(a: &[Vec<f64>], b: &[f64]) -> Vec<f64> {
        let n = b.len();
        let mut m: Vec<Vec<f64>> = a
            .iter()
            .zip(b)
            .map(|(row, &b)| row.iter().copied().chain([b]).collect())
            .collect();
        for c in 0..n {
            let p = (c..n)
                .max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))
                .unwrap();
            m.swap(c, p);
            let pivot = m[c].clone();
            for row in m.iter_mut().skip(c + 1) {
                let f = row[c] / pivot[c];
                for (x, p) in row.iter_mut().zip(&pivot).skip(c) {
                    *x -= f * p;
                }
            }
        }
        let mut x = vec![0.0; n];
        for r in (0..n).rev() {
            let s: f64 = (r + 1..n).map(|k| m[r][k] * x[k]).sum();
            x[r] = (m[r][n] - s) / m[r][r];
        }
        x
    }

    #[test]
    fn gmres_matches_dense_lu() {
        for n in [5, 40, 75] {
            let a = matrix(n);
            let b: Vec<f64> = (0..n).map(|i| (i as f64).sin() + 0.5).collect();
            let mut mv = |x: &[f64]| -> Vec<f64> {
                a.iter()
                    .map(|row| row.iter().zip(x).map(|(a, x)| a * x).sum())
                    .collect()
            };
            let (x, products) = gmres(&mut mv, &b, 1e-12, 30, 10).unwrap();
            let want = dense_solve(&a, &b);
            let err = x
                .iter()
                .zip(&want)
                .fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
            assert!(err < 1e-10, "n {n}: {err:e}");
            assert!(products <= 300);
        }
    }

    #[test]
    fn gmres_reports_a_singular_system() {
        // rank one: no solution exists for this right-hand side
        let mut mv = |x: &[f64]| -> Vec<f64> { vec![x.iter().sum::<f64>(); x.len()] };
        let b: Vec<f64> = (0..6).map(|i| i as f64).collect();
        assert!(gmres(&mut mv, &b, 1e-8, 30, 10).is_err());
    }
}
