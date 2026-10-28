//! PIGL final demand (ADR 0021): the multi-sector PIGL of Alder, Boppart and Mueller (2022) across
//! three categories, a CES over goods within each category, and a pop's lognormal income
//! integrated in closed form through E[y^(1 - eps)]. Vectorized over rows of pops (or draws).

use statrs::function::erf::erf_inv;

use crate::io::Mat;

pub const N_CATEGORIES: usize = 3;

/// P46 sigma_b, phi, gamma, theta (A, M, S); P47 eps.
#[derive(Clone, Debug, PartialEq)]
pub struct Pigl {
    pub sigma_b: f64,
    pub phi: f64,
    pub gamma: f64,
    pub theta: [f64; N_CATEGORIES],
    pub eps: f64,
}

/// Per country: scale lambda (C,), category weights omega (C, 3), within-category weights (C, G).
#[derive(Clone, Debug, PartialEq)]
pub struct Weights {
    pub scale: Vec<f64>,
    pub omega: Mat,
    pub within: Mat,
}

/// sqrt(2) Phi^-1((G + 1) / 2) = 2 erfinv(G).
pub fn lognormal_sigma(gini: &[f64]) -> Vec<f64> {
    gini.iter().map(|&g| 2.0 * erf_inv(g)).collect()
}

/// Shares normalized over each row, and their sums per category (rows, 3).
fn by_category(omega: &Mat, category: &[usize]) -> (Mat, Mat) {
    let total = omega.row_sums();
    let om = Mat::from_fn(omega.rows, omega.cols, |r, g| omega[(r, g)] / total[r]);
    let cat = category_sums(&om, category);
    (om, cat)
}

/// `m @ onehot(category)`: each row's sum over the goods of every category.
fn category_sums(m: &Mat, category: &[usize]) -> Mat {
    let mut out = Mat::zeros(m.rows, N_CATEGORIES);
    for r in 0..m.rows {
        for (g, &c) in category.iter().enumerate() {
            out[(r, c)] += m[(r, g)];
        }
    }
    out
}

fn negative_weight(bad: &[usize]) -> String {
    format!("PIGL inversion gives a negative weight in countries {bad:?}")
}

/// `invert_pigl` for every country: omega (C, G), base spending per person and sigma (C,). Errs
/// where a weight would be negative.
pub fn invert(
    omega: &Mat,
    base_spend: &[f64],
    sigma: &[f64],
    category: &[usize],
    p: &Pigl,
) -> Result<Weights, String> {
    let (om, cat) = by_category(omega, category);
    let within = Mat::from_fn(om.rows, om.cols, |r, g| {
        if om[(r, g)] > 0.0 {
            om[(r, g)] / cat[(r, category[g])]
        } else {
            0.0
        }
    });
    let z: Vec<f64> = (0..cat.rows).map(|r| cat[(r, 0)] / p.theta[0]).collect();
    let mut weight = Mat::from_fn(cat.rows, N_CATEGORIES, |r, j| {
        (cat[(r, j)] - z[r] * p.theta[j]) / (1.0 - z[r])
    });
    for r in 0..weight.rows {
        weight[(r, 0)] = 0.0;
    }
    let bad: Vec<usize> = (0..weight.rows)
        .filter(|&r| z[r] >= 1.0 || weight.row(r).iter().any(|&w| w < 0.0))
        .collect();
    if !bad.is_empty() {
        return Err(negative_weight(&bad));
    }
    let eps = p.eps;
    let scale = (0..z.len())
        .map(|r| {
            z[r] * base_spend[r].powf(eps) * (eps * (1.0 - eps) * sigma[r].powi(2) / 2.0).exp()
        })
        .collect();
    Ok(Weights {
        scale,
        omega: weight,
        within,
    })
}

/// Stage 4 of ADR 0089: the weights of every country whose shares at buyer prices `price`
/// (C, G) and spending per person `spend` (C,) are its final demand shares `omega`; `invert` at
/// prices 1. Within a category, alpha_g is proportional to Omega_g p_g^(sigma - 1); across
/// categories, omega_A = 0 gives the income-effect weight K = Omega_A / sD_A, then
/// sB = (Omega - K sD) / (1 - K) and lambda in closed form. Errs where a weight would be negative.
#[allow(clippy::too_many_arguments)]
pub fn invert_at(
    omega: &Mat,
    price: &Mat,
    spend: &[f64],
    sigma: &[f64],
    category: &[usize],
    sigma_demand: f64,
    p: &Pigl,
) -> Result<Weights, String> {
    let (om, cat) = by_category(omega, category);
    let (rows, cols) = (om.rows, om.cols);
    let raw = Mat::from_fn(rows, cols, |r, g| {
        if om[(r, g)] > 0.0 {
            om[(r, g)] * price[(r, g)].powf(sigma_demand - 1.0)
        } else {
            0.0
        }
    });
    let raw_cat = category_sums(&raw, category);
    let within = Mat::from_fn(rows, cols, |r, g| {
        if om[(r, g)] > 0.0 {
            raw[(r, g)] / raw_cat[(r, category[g])]
        } else {
            0.0
        }
    });
    let term = category_sums(
        &Mat::from_fn(rows, cols, |r, g| {
            if within[(r, g)] > 0.0 {
                within[(r, g)] * price[(r, g)].powf(1.0 - sigma_demand)
            } else {
                0.0
            }
        }),
        category,
    );
    let index = Mat::from_fn(rows, N_CATEGORIES, |r, j| {
        if cat[(r, j)] > 0.0 {
            term[(r, j)].powf(1.0 / (1.0 - sigma_demand))
        } else {
            1.0
        }
    });
    let theta = Mat::from_fn(rows, N_CATEGORIES, |_, j| p.theta[j]);
    let (dt, s_d) = aggregate(&theta, &index, p.phi);
    let k: Vec<f64> = (0..rows).map(|r| cat[(r, 0)] / s_d[(r, 0)]).collect();
    let mut s_b = Mat::from_fn(rows, N_CATEGORIES, |r, j| {
        (cat[(r, j)] - k[r] * s_d[(r, j)]) / (1.0 - k[r])
    });
    for r in 0..rows {
        s_b[(r, 0)] = 0.0;
    }
    let bad: Vec<usize> = (0..rows)
        .filter(|&r| k[r] >= 1.0 || s_b.row(r).iter().any(|&w| w < 0.0))
        .collect();
    if !bad.is_empty() {
        return Err(negative_weight(&bad));
    }
    let mut weight = Mat::from_fn(rows, N_CATEGORIES, |r, j| {
        s_b[(r, j)] * index[(r, j)].powf(p.sigma_b - 1.0)
    });
    let total = weight.row_sums();
    for (r, t) in total.iter().enumerate() {
        for w in weight.row_mut(r) {
            *w /= t;
        }
    }
    let (b, _) = aggregate(&weight, &index, p.sigma_b);
    let eps = p.eps;
    // A country without people spends 0 and gets scale 0, as `invert`.
    let scale = (0..rows)
        .map(|r| {
            k[r] / ((dt[r] / b[r]).powf(p.gamma)
                * (spend[r] / b[r]).powf(-eps)
                * (-eps * (1.0 - eps) * sigma[r].powi(2) / 2.0).exp())
        })
        .collect();
    Ok(Weights {
        scale,
        omega: weight,
        within,
    })
}

/// CES aggregator over categories with a weight and a finite index, weights renormalized over
/// them: the aggregate (K,) and each category's share (K, 3).
fn aggregate(weight: &Mat, index: &Mat, sigma: f64) -> (Vec<f64>, Mat) {
    let mut agg = Vec::with_capacity(weight.rows);
    let mut share = Mat::zeros(weight.rows, N_CATEGORIES);
    for r in 0..weight.rows {
        let (mut total, mut mass) = (0.0, 0.0);
        let mut term = [0.0; N_CATEGORIES];
        for j in 0..N_CATEGORIES {
            let w = weight[(r, j)];
            if w > 0.0 && index[(r, j)].is_finite() {
                term[j] = w * index[(r, j)].powf(1.0 - sigma);
                mass += w;
            }
            total += term[j];
        }
        agg.push((total / mass).powf(1.0 / (1.0 - sigma)));
        for j in 0..N_CATEGORIES {
            share[(r, j)] = term[j] / total;
        }
    }
    (agg, share)
}

/// `pigl_shares`: budget shares (K, G) of pops of `country` (K,) at buyer prices (K, G), mean
/// income per person and log income dispersion (K,). Negative category shares are clamped to 0
/// and the rest rescaled; goods without a weight or at an infinite price get 0.
#[allow(clippy::too_many_arguments)]
pub fn shares(
    w: &Weights,
    country: &[usize],
    price: &Mat,
    income: &[f64],
    sigma: &[f64],
    category: &[usize],
    sigma_demand: f64,
    p: &Pigl,
) -> Mat {
    let within_w = w.within.select_rows(country);
    let (rows, cols) = (within_w.rows, within_w.cols);
    let term = Mat::from_fn(rows, cols, |r, g| {
        let (a, q) = (within_w[(r, g)], price[(r, g)]);
        if a > 0.0 && q.is_finite() {
            a * q.powf(1.0 - sigma_demand)
        } else {
            0.0
        }
    });
    let total = category_sums(&term, category);
    let index = Mat::from_fn(rows, N_CATEGORIES, |r, j| {
        if total[(r, j)] > 0.0 {
            total[(r, j)].powf(1.0 / (1.0 - sigma_demand))
        } else {
            f64::INFINITY
        }
    });
    let within = Mat::from_fn(rows, cols, |r, g| {
        if term[(r, g)] > 0.0 {
            term[(r, g)] / total[(r, category[g])]
        } else {
            0.0
        }
    });

    let theta = Mat::from_fn(rows, N_CATEGORIES, |_, j| p.theta[j]);
    let (dt, s_d) = aggregate(&theta, &index, p.phi);
    let omega = w.omega.select_rows(country);
    let (b, s_b) = aggregate(&omega, &index, p.sigma_b);
    let eps = p.eps;
    let mut out = Mat::zeros(rows, cols);
    for r in 0..rows {
        let k = w.scale[country[r]]
            * (dt[r] / b[r]).powf(p.gamma)
            * (income[r] / b[r]).powf(-eps)
            * (-eps * (1.0 - eps) * sigma[r].powi(2) / 2.0).exp();
        // No reachable category in B(P): the income-effect bundle is all there is.
        let no_b = !(0..N_CATEGORIES).any(|j| omega[(r, j)] > 0.0 && index[(r, j)].is_finite());
        let mut raw = [0.0; N_CATEGORIES];
        for (j, v) in raw.iter_mut().enumerate() {
            *v = if no_b {
                s_d[(r, j)]
            } else if index[(r, j)].is_finite() {
                s_b[(r, j)] + k * (s_d[(r, j)] - s_b[(r, j)])
            } else {
                0.0
            };
            // np.maximum keeps NaN, f64::max drops it.
            if !v.is_nan() {
                *v = v.max(0.0);
            }
        }
        let sum: f64 = raw.iter().sum();
        for g in 0..cols {
            out[(r, g)] = raw[category[g]] / sum * within[(r, g)];
        }
    }
    out
}

/// `pigl`: units bought from budget shares (K, G) and spending (K,); none without money.
pub fn quantities(share: &Mat, price: &Mat, money: &[f64]) -> Mat {
    Mat::from_fn(share.rows, share.cols, |r, g| {
        let s = share[(r, g)];
        if money[r] > 0.0 && s > 0.0 {
            s * money[r] / price[(r, g)]
        } else {
            0.0
        }
    })
}
