//! PIGL final demand (ADR 0021; `specs/econ/SPEC.md`, Final demand): the multi-sector PIGL of
//! Alder, Boppart and Mueller (2022) across three categories, a CES over goods within each
//! category, and a pop's lognormal income integrated in closed form through E[y^(1 - eps)].

use crate::params::EconParams;

/// Categories A (0) agriculture and food, M (1) crude oil, mining and manufactures, S (2)
/// utilities, construction and services; each good's is `Goods::category`.
pub const N_CATEGORIES: usize = 3;

/// Preference weights of one country, backed out by [`invert_pigl`].
#[derive(Clone, Debug, PartialEq)]
pub struct PiglWeights {
    /// lambda = nu / kappa, the scale of the income-effect term.
    pub scale: f64,
    /// Weights omega_c of B(P); 0 for the category that binds the income-effect weight, A
    /// wherever omega_A = 0 fits (ADR 0313).
    pub omega: [f64; N_CATEGORIES],
    /// Within-category CES weights alpha_g, summing to 1 over each category with demand.
    pub within: Vec<f64>,
    /// Category of each good (`Goods::category`), which `within` sums over.
    pub category: Vec<u8>,
    /// Within-pop log income dispersion s_w the weights were inverted with, which every pop of
    /// the country uses (ADR 0094).
    pub sigma: f64,
    /// Not PIGL: government's and investment's fixed shares of a pop's spending by good (ADR
    /// 0099, RFC 0301), so households spend [`PiglWeights::household_share`] of it by PIGL.
    /// Empty where households buy all final demand.
    pub gov: Vec<f64>,
    pub inv: Vec<f64>,
}

impl PiglWeights {
    /// The share of a pop's spending its households spend by PIGL: 1 less government's and
    /// investment's.
    pub fn household_share(&self) -> f64 {
        1.0 - self.gov.iter().sum::<f64>() - self.inv.iter().sum::<f64>()
    }
}

/// Adds to `out` the quantities that `money` buys at fixed spending shares `share` (ADR 0099,
/// ADR 0305): each good with a share and a finite price gets its share of `money`, the
/// shares of goods with no reachable origin spread over the others in proportion.
pub fn fixed_shares(share: &[f64], price: &[f64], money: f64, out: &mut [f64]) {
    let reachable = |g: &usize| share[*g] > 0.0 && price[*g].is_finite();
    let total: f64 = share.iter().sum();
    let reached: f64 = (0..share.len()).filter(reachable).map(|g| share[g]).sum();
    if reached <= 0.0 {
        return;
    }
    for g in (0..share.len()).filter(reachable) {
        out[g] += money * total * (share[g] / reached) / price[g];
    }
}

/// Log standard deviation of a lognormal with Gini `gini`: sqrt(2) Phi^-1((G + 1) / 2), which
/// is 2 erfinv(G). Newton on `libm::erf` from Winitzki's approximation, a fixed number of
/// steps.
pub fn lognormal_sigma(gini: f64) -> f64 {
    assert!((0.0..1.0).contains(&gini), "Gini {gini}");
    if gini == 0.0 {
        return 0.0;
    }
    let a = 0.147;
    let l = libm::log(1.0 - gini * gini);
    let t = 2.0 / (std::f64::consts::PI * a) + l / 2.0;
    let mut x = libm::sqrt(libm::sqrt(t * t - l / a) - t);
    for _ in 0..6 {
        let slope = std::f64::consts::FRAC_2_SQRT_PI * libm::exp(-x * x);
        x -= (libm::erf(x) - gini) / slope;
    }
    2.0 * x
}

/// Category price indexes P_c = (sum_g alpha_g p_g^(1 - sigma))^(1 / (1 - sigma)) over the
/// category's goods with a weight and a finite price (infinite where there is none), and each
/// good's share of its category's spending.
fn category_prices(w: &PiglWeights, price: &[f64], sigma: f64) -> ([f64; N_CATEGORIES], Vec<f64>) {
    let n = w.within.len();
    let mut term = vec![0.0; n];
    let mut sum = [0.0; N_CATEGORIES];
    for g in (0..n).filter(|&g| w.within[g] > 0.0 && price[g].is_finite()) {
        term[g] = w.within[g] * libm::pow(price[g], 1.0 - sigma);
        sum[usize::from(w.category[g])] += term[g];
    }
    let index = sum.map(|s| {
        if s > 0.0 {
            libm::pow(s, 1.0 / (1.0 - sigma))
        } else {
            f64::INFINITY
        }
    });
    let mut within = vec![0.0; n];
    for g in (0..n).filter(|&g| term[g] > 0.0) {
        within[g] = term[g] / sum[usize::from(w.category[g])];
    }
    (index, within)
}

/// CES price aggregator over the categories with a finite index, weights renormalized over
/// them: the aggregate and each category's share of it.
fn aggregate(
    weight: &[f64; N_CATEGORIES],
    index: &[f64; N_CATEGORIES],
    sigma: f64,
) -> (f64, [f64; N_CATEGORIES]) {
    let mut term = [0.0; N_CATEGORIES];
    for c in (0..N_CATEGORIES).filter(|&c| weight[c] > 0.0 && index[c].is_finite()) {
        term[c] = weight[c] * libm::pow(index[c], 1.0 - sigma);
    }
    let sum: f64 = term.iter().sum();
    let mass: f64 = (0..N_CATEGORIES)
        .filter(|&c| term[c] > 0.0)
        .map(|c| weight[c])
        .sum();
    (
        libm::pow(sum / mass, 1.0 / (1.0 - sigma)),
        term.map(|t| t / sum),
    )
}

/// The parts of [`raw_shares`] that depend on the category indexes alone, shared by every
/// pop that faces them: the aggregates Dtilde(P) and B(P) with their category shares, and
/// `scale (Dt / B)^gamma`. `b` is `None` where no category of B(P) is reachable.
struct CategoryAt {
    s_d: [f64; N_CATEGORIES],
    b: Option<(f64, [f64; N_CATEGORIES], f64)>,
}

impl CategoryAt {
    fn new(w: &PiglWeights, index: &[f64; N_CATEGORIES], p: &EconParams) -> Self {
        let q = &p.pigl;
        let (dt, s_d) = aggregate(&q.theta, index, q.phi);
        if !(0..N_CATEGORIES).any(|c| w.omega[c] > 0.0 && index[c].is_finite()) {
            return Self { s_d, b: None };
        }
        let (b, s_b) = aggregate(&w.omega, index, q.sigma_b);
        let head = w.scale * libm::pow(dt / b, q.gamma);
        Self {
            s_d,
            b: Some((b, s_b, head)),
        }
    }

    /// Category budget shares of a pop before the clamp, eq. (22) of Alder, Boppart and
    /// Mueller (2022) for a pop with lognormal income:
    /// s_c = sB_c + lambda (Dt / B)^gamma (sD_c - sB_c) (m / B)^-eps exp(-eps (1 - eps) s^2 / 2),
    /// with sB and sD the category shares of B(P) and Dtilde(P).
    fn raw(
        &self,
        index: &[f64; N_CATEGORIES],
        income: f64,
        sigma_income: f64,
        p: &EconParams,
    ) -> [f64; N_CATEGORIES] {
        let Some((b, s_b, head)) = self.b else {
            // no reachable category in B(P): the income-effect bundle is all there is
            return self.s_d;
        };
        let eps = p.pigl.eps;
        let k = head
            * libm::pow(income / b, -eps)
            * libm::exp(-eps * (1.0 - eps) * sigma_income * sigma_income / 2.0);
        let mut out = [0.0; N_CATEGORIES];
        for c in (0..N_CATEGORIES).filter(|&c| index[c].is_finite()) {
            out[c] = s_b[c] + k * (self.s_d[c] - s_b[c]);
        }
        out
    }
}

/// Category budget shares of a pop before the clamp ([`CategoryAt::raw`]).
#[cfg(test)]
fn raw_shares(
    w: &PiglWeights,
    index: &[f64; N_CATEGORIES],
    income: f64,
    sigma_income: f64,
    p: &EconParams,
) -> [f64; N_CATEGORIES] {
    CategoryAt::new(w, index, p).raw(index, income, sigma_income, p)
}

/// Everything of a pop's PIGL demand that depends on its country's weights and the prices it
/// faces alone, so that the pops of one province compute it once ([`PiglAt::shares`]).
pub struct PiglAt {
    index: [f64; N_CATEGORIES],
    within: Vec<f64>,
    categories: CategoryAt,
}

impl PiglAt {
    pub fn new(w: &PiglWeights, price: &[f64], p: &EconParams) -> Self {
        let (index, within) = category_prices(w, price, p.sigma_demand);
        let categories = CategoryAt::new(w, &index, p);
        Self {
            index,
            within,
            categories,
        }
    }

    /// [`pigl_shares`] at these prices.
    pub fn shares(
        &self,
        w: &PiglWeights,
        income: f64,
        sigma_income: f64,
        p: &EconParams,
        out: &mut [f64],
    ) {
        let raw = self
            .categories
            .raw(&self.index, income, sigma_income, p)
            .map(|s| s.max(0.0));
        let total: f64 = raw.iter().sum();
        for (g, o) in out.iter_mut().enumerate() {
            *o = raw[usize::from(w.category[g])] / total * self.within[g];
        }
    }

    /// [`pigl`] at these prices.
    pub fn demand(
        &self,
        w: &PiglWeights,
        price: &[f64],
        money: f64,
        sigma_income: f64,
        p: &EconParams,
        out: &mut [f64],
    ) {
        out.fill(0.0);
        if money <= 0.0 {
            return;
        }
        let mut share = vec![0.0; out.len()];
        self.shares(w, money, sigma_income, p, &mut share);
        for g in (0..out.len()).filter(|&g| share[g] > 0.0) {
            out[g] = share[g] * money / price[g];
        }
    }
}

/// Budget shares by good of a pop with mean income `income` per person and log income
/// dispersion `sigma_income`, at buyer prices `price`. Category shares below 0 (a pop far
/// from its country's base, such as the fixture's island pops) are set to 0 and the rest
/// rescaled to sum to 1; goods without a weight or at an infinite price get 0.
pub fn pigl_shares(
    w: &PiglWeights,
    price: &[f64],
    income: f64,
    sigma_income: f64,
    p: &EconParams,
    out: &mut [f64],
) {
    PiglAt::new(w, price, p).shares(w, income, sigma_income, p, out);
}

/// Quantities bought per person by a pop whose mean spending per person is `money`: the
/// shares of [`pigl_shares`] at income `money`, so spending adds up to `money`. A
/// nonpositive balance buys nothing.
pub fn pigl(
    w: &PiglWeights,
    price: &[f64],
    money: f64,
    sigma_income: f64,
    p: &EconParams,
    out: &mut [f64],
) {
    PiglAt::new(w, price, p).demand(w, price, money, sigma_income, p, out);
}

/// The weights that reproduce the final demand shares `omega` (normalized) at prices 1 and
/// mean spending per person `base_spend`: within-category weights alpha_g = Omega_g / Omega_c;
/// category shares Omega_c = (1 - z) omega_c + z theta_c with the largest income-effect weight
/// at which every omega_c >= 0, z = min_c Omega_c / theta_c (ADR 0313; the binding category,
/// A on a tie, gets omega 0, so omega_A = 0 wherever that fits), omega_c = (Omega_c - z
/// theta_c) / (1 - z) and lambda = z base_spend^eps exp(eps (1 - eps) s^2 / 2). Panics where
/// z >= 1 (Omega = theta). `category` is each good's (`Goods::category`).
pub fn invert_pigl(
    omega: &[f64],
    category: &[u8],
    base_spend: f64,
    sigma_income: f64,
    p: &EconParams,
) -> PiglWeights {
    let q = &p.pigl;
    let total: f64 = omega.iter().sum();
    let n = omega.len();
    let of = |g: usize| usize::from(category[g]);
    let mut cat = [0.0; N_CATEGORIES];
    for g in 0..n {
        cat[of(g)] += omega[g] / total;
    }
    let mut within = vec![0.0; n];
    for g in (0..n).filter(|&g| omega[g] > 0.0) {
        within[g] = omega[g] / total / cat[of(g)];
    }
    let bind = binding(&cat, &q.theta);
    let z = cat[bind] / q.theta[bind];
    assert!(
        z < 1.0,
        "PIGL inversion gives no weight: category shares {cat:?} are theta"
    );
    let mut weight = [0.0; N_CATEGORIES];
    for c in (0..N_CATEGORIES).filter(|&c| c != bind) {
        weight[c] = ((cat[c] - z * q.theta[c]) / (1.0 - z)).max(0.0);
    }
    let eps = q.eps;
    PiglWeights {
        scale: z
            * libm::pow(base_spend, eps)
            * libm::exp(eps * (1.0 - eps) * sigma_income * sigma_income / 2.0),
        omega: weight,
        within,
        category: category.to_vec(),
        sigma: sigma_income,
        gov: Vec::new(),
        inv: Vec::new(),
    }
}

/// The category whose Omega_c / theta_c is least, the first on a tie (ADR 0313).
fn binding(cat: &[f64; N_CATEGORIES], theta: &[f64; N_CATEGORIES]) -> usize {
    (1..N_CATEGORIES).fold(0, |b, c| {
        if cat[c] * theta[b] < cat[b] * theta[c] {
            c
        } else {
            b
        }
    })
}

/// ADR 0094: a country's within-pop dispersion s_w = sqrt(max(0, s_nat^2 - Var ln m)), s_nat
/// = [`lognormal_sigma`] of its Gini and the variance over its pops (people, income per person)
/// weighted by people, pops without people or income left out; s_nat without any.
pub fn within_sigma(gini: f64, pops: &[(f64, f64)]) -> f64 {
    let national = lognormal_sigma(gini);
    let counted = || pops.iter().filter(|&&(n, m)| n > 0.0 && m > 0.0);
    let people: f64 = counted().map(|&(n, _)| n).sum();
    if people <= 0.0 {
        return national;
    }
    let mean = counted().map(|&(n, m)| n * libm::log(m)).sum::<f64>() / people;
    let var = counted()
        .map(|&(n, m)| {
            let d = libm::log(m) - mean;
            n * d * d
        })
        .sum::<f64>()
        / people;
    libm::sqrt((national * national - var).max(0.0))
}

/// One pop of a country in [`invert_pigl_pops`].
#[derive(Clone, Debug)]
pub struct PopAt {
    pub price: Vec<f64>,
    /// Mean income per person.
    pub income: f64,
    /// Spending, the pop's weight in its country's aggregate.
    pub spend: f64,
}

/// Rounds of [`invert_pigl_pops`] before it panics.
const MAX_ROUNDS: usize = 1000;
/// Secant steps of one split in [`invert_pigl_pops`] before it panics.
const MAX_SECANT_STEPS: usize = 400;
/// Doublings of lambda's bracket in [`invert_pigl_pops`] where shares are clamped (ADR 0316).
const MAX_DOUBLINGS: usize = 64;

/// A pop at the current alpha: category indexes, shares within them, sD and the factor of
/// lambda without B, Dt^gamma m^-eps exp(-eps (1 - eps) s^2 / 2).
type Fixed = ([f64; N_CATEGORIES], Vec<f64>, [f64; N_CATEGORIES], f64);

/// ADR 0094, Amendment: the weights at which the PIGL shares of a country's pops (each at its
/// prices and income, with dispersion `sigma`), weighted by their spending, are its final demand
/// shares `omega` (normalized). From alpha_g = Omega_g / Omega_c, rounds of: the split t =
/// omega_M of omega_M + omega_S = 1 that meets the M aggregate (bracketed secant), with lambda
/// from the food aggregate in closed form (omega_A = 0); where M (S) binds instead, omega_M
/// (omega_S) = 0, lambda from its aggregate and t = omega_A meets food (ADR 0313), on the shares
/// [`pigl_shares`] spends, a negative one at 0 (ADR 0316); then alpha_g times Omega_g over its
/// aggregate share, renormalized within its category; until every aggregate share is within
/// 1e-14 of Omega (or, below 1e-12, has stopped improving). Pops without spending are left
/// out; without any, [`invert_pigl`] at spending 0. Panics where no split meets the aggregates or
/// a split takes more than [`MAX_SECANT_STEPS`] secant steps. `category` is each good's
/// (`Goods::category`).
pub fn invert_pigl_pops(
    omega: &[f64],
    category: &[u8],
    pops: &[PopAt],
    sigma: f64,
    p: &EconParams,
) -> PiglWeights {
    let pops: Vec<PopAt> = pops.iter().filter(|k| k.spend > 0.0).cloned().collect();
    if pops.is_empty() {
        return invert_pigl(omega, category, 0.0, sigma, p);
    }
    let n = omega.len();
    let of = |g: usize| usize::from(category[g]);
    let total: f64 = omega.iter().sum();
    let share: Vec<f64> = omega.iter().map(|o| o / total).collect();
    let mut cat = [0.0; N_CATEGORIES];
    for g in 0..n {
        cat[of(g)] += share[g];
    }
    let mut w = PiglWeights {
        scale: 0.0,
        omega: [0.0; N_CATEGORIES],
        within: (0..n)
            .map(|g| {
                if share[g] > 0.0 {
                    share[g] / cat[of(g)]
                } else {
                    0.0
                }
            })
            .collect(),
        category: category.to_vec(),
        sigma,
        gov: Vec::new(),
        inv: Vec::new(),
    };
    let spent: f64 = pops.iter().map(|k| k.spend).sum();
    let q = &p.pigl;
    let eps = q.eps;
    let negative = || -> ! {
        panic!(
            "PIGL inversion gives a negative weight: Omega {share:?} over {} pops",
            pops.len()
        )
    };
    let (mut best, mut stalled) = (f64::INFINITY, 0);
    // the categories other than the binding one b, whose weights are t and 1 - t
    let others = |b: usize| -> (usize, usize) {
        match b {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        }
    };
    // lambda (from the b aggregate, omega_b = 0) and every pop's category shares at the split t,
    // negative ones set to 0 and the rest rescaled as `pigl_shares` does: lambda in closed form
    // where no pop's share is negative there, else by a bracketed secant (ADR 0316)
    let at = |fixed: &[Fixed], b: usize, t: f64| -> (f64, Vec<[f64; N_CATEGORIES]>) {
        let (o1, o2) = others(b);
        let mut weight = [0.0; N_CATEGORIES];
        (weight[o1], weight[o2]) = (t, 1.0 - t);
        let parts: Vec<([f64; N_CATEGORIES], f64)> = fixed
            .iter()
            .map(|(index, _, _, x)| {
                let (b, s_b) = aggregate(&weight, index, q.sigma_b);
                (s_b, x * libm::pow(b, eps - q.gamma))
            })
            .collect();
        let raw = |lambda: f64| -> Vec<[f64; N_CATEGORIES]> {
            parts
                .iter()
                .zip(fixed)
                .map(|((s_b, f), (_, _, s_d, _))| {
                    std::array::from_fn(|c| s_b[c] + lambda * f * (s_d[c] - s_b[c]))
                })
                .collect()
        };
        let clamped = |lambda: f64| -> Vec<[f64; N_CATEGORIES]> {
            raw(lambda)
                .into_iter()
                .map(|r| {
                    let r = r.map(|s| s.max(0.0));
                    let total: f64 = r.iter().sum();
                    r.map(|s| s / total)
                })
                .collect()
        };
        let bound: f64 = parts
            .iter()
            .zip(fixed)
            .zip(&pops)
            .map(|(((_, f), (_, _, s_d, _)), k)| k.spend * f * s_d[b])
            .sum();
        if cat[b] <= 0.0 {
            return (0.0, raw(0.0));
        }
        let closed = cat[b] * spent / bound;
        let unclamped = raw(closed);
        if unclamped.iter().all(|r| r.iter().all(|&s| s >= 0.0)) {
            return (closed, unclamped);
        }
        let short = |lambda: f64| -> f64 {
            clamped(lambda)
                .iter()
                .zip(&pops)
                .map(|(s, k)| k.spend * s[b])
                .sum::<f64>()
                - cat[b] * spent
        };
        let mut hi = closed;
        for _ in 0..MAX_DOUBLINGS {
            if short(hi) >= 0.0 {
                break;
            }
            hi *= 2.0;
        }
        let lambda = hi * illinois(|u| short(hi * u));
        (lambda, clamped(lambda))
    };
    let miss = |fixed: &[Fixed], b: usize, t: f64| -> f64 {
        let o1 = others(b).0;
        let (_, shares) = at(fixed, b, t);
        shares
            .iter()
            .zip(&pops)
            .map(|(s, k)| k.spend * s[o1])
            .sum::<f64>()
            - cat[o1] * spent
    };
    for _ in 0..MAX_ROUNDS {
        let fixed: Vec<Fixed> = pops
            .iter()
            .map(|k| {
                let (index, within) = category_prices(&w, &k.price, p.sigma_demand);
                let (dt, s_d) = aggregate(&q.theta, &index, q.phi);
                let x = libm::pow(dt, q.gamma)
                    * libm::pow(k.income, -eps)
                    * libm::exp(-eps * (1.0 - eps) * sigma * sigma / 2.0);
                (index, within, s_d, x)
            })
            .collect();
        // omega_A = 0 where it fits; else M (S) binds: its aggregate is above (below) Omega
        // even at omega_M = 0 (omega_S = 0), ADR 0313
        let (fa, fb) = (miss(&fixed, 0, 0.0), miss(&fixed, 0, 1.0));
        let bind = if fa > 0.0 {
            1
        } else if fb < 0.0 {
            2
        } else {
            0
        };
        if bind != 0 && (miss(&fixed, bind, 0.0) > 0.0 || miss(&fixed, bind, 1.0) < 0.0) {
            negative();
        }
        let t = illinois(|t| miss(&fixed, bind, t));
        let (lambda, shares) = at(&fixed, bind, t);
        if !(lambda.is_finite() && lambda >= 0.0) {
            negative();
        }
        w.scale = lambda;
        let (o1, o2) = others(bind);
        w.omega = [0.0; N_CATEGORIES];
        (w.omega[o1], w.omega[o2]) = (t, 1.0 - t);
        let mut model = vec![0.0; n];
        for ((s, (_, within, _, _)), k) in shares.iter().zip(&fixed).zip(&pops) {
            for g in 0..n {
                model[g] += k.spend * s[of(g)] * within[g];
            }
        }
        let miss = (0..n)
            .filter(|&g| share[g] > 0.0)
            .map(|g| (model[g] / spent / share[g] - 1.0).abs())
            .fold(0.0, f64::max);
        if miss <= 1e-14 || (miss <= 1e-12 && stalled >= 20) {
            return w;
        }
        if miss < best {
            (best, stalled) = (miss, 0);
        } else {
            stalled += 1;
        }
        let mut sum = [0.0; N_CATEGORIES];
        for g in (0..n).filter(|&g| share[g] > 0.0) {
            w.within[g] *= share[g] * spent / model[g];
            sum[of(g)] += w.within[g];
        }
        for g in (0..n).filter(|&g| share[g] > 0.0) {
            w.within[g] /= sum[of(g)];
        }
    }
    panic!(
        "PIGL inversion over {} pops did not settle in {MAX_ROUNDS} rounds",
        pops.len()
    );
}

/// The root in [0, 1] of `f`, increasing with f(0) <= 0 <= f(1): a bracketed secant (Illinois:
/// the retained end's value is halved when the same end moves twice) to the last bit, then the
/// end with the smaller miss. Panics after [`MAX_SECANT_STEPS`] steps.
fn illinois(f: impl Fn(f64) -> f64) -> f64 {
    let (mut a, mut b) = (0.0, 1.0);
    let (mut fa, mut fb) = (f(a), f(b));
    let mut last = 0i8;
    let mut steps = 0;
    while fa != 0.0 && fb != 0.0 {
        steps += 1;
        assert!(
            steps <= MAX_SECANT_STEPS,
            "PIGL inversion: the split did not settle in {MAX_SECANT_STEPS} steps"
        );
        let mut t = (a * fb - b * fa) / (fb - fa);
        if !(t > a && t < b) {
            t = 0.5 * (a + b);
        }
        if !(t > a && t < b) {
            break;
        }
        let ft = f(t);
        if ft <= 0.0 {
            (a, fa) = (t, ft);
            if last == 1 {
                fb *= 0.5;
            }
            last = 1;
        } else {
            (b, fb) = (t, ft);
            if last == 2 {
                fa *= 0.5;
            }
            last = 2;
        }
    }
    if fa == 0.0 || -fa <= fb { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::small_world;
    use crate::inputs::EconInputs;
    use crate::world::base_income;
    #[allow(unused_imports)]
    use ceteris_trade::goods::p2::{CATEGORY, N as N_GOODS, TRADED as N_TRADED};

    const OMEGA: [f64; N_GOODS] = [0.06, 0.0, 0.01, 0.12, 0.08, 0.04, 0.07, 0.04, 0.10, 0.48];
    const PRICES: [f64; N_GOODS] = [1.0, 3.0, 0.7, 2.2, 1.4, 5.0, 0.9, 1.1, 2.0, 0.5];
    const BASE: f64 = 2.0;
    const SIGMA: f64 = 0.7;

    fn weights() -> PiglWeights {
        invert_pigl(&OMEGA, &CATEGORY, BASE, SIGMA, &EconParams::default())
    }

    fn spent(price: &[f64; N_GOODS], x: &[f64; N_GOODS]) -> f64 {
        (0..N_GOODS)
            .filter(|&g| x[g] > 0.0)
            .map(|g| price[g] * x[g])
            .sum()
    }

    /// Nodes and weights of `n`-point Gauss-Hermite quadrature (weight exp(-x^2)), by Newton on
    /// the orthonormal Hermite recurrence (Numerical Recipes, gauher).
    fn gauss_hermite(n: usize) -> Vec<(f64, f64)> {
        let pim4 = std::f64::consts::PI.powf(-0.25);
        let nf = n as f64;
        let mut out = vec![(0.0, 0.0); n];
        let mut z = 0.0;
        for i in 0..n.div_ceil(2) {
            z = match i {
                0 => (2.0 * nf + 1.0).sqrt() - 1.85575 * (2.0 * nf + 1.0).powf(-1.0 / 6.0),
                1 => z - 1.14 * nf.powf(0.426) / z,
                2 => 1.86 * z - 0.86 * out[0].0,
                3 => 1.91 * z - 0.91 * out[1].0,
                _ => 2.0 * z - out[i - 2].0,
            };
            let mut pp = 0.0;
            for _ in 0..100 {
                let (mut p1, mut p2) = (pim4, 0.0);
                for j in 0..n {
                    let p3 = p2;
                    p2 = p1;
                    let jf = j as f64;
                    p1 = z * (2.0 / (jf + 1.0)).sqrt() * p2 - (jf / (jf + 1.0)).sqrt() * p3;
                }
                pp = (2.0 * nf).sqrt() * p2;
                let z1 = z;
                z = z1 - p1 / pp;
                if (z - z1).abs() <= 1e-15 * z.abs().max(1.0) {
                    break;
                }
            }
            out[i] = (z, 2.0 / (pp * pp));
            out[n - 1 - i] = (-z, 2.0 / (pp * pp));
        }
        out
    }

    fn country_base(inp: &EconInputs, income: &[f64], c: usize) -> (f64, f64) {
        let spend = (0..inp.n_provinces())
            .filter(|&i| inp.country_of(i) == c)
            .map(|i| income[i])
            .sum();
        let people = (0..inp.n_pops())
            .filter(|&k| inp.country_of(inp.pops.province[k] as usize) == c)
            .map(|k| inp.pops.population[k])
            .sum();
        (spend, people)
    }

    #[test]
    fn gini_maps_to_the_lognormal_sigma() {
        // G = 2 Phi(sigma / sqrt 2) - 1 = erf(sigma / 2)
        for g in [0.05, 0.3, 0.45, 0.62, 0.9] {
            let s = lognormal_sigma(g);
            assert!(
                (libm::erf(s / 2.0) - g).abs() < 1e-15,
                "gini {g}: sigma {s}"
            );
        }
        assert_eq!(lognormal_sigma(0.0), 0.0);
    }

    #[test]
    fn gauss_hermite_integrates_exponentials() {
        // int exp(-x^2) exp(a x) dx = sqrt(pi) exp(a^2 / 4)
        let nodes = gauss_hermite(64);
        for a in [0.0, 0.5, 1.3] {
            let got: f64 = nodes.iter().map(|&(x, w)| w * (a * x).exp()).sum();
            let want = std::f64::consts::PI.sqrt() * (a * a / 4.0).exp();
            assert!((got / want - 1.0).abs() < 1e-13, "a {a}: {got} vs {want}");
        }
    }

    #[test]
    fn pigl_pop_demand_matches_quadrature() {
        let p = EconParams::default();
        let w = weights();
        let nodes = gauss_hermite(64);
        let (index, within) = category_prices(&w, &PRICES, p.sigma_demand);
        for (money, sigma) in [(2.0, 0.7), (1.3, 0.4), (3.5, 0.9)] {
            let raw = raw_shares(&w, &index, money, sigma, &p);
            assert!(raw.iter().all(|&s| s > 0.0), "test point must not clamp");
            let mu = libm::log(money) - sigma * sigma / 2.0;
            let mut cat = [0.0; N_CATEGORIES];
            for &(x, weight) in &nodes {
                let e = libm::exp(mu + std::f64::consts::SQRT_2 * sigma * x);
                let s = raw_shares(&w, &index, e, 0.0, &p);
                for c in 0..N_CATEGORIES {
                    cat[c] += weight / std::f64::consts::PI.sqrt() * s[c] * e;
                }
            }
            let mut got = [0.0; N_GOODS];
            pigl(&w, &PRICES, money, sigma, &p, &mut got);
            for g in 0..N_GOODS {
                let want = cat[usize::from(CATEGORY[g])] * within[g] / PRICES[g];
                assert!(
                    (got[g] - want).abs() <= 1e-10 * want,
                    "good {g}: {} vs {want}",
                    got[g]
                );
            }
        }
    }

    #[test]
    fn pigl_adds_up() {
        let p = EconParams::default();
        let w = weights();
        let mut x = [0.0; N_GOODS];
        let mut unreachable = PRICES;
        unreachable[4] = f64::INFINITY;
        for prices in [PRICES, unreachable, [1.0; N_GOODS]] {
            for money in [0.01, 0.5, 2.0, 7.0, 300.0] {
                pigl(&w, &prices, money, SIGMA, &p, &mut x);
                let s = spent(&prices, &x);
                assert!((s / money - 1.0).abs() <= 1e-14, "money {money}: spent {s}");
                assert!(x.iter().all(|v| v.is_finite() && *v >= 0.0));
            }
        }
        pigl(&w, &unreachable, 2.0, SIGMA, &p, &mut x);
        assert_eq!(x[4], 0.0);
        assert_eq!(x[1], 0.0, "no demand without a weight");
    }

    #[test]
    fn pigl_spends_the_rest_when_a_category_is_unreachable() {
        let p = EconParams::default();
        let w = weights();
        let mut prices = PRICES;
        prices[0] = f64::INFINITY;
        prices[3] = f64::INFINITY;
        let mut x = [0.0; N_GOODS];
        pigl(&w, &prices, 0.5, SIGMA, &p, &mut x);
        assert_eq!((x[0], x[3]), (0.0, 0.0));
        assert!((spent(&prices, &x) / 0.5 - 1.0).abs() <= 1e-14);
    }

    #[test]
    fn pigl_buys_nothing_without_money() {
        let p = EconParams::default();
        let mut x = [1.0; N_GOODS];
        pigl(&weights(), &PRICES, 0.0, SIGMA, &p, &mut x);
        assert_eq!(x, [0.0; N_GOODS]);
        pigl(&weights(), &PRICES, -1.0, SIGMA, &p, &mut x);
        assert_eq!(x, [0.0; N_GOODS]);
    }

    #[test]
    fn pigl_inversion_reproduces_base_shares() {
        let p = EconParams::default();
        let w = weights();
        let total: f64 = OMEGA.iter().sum();
        let mut s = [0.0; N_GOODS];
        pigl_shares(&w, &[1.0; N_GOODS], BASE, SIGMA, &p, &mut s);
        for g in 0..N_GOODS {
            assert!(
                (s[g] - OMEGA[g] / total).abs() <= 1e-12,
                "good {g}: {}",
                s[g]
            );
        }
        assert_eq!(w.omega[0], 0.0);
        assert!(w.scale > 0.0 && w.omega.iter().all(|&o| o >= 0.0));
        let inp = small_world();
        let income = base_income(&inp, &p);
        for c in 0..inp.n_countries() {
            let (spend, people) = country_base(&inp, &income, c);
            let sigma = lognormal_sigma(inp.countries.gini[c]);
            let w = invert_pigl(
                &inp.countries.omega[c],
                &CATEGORY,
                spend / people,
                sigma,
                &p,
            );
            pigl_shares(&w, &[1.0; N_GOODS], spend / people, sigma, &p, &mut s);
            for (g, (got, want)) in s.iter().zip(&inp.countries.omega[c]).enumerate() {
                assert!((got - want).abs() <= 1e-12, "country {c} good {g}");
            }
        }
    }

    /// ADR 0094: the weights the world loads reproduce each country's Omega as the aggregate of
    /// its pops' shares at prices 1, weighted by their base spending; with one pop at the
    /// country's base spending the joint inversion is the closed form.
    #[test]
    fn pigl_pops_inversion_reproduces_base_shares() {
        let p = EconParams::default();
        let inp = small_world();
        let (weights, sigma) = crate::world::household_preferences(&inp, &p);
        let income = crate::world::pop_base_income(&inp, &p);
        let mut spent = vec![[0.0; N_GOODS]; inp.n_countries()];
        for (k, &m) in income.iter().enumerate() {
            let c = inp.country_of(inp.pops.province[k] as usize);
            let mut s = [0.0; N_GOODS];
            pigl_shares(&weights[c], &[1.0; N_GOODS], m, sigma[c], &p, &mut s);
            for g in 0..N_GOODS {
                spent[c][g] += s[g] * m * inp.pops.population[k];
            }
        }
        for (c, row) in spent.iter().enumerate() {
            let total: f64 = row.iter().sum();
            for (g, v) in row.iter().enumerate() {
                let want = inp.countries.omega[c][g];
                assert!((v / total - want).abs() <= 1e-12, "country {c} good {g}");
            }
        }
        let alone = invert_pigl_pops(
            &OMEGA,
            &CATEGORY,
            &[PopAt {
                price: vec![1.0; N_GOODS],
                income: BASE,
                spend: 3.0,
            }],
            SIGMA,
            &p,
        );
        let closed = weights_at(&OMEGA);
        assert!((alone.scale / closed.scale - 1.0).abs() <= 1e-13);
        for (a, b) in alone.omega.iter().zip(&closed.omega) {
            assert!((a - b).abs() <= 1e-14);
        }
        for (a, b) in alone.within.iter().zip(&closed.within) {
            assert!((a - b).abs() <= 1e-14);
        }
    }

    fn weights_at(omega: &[f64; N_GOODS]) -> PiglWeights {
        invert_pigl(omega, &CATEGORY, BASE, SIGMA, &EconParams::default())
    }

    #[test]
    fn within_sigma_subtracts_the_between_pop_variance() {
        let s = lognormal_sigma(0.4);
        // two pops of equal size at incomes e^-0.1 and e^0.1: variance 0.01
        let pops = [(1.0, libm::exp(-0.1)), (1.0, libm::exp(0.1))];
        let w = within_sigma(0.4, &pops);
        assert!((w * w + 0.01 - s * s).abs() <= 1e-14);
        // a spread beyond the national dispersion leaves none within
        assert_eq!(within_sigma(0.05, &[(1.0, 0.1), (1.0, 10.0)]), 0.0);
        assert_eq!(within_sigma(0.4, &[]), s);
    }

    /// ADR 0316: a pop far below its country's base has negative raw category shares, which
    /// `pigl_shares` sets to 0; the inversion meets the country's shares on those clamped shares,
    /// as the run spends them.
    #[test]
    fn inversion_meets_the_shares_with_clamped_pops() {
        let p = EconParams::default();
        let omega = [0.25, 0.0, 0.0, 0.25, 0.01, 0.01, 0.0, 0.0, 0.0, 0.48];
        let pops: Vec<PopAt> = [0.02, 1.0, 4.0]
            .iter()
            .map(|&f| PopAt {
                price: (0..N_GOODS)
                    .map(|g| if g < N_TRADED { 1.0 } else { 0.3 })
                    .collect(),
                income: BASE * f,
                spend: 10.0 * BASE * f,
            })
            .collect();
        let w = invert_pigl_pops(&omega, &CATEGORY, &pops, SIGMA, &p);
        let total: f64 = omega.iter().sum();
        let (mut agg, mut s) = ([0.0; N_GOODS], [0.0; N_GOODS]);
        let mut clamped = false;
        for k in &pops {
            pigl_shares(&w, &k.price, k.income, SIGMA, &p, &mut s);
            let services: f64 = (7..N_GOODS).map(|g| s[g]).sum();
            clamped |= services == 0.0;
            for g in 0..N_GOODS {
                agg[g] += s[g] * k.spend;
            }
        }
        assert!(clamped, "the poorest pop's services share is clamped");
        let spent: f64 = agg.iter().sum();
        for g in (0..N_GOODS).filter(|&g| omega[g] > 0.0) {
            let miss = agg[g] / spent / (omega[g] / total) - 1.0;
            assert!(miss.abs() <= 1e-11, "good {g}: {miss}");
        }
    }

    /// ADR 0313: where omega_A = 0 would leave M or S negative (services below z theta_S; Ghana's
    /// manufactures below z theta_M; food above theta_A), the income-effect weight is the
    /// largest that keeps every omega nonnegative: the binding category's omega is 0, omega_A
    /// takes the rest, and the shares are met, in the closed form and over pops at their own
    /// prices and incomes.
    #[test]
    fn inversion_outside_omega_a_zero_binds_another_category() {
        let p = EconParams::default();
        let low_s = [0.30, 0.0, 0.0, 0.30, 0.3, 0.0, 0.0, 0.0, 0.0, 0.1];
        let low_m = [
            0.23, 0.006, 0.0, 0.21, 0.006, 0.008, 0.0013, 0.003, 0.0001, 0.535,
        ];
        let food = [0.40, 0.0, 0.0, 0.30, 0.1, 0.0, 0.0, 0.0, 0.0, 0.2];
        for (omega, c) in [(low_s, 2), (low_m, 1), (food, 2)] {
            let w = weights_at(&omega);
            assert_eq!(w.omega[c], 0.0);
            assert!(w.omega[0] > 0.0 && w.scale > 0.0);
            assert!((w.omega.iter().sum::<f64>() - 1.0).abs() <= 1e-15);
            let total: f64 = omega.iter().sum();
            let mut s = [0.0; N_GOODS];
            pigl_shares(&w, &[1.0; N_GOODS], BASE, SIGMA, &p, &mut s);
            for g in 0..N_GOODS {
                assert!((s[g] - omega[g] / total).abs() <= 1e-12, "good {g}");
            }
            let pops: Vec<PopAt> = (0..4)
                .map(|k| PopAt {
                    price: (0..N_GOODS)
                        .map(|g| {
                            if g < N_TRADED {
                                1.0
                            } else {
                                0.6 + 0.1 * k as f64
                            }
                        })
                        .collect(),
                    income: BASE * (1.0 + 0.2 * k as f64),
                    spend: 10.0 - k as f64,
                })
                .collect();
            let w = invert_pigl_pops(&omega, &CATEGORY, &pops, SIGMA, &p);
            assert!(w.omega[c] == 0.0 && w.omega[0] > 0.0);
            let mut agg = [0.0; N_GOODS];
            for k in &pops {
                pigl_shares(&w, &k.price, k.income, SIGMA, &p, &mut s);
                for g in 0..N_GOODS {
                    agg[g] += s[g] * k.spend;
                }
            }
            let spent: f64 = agg.iter().sum();
            for g in (0..N_GOODS).filter(|&g| omega[g] > 0.0) {
                let miss = agg[g] / spent / (omega[g] / total) - 1.0;
                assert!(miss.abs() <= 1e-11, "good {g}: {miss}");
            }
        }
    }

    #[test]
    fn engel_curves_on_the_fixture() {
        let p = EconParams::default();
        let inp = small_world();
        let income = base_income(&inp, &p);
        let supply = inp.labour_supply(&p.edu_eff);
        let (food, services) = (3, 9);
        for c in 0..inp.n_countries() {
            let (spend, people) = country_base(&inp, &income, c);
            let sigma = lognormal_sigma(inp.countries.gini[c]);
            let w = invert_pigl(
                &inp.countries.omega[c],
                &CATEGORY,
                spend / people,
                sigma,
                &p,
            );
            // base income per person of each pop: its effective labour share of the province
            let mut pops: Vec<(f64, [f64; N_GOODS])> = (0..inp.n_pops())
                .filter(|&k| inp.country_of(inp.pops.province[k] as usize) == c)
                .map(|k| {
                    let i = inp.pops.province[k] as usize;
                    let eff = p.edu_eff[usize::from(inp.pops.edu[k])] * inp.pops.labour_force[k];
                    let m = eff / supply[i] * income[i] / inp.pops.population[k];
                    let mut s = [0.0; N_GOODS];
                    pigl_shares(&w, &[1.0; N_GOODS], m, sigma, &p, &mut s);
                    (m, s)
                })
                .collect();
            pops.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert!(
                pops[0].0 < 0.5 * pops[pops.len() - 1].0,
                "too little income spread"
            );
            for pair in pops.windows(2) {
                let ((m0, s0), (m1, s1)) = (pair[0], pair[1]);
                if m1 > m0 * (1.0 + 1e-9) {
                    assert!(s1[food] < s0[food], "country {c}: food share rises at {m1}");
                    assert!(
                        s1[services] > s0[services],
                        "country {c}: services fall at {m1}"
                    );
                }
            }
        }
    }

    #[test]
    fn pigl_shares_stay_in_unit_interval_far_from_base() {
        let p = EconParams::default();
        let w = weights();
        let index = category_prices(&w, &PRICES, p.sigma_demand).0;
        let mut s = [0.0; N_GOODS];
        let mut x = [0.0; N_GOODS];
        for factor in [1e-3, 1e3] {
            let money = BASE * factor;
            // the poor pop's raw shares leave [0, 1]; the rich one's tend to omega
            let raw = raw_shares(&w, &index, money, SIGMA, &p);
            let outside = raw.iter().any(|&r| !(0.0..=1.0).contains(&r));
            assert_eq!(outside, factor < 1.0, "raw {raw:?}");
            pigl_shares(&w, &PRICES, money, SIGMA, &p, &mut s);
            assert!(s.iter().all(|&v| (0.0..=1.0).contains(&v)), "{s:?}");
            assert!((s.iter().sum::<f64>() - 1.0).abs() <= 1e-14);
            pigl(&w, &PRICES, money, SIGMA, &p, &mut x);
            assert!((spent(&PRICES, &x) / money - 1.0).abs() <= 1e-14);
        }
    }

    #[test]
    fn homogeneity_of_degree_zero() {
        let p = EconParams::default();
        let w = weights();
        let (mut a, mut b) = ([0.0; N_GOODS], [0.0; N_GOODS]);
        for money in [0.002, 2.0, 2000.0] {
            pigl(&w, &PRICES, money, SIGMA, &p, &mut a);
            pigl(&w, &PRICES.map(|q| 3.7 * q), 3.7 * money, SIGMA, &p, &mut b);
            for g in 0..N_GOODS {
                assert!(
                    (a[g] - b[g]).abs() <= 1e-12 * a[g],
                    "money {money} good {g}"
                );
            }
        }
    }
}
