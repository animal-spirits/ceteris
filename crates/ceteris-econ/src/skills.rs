//! Two skill groups under a CES (P4 Branch C1, ADR 0105, `specs/pop/SPEC.md`): high = tertiary,
//! low = the rest. `L = (alpha L_h^((s-1)/s) + (1 - alpha) L_l^((s-1)/s))^(s/(s-1))`.
//! The kernel is not yet in the tick: the wage state per (province, sector, skill) is not
//! widened and `s`, `alpha` have no register codes.

#[derive(Clone, Debug, PartialEq)]
pub struct SkillCes {
    /// Off: the groups are perfect substitutes, `L = L_h + L_l`, and the premium is 1.
    pub enabled: bool,
    /// Elasticity of substitution between the groups.
    pub s: f64,
    /// Weight of the high group.
    pub alpha: f64,
}

impl SkillCes {
    pub fn new(s: f64, alpha: f64) -> Self {
        Self {
            enabled: false,
            s,
            alpha,
        }
    }

    /// Weight `alpha` that makes the premium `w_h / w_l` equal `premium` at supplies `l_h`, `l_l`.
    pub fn alpha_for_premium(s: f64, premium: f64, l_h: f64, l_l: f64) -> f64 {
        let r = premium * libm::pow(l_h / l_l, 1.0 / s);
        r / (1.0 + r)
    }

    /// Effective labour.
    pub fn effective(&self, l_h: f64, l_l: f64) -> f64 {
        if !self.enabled {
            return l_h + l_l;
        }
        let rho = (self.s - 1.0) / self.s;
        libm::pow(
            self.alpha * libm::pow(l_h, rho) + (1.0 - self.alpha) * libm::pow(l_l, rho),
            1.0 / rho,
        )
    }

    /// Marginal products `(dL/dL_h, dL/dL_l)`: the wages per unit of effective labour.
    pub fn marginal(&self, l_h: f64, l_l: f64) -> (f64, f64) {
        if !self.enabled {
            return (1.0, 1.0);
        }
        let l = self.effective(l_h, l_l);
        (
            self.alpha * libm::pow(l / l_h, 1.0 / self.s),
            (1.0 - self.alpha) * libm::pow(l / l_l, 1.0 / self.s),
        )
    }

    /// Skill premium `w_h / w_l`.
    pub fn premium(&self, l_h: f64, l_l: f64) -> f64 {
        let (h, l) = self.marginal(l_h, l_l);
        h / l
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(alpha: f64) -> SkillCes {
        let mut c = SkillCes::new(1.6, alpha);
        c.enabled = true;
        c
    }

    #[test]
    fn skills_off_equals_status_quo() {
        let c = SkillCes::new(1.6, 0.7);
        assert_eq!(c.effective(30.0, 70.0), 100.0);
        assert_eq!(c.marginal(30.0, 70.0), (1.0, 1.0));
        assert_eq!(c.premium(30.0, 70.0), 1.0);
    }

    #[test]
    fn skill_premium_matches_the_target_after_calibration() {
        for premium in [1.2, 1.8, 2.5] {
            let alpha = SkillCes::alpha_for_premium(1.6, premium, 25.0, 75.0);
            let got = on(alpha).premium(25.0, 75.0);
            assert!((got - premium).abs() < 1e-12, "{premium}: {got}");
        }
    }

    #[test]
    fn premium_falls_with_skilled_supply_at_elasticity_minus_one_over_s() {
        let c = on(0.4);
        let (a, b) = (c.premium(20.0, 80.0), c.premium(40.0, 80.0));
        assert!(b < a);
        let slope = libm::log(b / a) / libm::log(2.0);
        assert!((slope + 1.0 / 1.6).abs() < 1e-12, "{slope}");
    }

    #[test]
    fn constant_returns_exhaust_effective_labour() {
        let c = on(0.35);
        let (h, l) = c.marginal(18.0, 62.0);
        let total = h * 18.0 + l * 62.0;
        assert!((total - c.effective(18.0, 62.0)).abs() < 1e-9);
    }

    #[test]
    fn high_elasticity_approaches_perfect_substitutes() {
        let mut c = SkillCes::new(1e6, 0.5);
        c.enabled = true;
        let l = c.effective(30.0, 70.0);
        // s -> infinity gives the weighted sum alpha L_h + (1 - alpha) L_l.
        assert!((l - 50.0).abs() < 1e-3, "{l}");
    }
}
