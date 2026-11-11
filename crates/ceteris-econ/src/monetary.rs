//! Monetary regimes and policy rules of one central bank (P5 Branch B1, spec/09 9.2, ADR 0176,
//! `specs/money/SPEC.md`). Floating rule `i = max(lb, r* + pi + a_pi (pi - pi*) + a_y y)`, asset
//! purchases at the bound, hard-peg defence from reserves, union and dollarized members taking the
//! anchor's rate. The kernel is not yet in the tick.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    /// Neutral real rate; the 2000 value is P35's intercept, later the HLW filter state.
    pub r_star: f64,
    pub a_pi: f64,
    pub a_y: f64,
    pub pi_star: f64,
    /// Effective lower bound on the policy rate.
    pub lower_bound: f64,
}

impl Rule {
    pub fn unconstrained(&self, pi: f64, y_gap: f64) -> f64 {
        self.r_star + pi + self.a_pi * (pi - self.pi_star) + self.a_y * y_gap
    }

    pub fn rate(&self, pi: f64, y_gap: f64) -> f64 {
        self.unconstrained(pi, y_gap).max(self.lower_bound)
    }

    /// Asset-purchase intensity at the bound: how far the rule wants to go below it.
    pub fn purchases(&self, pi: f64, y_gap: f64) -> f64 {
        (self.lower_bound - self.unconstrained(pi, y_gap)).max(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Regime {
    Float(Rule),
    /// Rate defends the anchor's parity from reserves while cover lasts, then the rule floats.
    HardPeg {
        anchor: usize,
        fallback: Rule,
        defence_premium: f64,
    },
    /// Members of a union or dollarized economy take the anchor's rate.
    Follower {
        anchor: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CentralBank {
    pub regime: Regime,
    pub reserves: f64,
    pub short_term_external_debt: f64,
}

impl CentralBank {
    /// Reserve cover `R / short-term external debt`.
    pub fn cover(&self) -> f64 {
        self.reserves / self.short_term_external_debt
    }

    /// Sell reserves against `pressure` of currency outflow. A peg holds while reserves cover it;
    /// otherwise reserves go to zero and the bank floats on its fallback rule. Returns whether
    /// the peg held.
    pub fn defend(&mut self, pressure: f64) -> bool {
        let Regime::HardPeg { fallback, .. } = self.regime else {
            return true;
        };
        if self.reserves >= pressure {
            self.reserves -= pressure;
            true
        } else {
            self.reserves = 0.0;
            self.regime = Regime::Float(fallback);
            false
        }
    }
}

/// Policy rates of all banks for one tick. `pressure[i]` is bank i's currency outflow as a share of
/// its short-term external debt, which raises a defended peg's rate above the anchor's. Anchors
/// are resolved on their own rate, which must be a floating or peg bank, never a follower.
pub fn policy_rates(
    banks: &[CentralBank],
    pi: &[f64],
    y_gap: &[f64],
    pressure: &[f64],
) -> Vec<f64> {
    let own = |i: usize| match banks[i].regime {
        Regime::Float(rule) => Some(rule.rate(pi[i], y_gap[i])),
        _ => None,
    };
    (0..banks.len())
        .map(|i| match banks[i].regime {
            Regime::Float(rule) => rule.rate(pi[i], y_gap[i]),
            Regime::HardPeg {
                anchor,
                defence_premium,
                ..
            } => own(anchor).unwrap_or(0.0) + defence_premium * pressure[i],
            Regime::Follower { anchor } => own(anchor).unwrap_or(0.0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule() -> Rule {
        Rule {
            r_star: 0.02,
            a_pi: 0.5,
            a_y: 0.5,
            pi_star: 0.02,
            lower_bound: 0.0,
        }
    }

    #[test]
    fn rule_at_target_sets_neutral_nominal_rate() {
        assert!((rule().rate(0.02, 0.0) - 0.04).abs() < 1e-15);
    }

    #[test]
    fn rate_rises_more_than_one_for_one_with_inflation() {
        let r = rule();
        assert!(r.rate(0.06, 0.0) - r.rate(0.02, 0.0) > 0.04);
    }

    #[test]
    fn asset_purchases_start_at_the_lower_bound() {
        let r = rule();
        assert_eq!(r.purchases(0.02, 0.0), 0.0);
        assert!(r.purchases(-0.02, -0.06) > 0.0);
        assert_eq!(r.rate(-0.02, -0.06), r.lower_bound);
    }

    #[test]
    fn peg_defends_with_reserves_until_cover_runs_out() {
        let mut peg = CentralBank {
            regime: Regime::HardPeg {
                anchor: 0,
                fallback: rule(),
                defence_premium: 0.5,
            },
            reserves: 100.0,
            short_term_external_debt: 80.0,
        };
        for _ in 0..4 {
            assert!(peg.defend(20.0));
            assert!(matches!(peg.regime, Regime::HardPeg { .. }));
        }
        assert_eq!(peg.cover(), 0.25);
        assert!(!peg.defend(30.0));
        assert_eq!(peg.reserves, 0.0);
        assert_eq!(peg.regime, Regime::Float(rule()));
    }

    #[test]
    fn defended_peg_pays_a_premium_over_the_anchor() {
        let anchor = CentralBank {
            regime: Regime::Float(rule()),
            reserves: 0.0,
            short_term_external_debt: 1.0,
        };
        let peg = CentralBank {
            regime: Regime::HardPeg {
                anchor: 0,
                fallback: rule(),
                defence_premium: 0.5,
            },
            reserves: 10.0,
            short_term_external_debt: 5.0,
        };
        let i = policy_rates(&[anchor, peg], &[0.02, 0.0], &[0.0, 0.0], &[0.0, 0.1]);
        assert!((i[1] - i[0] - 0.05).abs() < 1e-15);
    }

    #[test]
    fn union_members_share_one_rate() {
        let ecb = CentralBank {
            regime: Regime::Float(rule()),
            reserves: 0.0,
            short_term_external_debt: 1.0,
        };
        let member = CentralBank {
            regime: Regime::Follower { anchor: 0 },
            reserves: 0.0,
            short_term_external_debt: 1.0,
        };
        let i = policy_rates(
            &[ecb, member, member],
            &[0.03, -0.05, 0.10],
            &[0.01, -0.04, 0.05],
            &[0.0; 3],
        );
        assert_eq!(i[0], i[1]);
        assert_eq!(i[0], i[2]);
    }
}
