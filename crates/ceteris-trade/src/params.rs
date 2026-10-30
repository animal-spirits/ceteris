//! Trade parameters with their register IDs (`specs/trade/SPEC.md`, Parameters).

use crate::goods::{N_GOODS, STORABLE};

/// Theta classes of P01 (ADR 0023, Amendment 2026-10-29).
pub const N_CLASSES: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeParams {
    /// P01 trade elasticity theta_g per good, local goods 0 (unused). Prior 2.91 to 16.91, the
    /// range of the class values (ADR 0101). Each traded good carries its class's theta, derived
    /// by the ADR 0023 rule from the OECD concordance (`ceteris_pack::econ::theta`): Caliendo and
    /// Parro (2015, Table 1, 99% sample) sector estimates, with Auto and Other Transport at 8.22
    /// as in the authors' replication vector, harmonic means over the HS lines of the good with
    /// BACI 2000 trade weights: agriculture 9.11, crude oil, gas and refined fuels 17.64, coal and
    /// other mining 13.53, food 2.62, light manufacturing 7.46, chemicals and metals 3.00,
    /// machinery, electronics and vehicles 4.14; grouped into 4 classes within a factor 1.5, each
    /// at the harmonic mean over its lines, to two decimals. The test
    /// `fixture_theta_defaults_follow_the_estimator` (ceteris-pack) keeps these and `theta_class`
    /// equal to the estimator's output.
    pub theta: [f64; N_GOODS],
    /// P01 theta class of each good (ADR 0023), 0 for the lowest theta; local goods 0
    /// (unused). Kernels stay per good, since weight classes are goods in P2 (ADR 0018); the
    /// map lets goods of one class share kernels once weight classes merge.
    pub theta_class: [u8; N_GOODS],
    /// P150 theta scale: the kernel is built at theta_g times this (ADR 0101). Tariff
    /// experiments report the lane {0.6, 1, 1.4}.
    pub theta_scale: f64,
    /// P02 inventory target coverage s_g in months; local goods 0.
    pub coverage: [f64; N_GOODS],
    /// P03a price response to the demand-supply gap, per month. Prior 0.01 to 0.5 (widened
    /// from 0.05 by owner decision, 2026-10-24, for stability), default 0.05 (owner decision,
    /// 2026-10-24, MIGRATION Task 6: at 0.02 the expectation loop of ADR 0019 breaks V1 for
    /// every gain in the P45 prior). The derived stability region (`specs/trade/SPEC.md`,
    /// Stability region) bounds it at 0.101 to 0.110 for the theta-16.91 goods, 0.191 to 0.222
    /// at theta 7.68, 0.287 to 0.366 at theta 4.14 and 0.307 to 0.473 at theta 2.91. Growth
    /// factor per tick of a small deviation from the fixture's GE equilibrium between ticks 300
    /// and 600 at the per-good thetas before ADR 0101 (10.57, 5.19, 2.62), kappa2 = 0
    /// (frictionless / default): at the defaults (pi^e gain 0.006) 0.99597 / 0.99636 on the
    /// blessed fixture (after the theta task; 0.99644 / 0.99676 at MIGRATION Task 8 and
    /// 0.9963 / 0.9969 at Task 6, at the previous thetas);
    /// measured at MIGRATION Task 6, with pi^e = 0, kappa1 0.02: 0.9954 / 0.9954; 0.05:
    /// 0.9862 / 0.9871; 0.1: 0.9718 / 0.9756; 0.2: 1.0000 / 1.0000 (oscillates), the same with
    /// every crossing lag at 3 months (the full pack's cap).
    pub kappa1: f64,
    /// P03b price response to the inventory gap, per month. Prior 0 to 0.5 (widened from 0.05
    /// by owner decision, 2026-10-24), default 0: the inventory term is off and inventories
    /// are untargeted in P2, so the default sits on the region's edge kappa2 = 0, where only
    /// the stock is neutral (counted as inside, owner ruling, MIGRATION Task 6). At kappa1 0.05
    /// the region ends at kappa2 0.049 to 0.392 in the default mode and 0.059 to 0.347
    /// frictionless. With pi^e = 0, kappa2 = 0.05 overflows the ledger in the default mode at
    /// kappa1 0.02 and grows at 0.05 (1.0014), and decays frictionless at kappa1 0.02 and 0.05
    /// (0.9876, 0.9738).
    pub kappa2: f64,
    /// P03c price step clip in log points per month, also the wage step clip; it bounds the gap
    /// terms only, expected inflation passes through (ADR 0106). Every tick it binds is counted
    /// per market (`Markets::clip_ticks`) and per wage.
    pub c_max: f64,
    /// P151 price step clip of the storable commodities (`goods::STORABLE`), in log points per
    /// month (ADR 0106): monthly commodity moves exceed P03c (oil fell about 30% in one month in
    /// late 2008).
    pub c_max_storable: f64,
    /// P45 expectation gain g: pi^e <- pi^e + g (ln(I_t / I_{t-1}) - pi^e) per month, I the
    /// world price index (ADR 0019). Prior 0.004 to 0.65: Poledna et al. (2023) learn an AR(1)
    /// on inflation by OLS over an expanding sample (gain 1/n, about 0.004 to 0.006 per month
    /// over their reference quarters); the adaptive heuristic of Anufriev and Hommes (2012,
    /// Table 1) weights the last observation 0.65 per period. Start 0.006, the Poledna end
    /// (owner decision, 2026-10-24, MIGRATION Task 6).
    pub expectation_gain: f64,
    /// P05 non-tariff border friction nu per theta class, ad valorem equivalent (ADR 0092): the
    /// default is the prior's start; a calibrated pack carries the values that match the ICIO
    /// 2000 cross-border share of each class (`ceteris_calibrate::border`), which
    /// `EconInputs::trade_params` puts here.
    pub border_nu: [f64; N_CLASSES],
    /// P43 ad valorem cost of a day in transit, lambda_g.
    pub time_cost: [f64; N_GOODS],
    /// P44 route-choice dispersion theta_r (ADR 0017): edge factors `a = t^-theta_r`, route
    /// costs `tau = [(I - A)^-1]^(-1/theta_r)`. Prior 41 to 182 (Fan, Lu and Luo 2021: 111.5,
    /// s.e. 35.4), value its point estimate. Unused in P2: route choice is deferred (ADR 0017),
    /// since no value in the prior brings the spectral radius below 1 on the P1 graph, and
    /// `Network::route_taus` stays `None`.
    pub theta_r: f64,
    /// P154 bulk freight rate (ADR 0247): multiplier on the sea cost per tonne-km (P04c, sea and
    /// canal edges) for the legs of each theta class. 1 for classes 0 to 2; class 3 (fuels and
    /// mining) 0.026, the slope of 2000 tanker and dry-bulk voyage rates per tonne against sea
    /// km (UNCTAD Review of Maritime Transport 2001 and 2002; trade SPEC, Parameters) over
    /// P04c. On by default by owner ruling (2026-10-30). It reaches the kernel only through
    /// `Network::bulk`.
    pub bulk_sea: [f64; N_CLASSES],
    /// P155 bulk freight rate (ADR 0247): multiplier on the listed and minor port fees (P04m)
    /// per theta class. 1 for classes 0 to 2; class 3 0.40, half the fixed part of the same
    /// rates over the listed-port fee (on by default, as P154).
    pub bulk_port: [f64; N_CLASSES],
}

impl TradeParams {
    /// The border friction of good `g`'s theta class.
    pub fn border_nu_of(&self, g: usize) -> f64 {
        self.border_nu[usize::from(self.theta_class[g])]
    }

    /// The bulk freight rate (ADR 0247) as the theta classes that use it and their (sea, port)
    /// multipliers, or `None` when every class is at 1. Panics when two classes use different
    /// rates: the network carries one set of bulk legs.
    pub fn bulk_rate(&self) -> Option<([bool; N_CLASSES], f64, f64)> {
        let classes: [bool; N_CLASSES] =
            std::array::from_fn(|c| self.bulk_sea[c] != 1.0 || self.bulk_port[c] != 1.0);
        let c = classes.iter().position(|&b| b)?;
        let rate = (self.bulk_sea[c], self.bulk_port[c]);
        for k in (0..N_CLASSES).filter(|&k| classes[k]) {
            let r = (self.bulk_sea[k], self.bulk_port[k]);
            assert!(
                r.0 > 0.0 && r.1 >= 0.0 && r.0.is_finite() && r.1.is_finite(),
                "bulk freight rate {r:?} of class {k}"
            );
            assert!(
                r == rate,
                "classes {c} and {k} have different bulk freight rates"
            );
        }
        Some((classes, rate.0, rate.1))
    }

    /// These parameters with the bulk freight rate off (P154 and P155 at 1 for every class), for
    /// a network without bulk legs: one built without a graph, or one with route taus.
    #[must_use]
    pub fn bulk_off(self) -> Self {
        Self {
            bulk_sea: [1.0; N_CLASSES],
            bulk_port: [1.0; N_CLASSES],
            ..self
        }
    }

    /// The price step clip of good `g`: P151 for storable commodities, else P03c.
    pub fn price_clip(&self, g: usize) -> f64 {
        if STORABLE[g] {
            self.c_max_storable
        } else {
            self.c_max
        }
    }
}

impl Default for TradeParams {
    fn default() -> Self {
        Self {
            theta: [7.68, 16.91, 16.91, 2.91, 7.68, 2.91, 4.14, 0.0, 0.0, 0.0],
            theta_class: [2, 3, 3, 0, 2, 0, 1, 0, 0, 0],
            theta_scale: 1.0,
            coverage: [3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0],
            kappa1: 0.05,
            kappa2: 0.0,
            c_max: 0.1,
            c_max_storable: 0.3,
            expectation_gain: 0.006,
            border_nu: [0.3; N_CLASSES],
            time_cost: [
                0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010, 0.0, 0.0, 0.0,
            ],
            theta_r: 111.5,
            bulk_sea: [1.0, 1.0, 1.0, 0.026],
            bulk_port: [1.0, 1.0, 1.0, 0.40],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P154 and P155 default to the sourced bulk freight rate for class 3 only (ADR 0247,
    /// owner ruling 2026-10-30).
    #[test]
    fn bulk_rate_is_on_for_class_3_by_default() {
        assert_eq!(
            TradeParams::default().bulk_rate(),
            Some(([false, false, false, true], 0.026, 0.40))
        );
    }

    /// A NaN multiplier is reported as a bad rate, not as two classes that differ.
    #[test]
    #[should_panic(expected = "bulk freight rate (NaN, 1.0) of class 3")]
    fn bulk_rate_rejects_nan() {
        TradeParams {
            bulk_sea: [1.0, 1.0, 1.0, f64::NAN],
            ..TradeParams::default().bulk_off()
        }
        .bulk_rate();
    }
}
