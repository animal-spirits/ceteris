//! Spike (RFC 0361 D2): restarted GMRES on a matrix-free operator, serial, operations in index
//! order. Not for merge.

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
