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

/// Per country: scale lambda (C,), category weights omega (C, 3), within-category weights (C, G)
/// and the within-pop log income dispersion s_w (C,) they were inverted with (ADR 0094).
#[derive(Clone, Debug, PartialEq)]
pub struct Weights {
    pub scale: Vec<f64>,
    pub omega: Mat,
    pub within: Mat,
    pub sigma: Vec<f64>,
}

/// sqrt(2) Phi^-1((G + 1) / 2) = 2 erfinv(G). Infinite for |G| >= 1 (`io::validate` keeps G in [0, 1)).
pub fn lognormal_sigma(gini: &[f64]) -> Vec<f64> {
    gini.iter().map(|&g| 2.0 * erf_inv(g)).collect()
}

/// ADR 0094: the within-pop dispersion of each country (C,), s_w = sqrt(max(0, s_nat^2 -
/// Var_k ln m_k)), s_nat from its Gini and the variance population-weighted over its pops with
/// people and income: `country`, `people` and income per person `income` (K,). A country
/// without such pops keeps s_nat.
pub fn within_sigma(gini: &[f64], country: &[usize], people: &[f64], income: &[f64]) -> Vec<f64> {
    let national = lognormal_sigma(gini);
    let n_c = gini.len();
    let counted = |k: usize| people[k] > 0.0 && income[k] > 0.0;
    let (mut mass, mut sum) = (vec![0.0; n_c], vec![0.0; n_c]);
    for k in (0..country.len()).filter(|&k| counted(k)) {
        mass[country[k]] += people[k];
        sum[country[k]] += people[k] * income[k].ln();
    }
    let mut var = vec![0.0; n_c];
    for k in (0..country.len()).filter(|&k| counted(k)) {
        let c = country[k];
        var[c] += people[k] * (income[k].ln() - sum[c] / mass[c]).powi(2);
    }
    (0..n_c)
        .map(|c| {
            if mass[c] > 0.0 {
                (national[c].powi(2) - var[c] / mass[c]).max(0.0).sqrt()
            } else {
                national[c]
            }
        })
        .collect()
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
        sigma: sigma.to_vec(),
    })
}

/// A country's pops as the joint inversion sees them (K rows): country, buyer prices (K, G),
/// income per person and spending E_k, the weight of the pop in its country's aggregate.
pub struct Pops<'a> {
    pub country: &'a [usize],
    pub price: &'a Mat,
    pub income: &'a [f64],
    pub spend: &'a [f64],
}

/// Rounds of [`invert_pops`] before it gives up.
const MAX_ROUNDS: usize = 1000;
/// Secant steps of one split in [`invert_pops`] before it errs.
const MAX_SECANT_STEPS: usize = 400;
/// Largest relative miss of an aggregate good share at which [`invert_pops`] stops.
const SHARE_TOL: f64 = 1e-14;

/// ADR 0094, Amendment: per country, the weights at which its pops' PIGL shares, each pop at its
/// own buyer prices, income per person and the country's within-pop dispersion `sigma` (C,),
/// weighted by their spending, are its final demand shares `omega` (normalized). Rounds of two
/// steps from alpha_g = Omega_g / Omega_c: at the current alpha, the split t = omega_M of
/// omega_M + omega_S = 1 solves the M aggregate by a bracketed secant, lambda following in
/// closed form from food (omega_A = 0); then alpha_g is rescaled by Omega_g over its aggregate
/// share and renormalized within its category. A country with no spending pop takes [`invert`]
/// at spending 0. Errs where a weight would be negative (no root t in [0, 1]) or the rounds do
/// not settle.
pub fn invert_pops(
    omega: &Mat,
    pops: &Pops,
    sigma: &[f64],
    category: &[usize],
    sigma_demand: f64,
    p: &Pigl,
) -> Result<Weights, String> {
    let (n_c, n_g) = (omega.rows, omega.cols);
    let mut out = Weights {
        scale: vec![0.0; n_c],
        omega: Mat::zeros(n_c, N_CATEGORIES),
        within: Mat::zeros(n_c, n_g),
        sigma: sigma.to_vec(),
    };
    let (om, cat) = by_category(omega, category);
    for c in 0..n_c {
        let members: Vec<usize> = (0..pops.country.len())
            .filter(|&k| pops.country[k] == c && pops.spend[k] > 0.0)
            .collect();
        if members.is_empty() {
            let alone = invert(&omega.select_rows(&[c]), &[0.0], &sigma[c..=c], category, p)
                .map_err(|_| negative_weight(&[c]))?;
            out.omega.row_mut(c).copy_from_slice(alone.omega.row(0));
            out.within.row_mut(c).copy_from_slice(alone.within.row(0));
            continue;
        }
        let country = Country {
            om: om.row(c),
            cat: cat.row(c),
            members: &members,
            pops,
            sigma: sigma[c],
            category,
            sigma_demand,
            p,
        };
        let (scale, split, within) = country.solve().map_err(|e| match e {
            Unsettled(miss) => format!(
                "the PIGL inversion over the pops of country {c} did not settle in \
                 {MAX_ROUNDS} rounds (miss {miss:e})"
            ),
            SplitUnsettled => format!(
                "the PIGL inversion over the pops of country {c}: the split did not settle in \
                 {MAX_SECANT_STEPS} steps"
            ),
            Negative => negative_weight(&[c]),
        })?;
        out.scale[c] = scale;
        out.omega
            .row_mut(c)
            .copy_from_slice(&[0.0, split, 1.0 - split]);
        out.within.row_mut(c).copy_from_slice(&within[..n_g]);
    }
    Ok(out)
}

/// Why a country's inversion fails.
enum Failure {
    Negative,
    Unsettled(f64),
    SplitUnsettled,
}
use Failure::{Negative, SplitUnsettled, Unsettled};

/// One country's inversion over its spending pops `members`.
struct Country<'a> {
    om: &'a [f64],
    cat: &'a [f64],
    members: &'a [usize],
    pops: &'a Pops<'a>,
    sigma: f64,
    category: &'a [usize],
    sigma_demand: f64,
    p: &'a Pigl,
}

/// What a pop's shares depend on once alpha is fixed.
struct Fixed {
    /// Good shares within their category.
    within: Vec<f64>,
    /// P_c^(1 - sigma_B) where the category index is finite, else None.
    u: [Option<f64>; N_CATEGORIES],
    s_d: [f64; N_CATEGORIES],
    /// Dt^gamma m^-eps exp(-eps (1 - eps) s^2 / 2): the factor of lambda is this times
    /// B^(eps - gamma).
    x: f64,
}

impl Country<'_> {
    fn fixed(&self, alpha: &[f64]) -> Vec<Fixed> {
        let (p, s) = (self.p, self.sigma_demand);
        let n_g = self.om.len();
        self.members
            .iter()
            .map(|&k| {
                let mut term = vec![0.0; n_g];
                let mut sum = [0.0; N_CATEGORIES];
                for g in 0..n_g {
                    let q = self.pops.price[(k, g)];
                    if alpha[g] > 0.0 && q.is_finite() {
                        term[g] = alpha[g] * q.powf(1.0 - s);
                        sum[self.category[g]] += term[g];
                    }
                }
                let index = sum.map(|v| {
                    if v > 0.0 {
                        v.powf(1.0 / (1.0 - s))
                    } else {
                        f64::INFINITY
                    }
                });
                let within = (0..n_g)
                    .map(|g| {
                        if term[g] > 0.0 {
                            term[g] / sum[self.category[g]]
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let theta = Mat::from_fn(1, N_CATEGORIES, |_, j| p.theta[j]);
                let idx = Mat::from_fn(1, N_CATEGORIES, |_, j| index[j]);
                let (dt, s_d) = aggregate(&theta, &idx, p.phi);
                let eps = p.eps;
                Fixed {
                    within,
                    u: index.map(|i| i.is_finite().then(|| i.powf(1.0 - p.sigma_b))),
                    s_d: std::array::from_fn(|j| s_d[(0, j)]),
                    x: dt[0].powf(p.gamma)
                        * self.pops.income[k].powf(-eps)
                        * (-eps * (1.0 - eps) * self.sigma.powi(2) / 2.0).exp(),
                }
            })
            .collect()
    }

    /// sB of a pop at split t, and its factor of lambda.
    fn at(&self, f: &Fixed, t: f64) -> ([f64; N_CATEGORIES], f64) {
        let p = self.p;
        let weight = [0.0, t, 1.0 - t];
        let mut term = [0.0; N_CATEGORIES];
        let mut mass = 0.0;
        for j in 0..N_CATEGORIES {
            if let (true, Some(u)) = (weight[j] > 0.0, f.u[j]) {
                term[j] = weight[j] * u;
                mass += weight[j];
            }
        }
        let total: f64 = term.iter().sum();
        let b = (total / mass).powf(1.0 / (1.0 - p.sigma_b));
        (term.map(|v| v / total), f.x * b.powf(p.eps - p.gamma))
    }

    /// lambda and every pop's category shares at split t.
    fn categories(&self, fixed: &[Fixed], t: f64) -> (f64, Vec<[f64; N_CATEGORIES]>) {
        let at: Vec<_> = fixed.iter().map(|f| self.at(f, t)).collect();
        let spend = |i: usize| self.pops.spend[self.members[i]];
        let total: f64 = (0..fixed.len()).map(spend).sum();
        let food: f64 = (0..fixed.len())
            .map(|i| spend(i) * at[i].1 * fixed[i].s_d[0])
            .sum();
        let lambda = if self.cat[0] > 0.0 {
            self.cat[0] * total / food
        } else {
            0.0
        };
        let shares = at
            .iter()
            .zip(fixed)
            .map(|((s_b, factor), f)| {
                std::array::from_fn(|j| s_b[j] + lambda * factor * (f.s_d[j] - s_b[j]))
            })
            .collect();
        (lambda, shares)
    }

    /// The miss of the M aggregate at split t.
    fn miss(&self, fixed: &[Fixed], t: f64) -> f64 {
        let (_, shares) = self.categories(fixed, t);
        let spend = |i: usize| self.pops.spend[self.members[i]];
        let total: f64 = (0..fixed.len()).map(spend).sum();
        (0..fixed.len())
            .map(|i| spend(i) * shares[i][1])
            .sum::<f64>()
            - self.cat[1] * total
    }

    /// The split t in [0, 1] at which the M aggregate is met: a bracketed secant (Illinois) to
    /// the last bit, an error after [`MAX_SECANT_STEPS`] steps.
    fn split(&self, fixed: &[Fixed]) -> Result<f64, Failure> {
        let (mut a, mut b) = (0.0, 1.0);
        let (mut fa, mut fb) = (self.miss(fixed, a), self.miss(fixed, b));
        if fa == 0.0 {
            return Ok(a);
        }
        if fb == 0.0 {
            return Ok(b);
        }
        if fa > 0.0 || fb < 0.0 {
            return Err(Negative);
        }
        let mut side = 0;
        for _ in 0..MAX_SECANT_STEPS {
            let t = (a * fb - b * fa) / (fb - fa);
            let t = if t > a && t < b { t } else { 0.5 * (a + b) };
            if t <= a || t >= b {
                return Ok(if -fa <= fb { a } else { b });
            }
            let ft = self.miss(fixed, t);
            if ft == 0.0 {
                return Ok(t);
            }
            if ft < 0.0 {
                (a, fa) = (t, ft);
                if side == -1 {
                    fb /= 2.0;
                }
                side = -1;
            } else {
                (b, fb) = (t, ft);
                if side == 1 {
                    fa /= 2.0;
                }
                side = 1;
            }
        }
        Err(SplitUnsettled)
    }

    /// (lambda, t, alpha) at which every aggregate good share is Omega.
    fn solve(&self) -> Result<(f64, f64, Vec<f64>), Failure> {
        let n_g = self.om.len();
        let mut alpha: Vec<f64> = (0..n_g)
            .map(|g| {
                if self.om[g] > 0.0 {
                    self.om[g] / self.cat[self.category[g]]
                } else {
                    0.0
                }
            })
            .collect();
        let spend = |i: usize| self.pops.spend[self.members[i]];
        let mut best = f64::INFINITY;
        let mut stalled = 0;
        for _ in 0..MAX_ROUNDS {
            let fixed = self.fixed(&alpha);
            let t = self.split(&fixed)?;
            let (lambda, shares) = self.categories(&fixed, t);
            if !(lambda.is_finite() && lambda >= 0.0) {
                return Err(Negative);
            }
            let total: f64 = (0..fixed.len()).map(spend).sum();
            let mut model = vec![0.0; n_g];
            for (i, f) in fixed.iter().enumerate() {
                for g in 0..n_g {
                    model[g] += spend(i) * shares[i][self.category[g]] * f.within[g];
                }
            }
            let miss = (0..n_g)
                .filter(|&g| self.om[g] > 0.0)
                .map(|g| (model[g] / total / self.om[g] - 1.0).abs())
                .fold(0.0f64, f64::max);
            if miss <= SHARE_TOL || (miss <= 1e-12 && stalled >= 20) {
                return Ok((lambda, t, alpha));
            }
            if miss < best {
                (best, stalled) = (miss, 0);
            } else {
                stalled += 1;
            }
            for g in (0..n_g).filter(|&g| self.om[g] > 0.0) {
                alpha[g] *= self.om[g] * total / model[g];
            }
            let mut sum = [0.0; N_CATEGORIES];
            for g in 0..n_g {
                sum[self.category[g]] += alpha[g];
            }
            for g in 0..n_g {
                if alpha[g] > 0.0 {
                    alpha[g] /= sum[self.category[g]];
                }
            }
        }
        Err(Unsettled(best))
    }
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
