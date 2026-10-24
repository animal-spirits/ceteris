//! Employment adjustment, wage rule and the Walrasian clearing wage.

use ceteris_core::reduce::fixed_sum;

use crate::params::EconParams;

/// Doublings or halvings of the bracketing search, and iterations of the root finder.
const MAX_BRACKET: usize = 200;
const MAX_ITER: usize = 200;
/// Relative tolerance on the wage.
const TOL: f64 = 1e-13;

/// P39, P40: dlnw = clip(pi^e + kappa_w x, -c_max, c_max), scaled by (1 - delta_w) when
/// negative: downward rigidity applies to nominal cuts (ADR 0019). `excess` is
/// x_i = (sum L* - L^s) / L^s, `expected_inflation` pi^e. Returns the new wage.
pub fn wage_step(w: f64, excess: f64, expected_inflation: f64, p: &EconParams, c_max: f64) -> f64 {
    let mut dlnw = (expected_inflation + p.kappa_w * excess).clamp(-c_max, c_max);
    if dlnw < 0.0 {
        dlnw *= 1.0 - p.delta_w;
    }
    w * libm::exp(dlnw)
}

/// Wage at which `demand(w) = l_supply`, `demand` being decreasing in w.
///
/// Brackets the root in ln w by doubling or halving from `w_guess`, then runs a Newton
/// iteration with a central-difference derivative, falling back to bisection whenever the
/// step leaves the bracket or is not finite. Stops at relative tolerance 1e-13 on w.
///
/// With no root the search runs out after 200 doublings or halvings and returns the limit
/// reached: the lowest wage if demand is below supply everywhere (for example all value-added
/// prices nonpositive, so the caller sees zero employment), the highest if above everywhere.
pub fn clearing_wage(w_guess: f64, l_supply: f64, demand: impl Fn(f64) -> f64) -> f64 {
    let f = |x: f64| demand(libm::exp(x)) - l_supply;
    let mut x = libm::log(w_guess);
    let fx = f(x);
    if fx == 0.0 {
        return w_guess;
    }
    let ln2 = core::f64::consts::LN_2;
    // Invariant: f(lo) > 0 >= f(hi).
    let (mut lo, mut hi);
    if fx > 0.0 {
        lo = x;
        hi = x;
        let mut found = false;
        for _ in 0..MAX_BRACKET {
            lo = hi;
            hi += ln2;
            if f(hi) <= 0.0 {
                found = true;
                break;
            }
        }
        if !found {
            return libm::exp(hi);
        }
    } else {
        hi = x;
        lo = x;
        let mut found = false;
        for _ in 0..MAX_BRACKET {
            hi = lo;
            lo -= ln2;
            if f(lo) > 0.0 {
                found = true;
                break;
            }
        }
        if !found {
            return libm::exp(lo);
        }
    }
    x = 0.5 * (lo + hi);
    let mut fx = f(x);
    for _ in 0..MAX_ITER {
        if fx == 0.0 {
            break;
        }
        if fx > 0.0 {
            lo = x;
        } else {
            hi = x;
        }
        let h = 1e-7;
        let slope = (f(x + h) - f(x - h)) / (2.0 * h);
        let mut next = x - fx / slope;
        if !next.is_finite() || next <= lo || next >= hi {
            next = 0.5 * (lo + hi);
        }
        let done = libm::fabs(libm::expm1(next - x)) < TOL || (hi - lo) < TOL;
        x = next;
        fx = f(x);
        if done {
            break;
        }
    }
    libm::exp(x)
}

/// L_t = prev + eta (target - prev) per enterprise; if the total exceeds `l_supply`, all are
/// scaled down pro rata. The total is summed in fixed order.
pub fn allocate(targets: &[f64], prev: &[f64], eta: f64, l_supply: f64, out: &mut [f64]) {
    for ((o, &t), &p) in out.iter_mut().zip(targets).zip(prev) {
        *o = p + eta * (t - p);
    }
    let total = fixed_sum(out);
    if total > l_supply {
        let scale = l_supply / total;
        for o in out.iter_mut() {
            *o *= scale;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production::{labour_target, rho};

    #[test]
    fn wage_step_rigidity_halves_cuts() {
        let p = EconParams::default();
        let up = wage_step(2.0, 0.5, 0.0, &p, 0.1);
        assert!((up - 2.0 * libm::exp(0.05)).abs() < 1e-15);
        let down = wage_step(2.0, -0.5, 0.0, &p, 0.1);
        assert!((down - 2.0 * libm::exp(-0.025)).abs() < 1e-15);
        // Clip applies before rigidity.
        let big_down = wage_step(2.0, -10.0, 0.0, &p, 0.1);
        assert!((big_down - 2.0 * libm::exp(-0.05)).abs() < 1e-15);
        let big_up = wage_step(2.0, 10.0, 0.0, &p, 0.1);
        assert!((big_up - 2.0 * libm::exp(0.1)).abs() < 1e-15);
        // Frictionless: no rigidity.
        let f = EconParams::frictionless();
        let down = wage_step(2.0, -0.5, 0.0, &f, 0.1);
        assert!((down - 2.0 * libm::exp(-0.05)).abs() < 1e-15);
        assert_eq!(wage_step(2.0, 0.0, 0.0, &p, 0.1), 2.0);
    }

    #[test]
    fn wage_rigidity_applies_to_nominal_cuts() {
        let p = EconParams::default();
        // a real cut that is a nominal rise: no rigidity, 0.02 - 0.1 * 0.1
        let w = wage_step(2.0, -0.1, 0.02, &p, 0.1);
        assert!((w - 2.0 * libm::exp(0.01)).abs() < 1e-15);
        // a nominal cut: halved, (0.02 - 0.1 * 0.5) / 2
        let w = wage_step(2.0, -0.5, 0.02, &p, 0.1);
        assert!((w - 2.0 * libm::exp(-0.015)).abs() < 1e-15);
        // expected deflation turns full employment into a nominal cut
        let w = wage_step(2.0, 0.0, -0.01, &p, 0.1);
        assert!((w - 2.0 * libm::exp(-0.005)).abs() < 1e-15);
        // with no excess the wage moves by pi^e; the clip applies to the sum
        let w = wage_step(2.0, 0.0, 0.03, &p, 0.1);
        assert!((w - 2.0 * libm::exp(0.03)).abs() < 1e-15);
        assert!((wage_step(2.0, 0.5, 0.08, &p, 0.1) - 2.0 * libm::exp(0.1)).abs() < 1e-15);
    }

    const ENTERPRISES: [(f64, f64, f64, f64, f64); 4] = [
        // p_va, a, va0, l0, s_l
        (1.7, 1.3, 100.0, 50.0, 0.6),
        (0.9, 1.0, 80.0, 40.0, 0.5),
        (2.4, 0.8, 30.0, 12.0, 0.7),
        (-0.3, 1.0, 60.0, 30.0, 0.5),
    ];

    fn demand(sigma: f64, l_supply: f64) -> impl Fn(f64) -> f64 {
        move |w| {
            ENTERPRISES
                .iter()
                .map(|&(p, a, va0, l0, s)| labour_target(p, w, a, va0, l0, s, rho(sigma), l_supply))
                .sum()
        }
    }

    #[test]
    fn clearing_wage_clears() {
        for sigma in [0.5, 0.8, 1.0, 1.2] {
            let l_supply = 90.0;
            let d = demand(sigma, l_supply);
            for guess in [1e-3, 1.0, 2.0, 1e3] {
                let w = clearing_wage(guess, l_supply, &d);
                assert!(w > 0.0 && w.is_finite());
                assert!(
                    (d(w) - l_supply).abs() < 1e-12 * l_supply.max(1.0),
                    "sigma {sigma} guess {guess}: demand {} supply {l_supply}",
                    d(w)
                );
            }
        }
    }

    #[test]
    fn clearing_wage_without_root_returns_lower_limit() {
        let w = clearing_wage(1.0, 50.0, |_| 0.0);
        assert!(w > 0.0 && w < 1e-30, "got {w}");
    }

    #[test]
    fn allocate_blends_toward_targets() {
        let mut out = [0.0; 3];
        allocate(&[10.0, 20.0, 0.0], &[0.0, 10.0, 10.0], 0.3, 100.0, &mut out);
        assert!((out[0] - 3.0).abs() < 1e-15);
        assert!((out[1] - 13.0).abs() < 1e-15);
        assert!((out[2] - 7.0).abs() < 1e-15);
    }

    #[test]
    fn allocate_scales_pro_rata_when_short() {
        let mut out = [0.0; 3];
        allocate(&[60.0, 30.0, 10.0], &[0.0; 3], 1.0, 50.0, &mut out);
        assert!((out.iter().sum::<f64>() - 50.0).abs() < 1e-12);
        assert!((out[0] / out[1] - 2.0).abs() < 1e-12);
        assert!((out[1] / out[2] - 3.0).abs() < 1e-12);
    }
}
