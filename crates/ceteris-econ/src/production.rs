//! Normalized CES value added and the closed-form profit-maximizing labour target.

/// Cobb-Douglas threshold on |rho|.
const RHO_CD: f64 = 1e-9;

/// P08 mapping: rho = (sigma_FL - 1) / sigma_FL.
pub fn rho(sigma: f64) -> f64 {
    (sigma - 1.0) / sigma
}

/// VA = A VA0 [(1 - s_L) + s_L (L/L0)^rho]^(1/rho); Cobb-Douglas A VA0 (L/L0)^s_L as rho -> 0.
pub fn value_added(a: f64, va0: f64, l0: f64, s_l: f64, rho: f64, l: f64) -> f64 {
    let x = l / l0;
    if rho.abs() < RHO_CD {
        return a * va0 * libm::pow(x, s_l);
    }
    a * va0 * libm::pow((1.0 - s_l) + s_l * libm::pow(x, rho), 1.0 / rho)
}

/// Profit-maximizing employment of one enterprise at value-added price `p_va` and wage `w`
/// (closed form of p_VA dVA/dL = w), [`labour_target_uncapped`] capped at `l_supply`: one
/// enterprise cannot hire more than the province has.
#[allow(clippy::too_many_arguments)]
pub fn labour_target(
    p_va: f64,
    w: f64,
    a: f64,
    va0: f64,
    l0: f64,
    s_l: f64,
    rho: f64,
    l_supply: f64,
) -> f64 {
    labour_target_uncapped(p_va, w, a, va0, l0, s_l, rho).min(l_supply)
}

/// The closed form of [`labour_target`] without the cap. Corners: 0 if `p_va <= 0`; infinite
/// if `w <= 0`; when the bracket of the closed form is not positive, 0 for rho < 0 and infinite
/// for rho > 0 (no finite profit maximum).
pub fn labour_target_uncapped(
    p_va: f64,
    w: f64,
    a: f64,
    va0: f64,
    l0: f64,
    s_l: f64,
    rho: f64,
) -> f64 {
    if p_va <= 0.0 {
        return 0.0;
    }
    if w <= 0.0 {
        return f64::INFINITY;
    }
    let q = w * l0 / (p_va * a * va0 * s_l);
    if rho.abs() < RHO_CD {
        return l0 * libm::pow(q, 1.0 / (s_l - 1.0));
    }
    let bracket = (libm::pow(q, rho / (1.0 - rho)) - s_l) / (1.0 - s_l);
    if bracket <= 0.0 {
        return if rho < 0.0 { 0.0 } else { f64::INFINITY };
    }
    l0 * libm::pow(bracket, -1.0 / rho)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: f64 = 1.3;
    const VA0: f64 = 100.0;
    const L0: f64 = 50.0;
    const S_L: f64 = 0.6;

    fn profit(p_va: f64, w: f64, rho: f64, l: f64) -> f64 {
        p_va * value_added(A, VA0, L0, S_L, rho, l) - w * l
    }

    fn golden_max(f: impl Fn(f64) -> f64, mut lo: f64, mut hi: f64) -> f64 {
        let g = (5f64.sqrt() - 1.0) / 2.0;
        let (mut x1, mut x2) = (hi - g * (hi - lo), lo + g * (hi - lo));
        let (mut f1, mut f2) = (f(x1), f(x2));
        for _ in 0..200 {
            if f1 < f2 {
                lo = x1;
                x1 = x2;
                f1 = f2;
                x2 = lo + g * (hi - lo);
                f2 = f(x2);
            } else {
                hi = x2;
                x2 = x1;
                f2 = f1;
                x1 = hi - g * (hi - lo);
                f1 = f(x1);
            }
        }
        0.5 * (lo + hi)
    }

    #[test]
    fn value_added_at_base_is_a_va0() {
        for sigma in [0.5, 0.8, 1.0, 1.2] {
            let r = rho(sigma);
            let va = value_added(A, VA0, L0, S_L, r, L0);
            assert!((va - A * VA0).abs() < 1e-10, "sigma {sigma}: {va}");
        }
    }

    #[test]
    fn rho_matches_spec() {
        assert!((rho(0.8) - (-0.25)).abs() < 1e-15);
        assert_eq!(rho(1.0), 0.0);
    }

    #[test]
    fn labour_target_maximizes_profit() {
        let (p_va, w) = (1.7, 2.1);
        for sigma in [0.5, 0.8, 1.0, 1.2] {
            let r = rho(sigma);
            let l = labour_target(p_va, w, A, VA0, L0, S_L, r, 1e6);
            assert!(
                l > 0.0 && l < 1e6,
                "sigma {sigma}: interior target, got {l}"
            );
            let at = profit(p_va, w, r, l);
            for f in [1.0 - 1e-4, 1.0 + 1e-4] {
                assert!(at > profit(p_va, w, r, l * f), "sigma {sigma} factor {f}");
            }
            let numeric = golden_max(|x| profit(p_va, w, r, x), 1e-3, 1e5);
            assert!(
                ((numeric - l) / l).abs() < 1e-5,
                "sigma {sigma}: closed form {l}, golden section {numeric}"
            );
        }
    }

    #[test]
    fn labour_target_corners() {
        let args = |p_va, w, rho, supply| labour_target(p_va, w, A, VA0, L0, S_L, rho, supply);
        // Nonpositive value-added price.
        assert_eq!(args(0.0, 1.0, rho(0.8), 1e6), 0.0);
        assert_eq!(args(-1.0, 1.0, rho(1.2), 1e6), 0.0);
        // rho < 0, wage above the choke price: zero.
        assert_eq!(args(1.0, 1e6, rho(0.8), 1e6), 0.0);
        // rho > 0, wage near zero: the whole labour supply.
        assert_eq!(args(1.0, 1e-12, rho(1.2), 777.0), 777.0);
        // Interior target is capped at the supply.
        let free = args(1.7, 2.1, rho(0.8), 1e6);
        assert_eq!(args(1.7, 2.1, rho(0.8), free / 2.0), free / 2.0);
        // Cobb-Douglas closed form.
        let q = 2.1 * L0 / (1.7 * A * VA0 * S_L);
        let cd = L0 * libm::pow(q, 1.0 / (S_L - 1.0));
        assert!((args(1.7, 2.1, 0.0, 1e6) - cd).abs() < 1e-9);
        assert_eq!(args(0.0, 1.0, 0.0, 1e6), 0.0);
        // uncapped: the corners above the supply are infinite
        let free = |p_va, w, rho| labour_target_uncapped(p_va, w, A, VA0, L0, S_L, rho);
        assert_eq!(free(1.0, 1e-12, rho(1.2)), f64::INFINITY);
        assert_eq!(free(1.0, 0.0, rho(0.8)), f64::INFINITY);
        assert_eq!(free(-1.0, 0.0, rho(0.8)), 0.0);
        assert_eq!(free(1.7, 2.1, rho(0.8)), args(1.7, 2.1, rho(0.8), 1e6));
    }
}
