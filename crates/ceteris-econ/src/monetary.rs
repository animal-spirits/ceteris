//! Monetary regimes and policy rules of one central bank (P5 Branch B1, spec/09 9.2, ADR 0176,
//! `specs/money/SPEC.md`). Floating rule `i = max(lb, r* + pi + a_pi (pi - pi*) + a_y y)`, asset
//! purchases at the bound, hard-peg defence from reserves, union and dollarized members taking the
//! anchor's rate. Wired into the tick as system 230; the optional HLW neutral-rate filter (B3)
//! updates each floating bank's r* once a year.

use crate::hlw::{HlwParams, NeutralRateFilter, Observation};
use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

pub const SNAPSHOT_TAG: u32 = 0x3A6E_6D01;

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

/// Per-country policy state of system 230: one bank per country and the rate it set last month.
/// Inflation is the annualised log change of the country's consumption-weighted buyer price
/// level; the output gap is the log of employment against its level at the first step (a
/// labour-input proxy; the potential-output filter is not built). FX pressure on pegs is zero
/// until the exchange-rate system exists, so no peg is defended yet.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    pub banks: Vec<CentralBank>,
    /// Policy rate per country, annual, as of the last step.
    pub rates: Vec<f64>,
    /// Asset-purchase intensity per country at the last step.
    pub purchases: Vec<f64>,
    /// Price level and employment at the previous step; empty before the first.
    pub level: Vec<f64>,
    pub base_employment: Vec<f64>,
    /// Steps taken after the baseline step.
    pub months: u32,
    /// One HLW filter per country (B3), empty when off. Every 12th step it re-estimates r* from
    /// the year's log employment (the output proxy above), inflation and real policy rate, and
    /// writes it into a floating bank's rule.
    pub hlw: Vec<NeutralRateFilter>,
    /// Price level a year ago, for annual inflation; empty before the first step.
    pub year_level: Vec<f64>,
}

impl Policy {
    pub fn floating(n_countries: usize, rule: Rule) -> Self {
        let bank = CentralBank {
            regime: Regime::Float(rule),
            reserves: 0.0,
            short_term_external_debt: 1.0,
        };
        Self {
            banks: vec![bank; n_countries],
            rates: vec![rule.rate(rule.pi_star, 0.0); n_countries],
            purchases: vec![0.0; n_countries],
            level: Vec::new(),
            base_employment: Vec::new(),
            months: 0,
            hlw: Vec::new(),
            year_level: Vec::new(),
        }
    }

    /// Switches the annual HLW neutral-rate update on for floating banks; others keep a frozen
    /// filter. `g0` is the starting trend growth.
    pub fn with_hlw(mut self, params: HlwParams, g0: f64) -> Self {
        self.hlw = self
            .banks
            .iter()
            .map(|b| match b.regime {
                Regime::Float(rule) => {
                    let flat = Observation {
                        y: 0.0,
                        pi: rule.pi_star,
                        r: rule.r_star,
                    };
                    NeutralRateFilter::new(params, rule.r_star, g0, flat, flat)
                }
                _ => NeutralRateFilter::frozen(params, 0.0),
            })
            .collect();
        self
    }

    /// One month: sets every bank's rate from `price_level` and `employment` per country.
    pub fn step(&mut self, price_level: &[f64], employment: &[f64]) {
        let n = self.banks.len();
        if self.level.is_empty() {
            self.level = price_level.to_vec();
            self.base_employment = employment.to_vec();
            self.year_level = price_level.to_vec();
            return;
        }
        let pi: Vec<f64> = (0..n)
            .map(|c| 12.0 * libm::log(price_level[c] / self.level[c]))
            .collect();
        let y_gap: Vec<f64> = (0..n)
            .map(|c| libm::log(employment[c] / self.base_employment[c]))
            .collect();
        self.rates = policy_rates(&self.banks, &pi, &y_gap, &vec![0.0; n]);
        self.purchases = (0..n)
            .map(|c| match self.banks[c].regime {
                Regime::Float(rule) => rule.purchases(pi[c], y_gap[c]),
                _ => 0.0,
            })
            .collect();
        self.level = price_level.to_vec();
        self.months += 1;
        if !self.hlw.is_empty() && self.months.is_multiple_of(12) {
            for c in 0..n {
                let pi = libm::log(price_level[c] / self.year_level[c]);
                let obs = Observation {
                    y: libm::log(employment[c] / self.base_employment[c]),
                    pi,
                    r: self.rates[c] - pi,
                };
                let r_star = self.hlw[c].update(obs);
                if let Regime::Float(rule) = &mut self.banks[c].regime {
                    rule.r_star = r_star;
                }
            }
            self.year_level = price_level.to_vec();
        }
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.u32(SNAPSHOT_TAG);
        let rule = |r: &Rule| [r.r_star, r.a_pi, r.a_y, r.pi_star, r.lower_bound];
        let mut bank = Vec::new();
        for b in &self.banks {
            let (kind, anchor, premium, active, fallback) = match b.regime {
                Regime::Float(r) => (0.0, 0, 0.0, rule(&r), [0.0; 5]),
                Regime::HardPeg {
                    anchor,
                    fallback,
                    defence_premium,
                } => (1.0, anchor, defence_premium, [0.0; 5], rule(&fallback)),
                Regime::Follower { anchor } => (2.0, anchor, 0.0, [0.0; 5], [0.0; 5]),
            };
            bank.extend([kind, anchor as f64, premium]);
            bank.extend(active);
            bank.extend(fallback);
            bank.extend([b.reserves, b.short_term_external_debt]);
        }
        e.f64s(&bank);
        e.f64s(&self.rates);
        e.f64s(&self.purchases);
        e.f64s(&self.level);
        e.f64s(&self.base_employment);
        e.u32(self.months);
        e.f64s(&self.year_level);
        e.f64s(&self.hlw.iter().flat_map(|f| f.encode()).collect::<Vec<_>>());
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        if d.u32()? != SNAPSHOT_TAG {
            return Err(DecodeError("monetary section tag".into()));
        }
        let bank = d.f64s()?;
        let rule = |s: &[f64]| Rule {
            r_star: s[0],
            a_pi: s[1],
            a_y: s[2],
            pi_star: s[3],
            lower_bound: s[4],
        };
        let banks = bank
            .as_chunks::<15>()
            .0
            .iter()
            .map(|b| CentralBank {
                regime: match b[0] as u8 {
                    0 => Regime::Float(rule(&b[3..8])),
                    1 => Regime::HardPeg {
                        anchor: b[1] as usize,
                        fallback: rule(&b[8..13]),
                        defence_premium: b[2],
                    },
                    _ => Regime::Follower {
                        anchor: b[1] as usize,
                    },
                },
                reserves: b[13],
                short_term_external_debt: b[14],
            })
            .collect();
        let rates = d.f64s()?;
        let purchases = d.f64s()?;
        let level = d.f64s()?;
        let base_employment = d.f64s()?;
        let months = d.u32()?;
        let year_level = d.f64s()?;
        let hlw = d
            .f64s()?
            .as_chunks::<{ NeutralRateFilter::ENCODED_LEN }>()
            .0
            .iter()
            .map(|c| NeutralRateFilter::decode(c))
            .collect();
        Ok(Self {
            banks,
            rates,
            purchases,
            level,
            base_employment,
            months,
            hlw,
            year_level,
        })
    }
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
