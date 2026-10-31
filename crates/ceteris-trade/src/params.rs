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
    /// P03d share m of each market's stability limit (J2, `specs/trade/SPEC.md`, Stability
    /// region) that its price gain may use (ADR 0263): kappa1_cell = kappa1 min(1, 2 m /
    /// (kappa1 J_cell)), [`TradeParams::price_gain`]. Prior 0 to 1, start 0.5: J_ref = 20 at
    /// kappa1 0.05: frictionless the 298 traded cells of main's 0256 pack above 20 are capped;
    /// in the default mode none there (J at most 16.9), but 383 on the pre-0256 B scratch pack
    /// (J up to 32.4), pending B8 and an owner ruling (ADR 0263, As built).
    pub gain_share: f64,
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
    /// P56 physical storage loss delta_g per month at the origin markets of the storable goods
    /// (ADR 0091; `goods::STORABLE`, 0 elsewhere). Competitive storage is off by default (owner
    /// ruling 2026-10-31: the approved values destabilize the fixture, ADR 0091 As built), so
    /// the default is 0; [`TradeParams::with_storage`] sets the approved values: agriculture 0.003, a world blend of commercial
    /// grain shrink (Hofstrand, ISU A2-33, 1995) and smallholder post-harvest losses (APHLIS);
    /// fuels 0.0005 and coal and mining 0.001 by judgement; chemicals and metals 0 (base metals do
    /// not deteriorate, Cafiero et al. 2011). Owner approval 2026-10-30; refit in P9.
    pub storage_loss: [f64; N_GOODS],
    /// P56 money cost k_g of holding a unit for a month, interest 0.0047 (FRED TB3MS, 2000 mean
    /// 5.82% a year) plus physical storage: 0.009, 0.011, 0.007, 0.0067 (DESIGN 17, P56; weak).
    pub storage_cost: [f64; N_GOODS],
    /// P144 speculation intensity psi_g in months (ADR 0091), 0 by default (storage off); the
    /// approved values ([`TradeParams::with_storage`]) are 20, 1.0, 3.571, 5.714, derived as
    /// lambda s_g / (zeta (k + delta)) from the stock-persistence target lambda = 1 - rho of
    /// Gorton, Hayashi and Rouwenhorst (2013, Table 2); not estimated (DESIGN 17).
    pub speculation: [f64; N_GOODS],
    /// P145 convenience-yield elasticity zeta_g (ADR 0091): 1.8, 13, 3.5, 3.5, the slope of the
    /// basis in inventory of Gorton, Hayashi and Rouwenhorst (2013, Table 3) over k + delta, 3.5
    /// for goods 2 and 5 by owner choice (DESIGN 17).
    pub convenience: [f64; N_GOODS],
    /// P146 gain g_s of each stored good's market expectation pi^e_og per month (ADR 0091): 0.006,
    /// P45's start, the constant learning gain of Milani (2007), 0.0183 a quarter (DESIGN 17;
    /// owner approval 2026-10-31).
    pub storage_gain: f64,
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

    /// kappa1_cell of ADR 0263: kappa1 for a market whose loop gain kappa1 J stays within the
    /// share `gain_share` of its J2 limit 2, else the gain that puts it there, 2 m / J. Exactly
    /// kappa1 when uncapped.
    pub fn price_gain(&self, j: f64) -> f64 {
        self.kappa1 * (2.0 * self.gain_share / (self.kappa1 * j)).min(1.0)
    }

    /// These parameters with competitive storage on at the owner-approved values (2026-10-30;
    /// DESIGN 17 P56, P144 to P146): delta 0.003, 0.0005, 0.001, 0 and psi 20, 1.0, 3.571, 5.714
    /// for goods 0, 1, 2, 5. Off by default until a value set passes the stability checks
    /// (ADR 0091, As built).
    #[must_use]
    pub fn with_storage(self) -> Self {
        Self {
            storage_loss: [0.003, 0.0005, 0.001, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            speculation: [20.0, 1.0, 3.571, 0.0, 0.0, 5.714, 0.0, 0.0, 0.0, 0.0],
            ..self
        }
    }

    /// Whether good `g`'s origin markets hold stock by competitive storage (ADR 0091): a
    /// storable good with a positive speculation intensity or storage loss. Otherwise the
    /// stock rule applies, kappa2 term included.
    pub fn stores(&self, g: usize) -> bool {
        STORABLE[g] && (self.speculation[g] > 0.0 || self.storage_loss[g] > 0.0)
    }

    /// The storage order's factor on the static elasticity of an origin's orders (trade SPEC,
    /// Storage loop): (1 + psi zeta (k + delta)) / (1 + delta s) where good `g` is stored, 1
    /// otherwise.
    pub fn order_response(&self, g: usize) -> f64 {
        if !self.stores(g) {
            return 1.0;
        }
        let c = self.convenience[g] * (self.storage_cost[g] + self.storage_loss[g]);
        (1.0 + self.speculation[g] * c) / (1.0 + self.storage_loss[g] * self.coverage[g])
    }

    /// The expected net return x of holding a unit of storable good `g` a month (ADR 0091) at
    /// cover `z` = S / (s_g D) and expectation `pi`: pi + (k + delta) (z^-zeta - 1), the
    /// convenience yield relative to its value at z = 1, so x is 0 bit for bit there at pi = 0.
    pub fn storage_return(&self, g: usize, z: f64, pi: f64) -> f64 {
        let carry = self.storage_cost[g] + self.storage_loss[g];
        pi + carry * (libm::pow(z, -self.convenience[g]) - 1.0)
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
            gain_share: 0.5,
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
            storage_loss: [0.0; N_GOODS],
            storage_cost: [0.009, 0.011, 0.007, 0.0, 0.0, 0.0067, 0.0, 0.0, 0.0, 0.0],
            speculation: [0.0; N_GOODS],
            convenience: [1.8, 13.0, 3.5, 0.0, 0.0, 3.5, 0.0, 0.0, 0.0, 0.0],
            storage_gain: 0.006,
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

    /// ADR 0263: below J_ref = 2 m / kappa1 the gain is kappa1 bit for bit; above it the loop
    /// gain kappa1_cell J is 2 m.
    #[test]
    fn price_gain_caps_at_the_share_of_the_limit() {
        let p = TradeParams::default();
        assert_eq!((p.kappa1, p.gain_share), (0.05, 0.5));
        for j in [0.0, 0.6, 12.5, 16.9, 20.0] {
            assert_eq!(p.price_gain(j).to_bits(), p.kappa1.to_bits(), "J {j}");
        }
        for j in [20.5, 36.9, 42.0, 1e6] {
            let loop_gain = p.price_gain(j) * j;
            assert!((loop_gain - 1.0).abs() < 1e-15, "J {j}: {loop_gain}");
        }
        let p = TradeParams {
            gain_share: 0.25,
            kappa1: 0.1,
            ..p
        };
        assert_eq!(p.price_gain(5.0), 0.1);
        assert!((p.price_gain(10.0) - 0.05).abs() < 1e-17);
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
