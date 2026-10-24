//! PIGL final demand (ADR 0021; `specs/econ/SPEC.md`, Final demand): the multi-sector PIGL of
//! Alder, Boppart and Mueller (2022) across three categories, a CES over goods within each
//! category, and a pop's lognormal income integrated in closed form through E[y^(1 - eps)].

use ceteris_trade::N_GOODS;

use crate::params::EconParams;

pub const N_CATEGORIES: usize = 3;
/// Category of each good: A (0) agriculture and food, M (1) crude oil, mining and
/// manufactures, S (2) utilities, construction and services.
pub const CATEGORY: [usize; N_GOODS] = [0, 1, 1, 0, 1, 1, 1, 2, 2, 2];

/// Preference weights of one country, backed out by [`invert_pigl`].
#[derive(Clone, Debug, PartialEq)]
pub struct PiglWeights {
    /// lambda = nu / kappa, the scale of the income-effect term.
    pub scale: f64,
    /// Weights omega_c of B(P); omega_A = 0.
    pub omega: [f64; N_CATEGORIES],
    /// Within-category CES weights alpha_g, summing to 1 over each category with demand.
    pub within: [f64; N_GOODS],
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
fn category_prices(
    w: &PiglWeights,
    price: &[f64; N_GOODS],
    sigma: f64,
) -> ([f64; N_CATEGORIES], [f64; N_GOODS]) {
    let mut term = [0.0; N_GOODS];
    let mut sum = [0.0; N_CATEGORIES];
    for g in (0..N_GOODS).filter(|&g| w.within[g] > 0.0 && price[g].is_finite()) {
        term[g] = w.within[g] * libm::pow(price[g], 1.0 - sigma);
        sum[CATEGORY[g]] += term[g];
    }
    let index = sum.map(|s| {
        if s > 0.0 {
            libm::pow(s, 1.0 / (1.0 - sigma))
        } else {
            f64::INFINITY
        }
    });
    let mut within = [0.0; N_GOODS];
    for g in (0..N_GOODS).filter(|&g| term[g] > 0.0) {
        within[g] = term[g] / sum[CATEGORY[g]];
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

/// Category budget shares of a pop before the clamp, eq. (22) of Alder, Boppart and Mueller
/// (2022) for a pop with lognormal income:
/// s_c = sB_c + lambda (Dt / B)^gamma (sD_c - sB_c) (m / B)^-eps exp(-eps (1 - eps) s^2 / 2),
/// with sB and sD the category shares of B(P) and Dtilde(P).
fn raw_shares(
    w: &PiglWeights,
    index: &[f64; N_CATEGORIES],
    income: f64,
    sigma_income: f64,
    p: &EconParams,
) -> [f64; N_CATEGORIES] {
    let q = &p.pigl;
    let (dt, s_d) = aggregate(&q.theta, index, q.phi);
    if !(0..N_CATEGORIES).any(|c| w.omega[c] > 0.0 && index[c].is_finite()) {
        // no reachable category in B(P): the income-effect bundle is all there is
        return s_d;
    }
    let (b, s_b) = aggregate(&w.omega, index, q.sigma_b);
    let eps = q.eps;
    let k = w.scale
        * libm::pow(dt / b, q.gamma)
        * libm::pow(income / b, -eps)
        * libm::exp(-eps * (1.0 - eps) * sigma_income * sigma_income / 2.0);
    let mut out = [0.0; N_CATEGORIES];
    for c in (0..N_CATEGORIES).filter(|&c| index[c].is_finite()) {
        out[c] = s_b[c] + k * (s_d[c] - s_b[c]);
    }
    out
}

/// Budget shares by good of a pop with mean income `income` per person and log income
/// dispersion `sigma_income`, at buyer prices `price`. Category shares below 0 (a pop far
/// from its country's base) are set to 0 and the rest rescaled to sum to 1 (Review Focus 4);
/// goods without a weight or at an infinite price get 0.
pub fn pigl_shares(
    w: &PiglWeights,
    price: &[f64; N_GOODS],
    income: f64,
    sigma_income: f64,
    p: &EconParams,
    out: &mut [f64; N_GOODS],
) {
    let (index, within) = category_prices(w, price, p.sigma_demand);
    let raw = raw_shares(w, &index, income, sigma_income, p).map(|s| s.max(0.0));
    let total: f64 = raw.iter().sum();
    for g in 0..N_GOODS {
        out[g] = raw[CATEGORY[g]] / total * within[g];
    }
}

/// Quantities bought per person by a pop whose mean spending per person is `money`: the
/// shares of [`pigl_shares`] at income `money`, so spending adds up to `money`. A
/// nonpositive balance buys nothing.
pub fn pigl(
    w: &PiglWeights,
    price: &[f64; N_GOODS],
    money: f64,
    sigma_income: f64,
    p: &EconParams,
    out: &mut [f64; N_GOODS],
) {
    *out = [0.0; N_GOODS];
    if money <= 0.0 {
        return;
    }
    let mut share = [0.0; N_GOODS];
    pigl_shares(w, price, money, sigma_income, p, &mut share);
    for g in (0..N_GOODS).filter(|&g| share[g] > 0.0) {
        out[g] = share[g] * money / price[g];
    }
}

/// The weights that reproduce the final demand shares `omega` (normalized) at prices 1 and
/// mean spending per person `base_spend`: within-category weights alpha_g = Omega_g / Omega_c;
/// with omega_A = 0, category shares Omega_c = (1 - z) omega_c + z theta_c give
/// z = Omega_A / theta_A, omega_c = (Omega_c - z theta_c) / (1 - z) and
/// lambda = z base_spend^eps exp(eps (1 - eps) s^2 / 2). Panics where a weight would be
/// negative (z >= 1 or Omega_c < z theta_c): the country's shares are outside the form.
pub fn invert_pigl(
    omega: &[f64; N_GOODS],
    base_spend: f64,
    sigma_income: f64,
    p: &EconParams,
) -> PiglWeights {
    let q = &p.pigl;
    let total: f64 = omega.iter().sum();
    let mut cat = [0.0; N_CATEGORIES];
    for g in 0..N_GOODS {
        cat[CATEGORY[g]] += omega[g] / total;
    }
    let mut within = [0.0; N_GOODS];
    for g in (0..N_GOODS).filter(|&g| omega[g] > 0.0) {
        within[g] = omega[g] / total / cat[CATEGORY[g]];
    }
    let z = cat[0] / q.theta[0];
    let mut weight = [0.0; N_CATEGORIES];
    for c in 1..N_CATEGORIES {
        weight[c] = (cat[c] - z * q.theta[c]) / (1.0 - z);
    }
    assert!(
        z < 1.0 && weight.iter().all(|&o| o >= 0.0),
        "PIGL inversion gives a negative weight: category shares {cat:?}, z {z}, omega {weight:?}"
    );
    let eps = q.eps;
    PiglWeights {
        scale: z
            * libm::pow(base_spend, eps)
            * libm::exp(eps * (1.0 - eps) * sigma_income * sigma_income / 2.0),
        omega: weight,
        within,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::small_world;
    use crate::inputs::EconInputs;
    use crate::world::base_income;

    const OMEGA: [f64; N_GOODS] = [0.06, 0.0, 0.01, 0.12, 0.08, 0.04, 0.07, 0.04, 0.10, 0.48];
    const PRICES: [f64; N_GOODS] = [1.0, 3.0, 0.7, 2.2, 1.4, 5.0, 0.9, 1.1, 2.0, 0.5];
    const BASE: f64 = 2.0;
    const SIGMA: f64 = 0.7;

    fn weights() -> PiglWeights {
        invert_pigl(&OMEGA, BASE, SIGMA, &EconParams::default())
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
                let want = cat[CATEGORY[g]] * within[g] / PRICES[g];
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
            let w = invert_pigl(&inp.countries.omega[c], spend / people, sigma, &p);
            pigl_shares(&w, &[1.0; N_GOODS], spend / people, sigma, &p, &mut s);
            for (g, (got, want)) in s.iter().zip(&inp.countries.omega[c]).enumerate() {
                assert!((got - want).abs() <= 1e-12, "country {c} good {g}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "negative")]
    fn inversion_refuses_a_negative_weight() {
        // services below z theta_S: omega_S would be negative
        let omega = [0.30, 0.0, 0.0, 0.30, 0.3, 0.0, 0.0, 0.0, 0.0, 0.1];
        invert_pigl(&omega, BASE, SIGMA, &EconParams::default());
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
            let w = invert_pigl(&inp.countries.omega[c], spend / people, sigma, &p);
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
