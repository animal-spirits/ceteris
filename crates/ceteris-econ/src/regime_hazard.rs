//! Monthly regime transition hazards (P5 Branch B2, ADR 0242 D4, spec/09 9.2). Each state sits in
//! one coarse Ilzetzki-Reinhart-Rogoff class; the hazard of moving to class k is
//! `scale * exp(beta_k . x)` on inflation over 24 months, reserve cover, a recent peg break,
//! log income and trade share with the anchor. A switch is decided by thinning on one shared
//! uniform. Wired into the monetary system (230) when hazards are switched on; coefficients are
//! inputs (fitted on IRR in P9).

use ceteris_core::entity::Kind;

/// RNG stream kind of the monthly regime draw, one entity per country.
pub const KIND: Kind = Kind(0x5201);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegimeClass {
    HardPeg,
    BandOrCrawl,
    ManagedFloat,
    FreeFloat,
}

pub const CLASSES: [RegimeClass; 4] = [
    RegimeClass::HardPeg,
    RegimeClass::BandOrCrawl,
    RegimeClass::ManagedFloat,
    RegimeClass::FreeFloat,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Covariates {
    pub inflation_24m: f64,
    pub reserve_cover: f64,
    /// 1 if the state broke a peg in the past 24 months.
    pub recent_peg_break: f64,
    pub log_income: f64,
    pub anchor_trade_share: f64,
}

impl Covariates {
    fn x(&self) -> [f64; 6] {
        [
            1.0,
            self.inflation_24m,
            self.reserve_cover,
            self.recent_peg_break,
            self.log_income,
            self.anchor_trade_share,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionHazards {
    /// Multiplies every hazard; zero keeps all regimes fixed.
    pub scale: f64,
    /// Logit coefficients `[intercept, inflation, cover, break, log income, anchor share]` of
    /// the hazard of moving to each class, in `CLASSES` order.
    pub beta: [[f64; 6]; 4],
}

impl TransitionHazards {
    /// Monthly hazard of moving from `from` to each class; zero for `from` itself.
    pub fn hazards(&self, from: RegimeClass, cov: &Covariates) -> [f64; 4] {
        let x = cov.x();
        let mut h = [0.0; 4];
        for (k, class) in CLASSES.iter().enumerate() {
            if *class != from {
                let z: f64 = self.beta[k].iter().zip(x).map(|(b, x)| b * x).sum();
                h[k] = self.scale * libm::exp(z);
            }
        }
        h
    }

    /// Regime after one month given `u` in `[0, 1)`: a switch happens when `u < 1 - exp(-sum h)`,
    /// to class k with probability `h_k / sum h` (thinning on one shared uniform).
    pub fn step(&self, from: RegimeClass, cov: &Covariates, u: f64) -> RegimeClass {
        let h = self.hazards(from, cov);
        let total: f64 = h.iter().sum();
        if total <= 0.0 || u >= 1.0 - libm::exp(-total) {
            return from;
        }
        let pick = u / (1.0 - libm::exp(-total)) * total;
        let mut acc = 0.0;
        for (k, hk) in h.iter().enumerate() {
            acc += hk;
            if pick < acc {
                return CLASSES[k];
            }
        }
        from
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cov(inflation: f64, cover: f64, brk: f64) -> Covariates {
        Covariates {
            inflation_24m: inflation,
            reserve_cover: cover,
            recent_peg_break: brk,
            log_income: 8.0,
            anchor_trade_share: 0.3,
        }
    }

    /// Hazard of leaving a hard peg for a float rises with inflation and falls with cover.
    fn model(scale: f64) -> TransitionHazards {
        let mut beta = [[-8.0, 0.0, 0.0, 0.0, 0.0, 0.0]; 4];
        beta[3] = [-6.0, 4.0, -1.5, 1.0, 0.0, 0.0];
        TransitionHazards { scale, beta }
    }

    #[test]
    fn transition_hazards_at_zero_keep_regimes_fixed() {
        let m = model(0.0);
        for &from in &CLASSES {
            for i in 0..100 {
                let u = i as f64 / 100.0;
                assert_eq!(m.step(from, &cov(0.9, 0.0, 1.0), u), from);
            }
        }
    }

    #[test]
    fn hazard_to_free_float_rises_with_inflation_and_falls_with_cover() {
        let m = model(1.0);
        let h = |c: Covariates| m.hazards(RegimeClass::HardPeg, &c)[3];
        assert!(h(cov(0.3, 1.0, 0.0)) > h(cov(0.02, 1.0, 0.0)));
        assert!(h(cov(0.1, 0.2, 0.0)) > h(cov(0.1, 2.0, 0.0)));
        assert!(h(cov(0.1, 1.0, 1.0)) > h(cov(0.1, 1.0, 0.0)));
    }

    #[test]
    fn own_class_has_zero_hazard() {
        let h = model(1.0).hazards(RegimeClass::FreeFloat, &cov(0.1, 1.0, 0.0));
        assert_eq!(h[3], 0.0);
    }

    #[test]
    fn switch_frequency_matches_the_hazard() {
        let m = model(1.0);
        let c = cov(0.5, 0.1, 1.0);
        let total: f64 = m.hazards(RegimeClass::HardPeg, &c).iter().sum();
        let n = 100_000;
        let switched = (0..n)
            .filter(|&i| {
                m.step(RegimeClass::HardPeg, &c, (i as f64 + 0.5) / n as f64)
                    != RegimeClass::HardPeg
            })
            .count() as f64
            / n as f64;
        assert!((switched - (1.0 - libm::exp(-total))).abs() < 1e-4);
    }
}
