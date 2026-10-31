//! Trade parameters with their register IDs (`specs/trade/SPEC.md`, Parameters).

use crate::goods::{Goods, p2};

/// Theta classes of P01 (ADR 0023, Amendment 2026-10-29).
pub const N_CLASSES: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeParams {
    /// The goods these parameters are for: every per-good field has one entry per good.
    pub goods: Goods,
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
    pub theta: Vec<f64>,
    /// P01 theta class of each good (ADR 0023), 0 for the lowest theta; local goods 0
    /// (unused). Kernels stay per good, since weight classes are goods in P2 (ADR 0018); the
    /// map lets goods of one class share kernels once weight classes merge.
    pub theta_class: Vec<u8>,
    /// P150 theta scale: the kernel is built at theta_g times this (ADR 0101). Tariff
    /// experiments report the lane {0.6, 1, 1.4}.
    pub theta_scale: f64,
    /// P02 inventory target coverage s_g in months; local goods 0.
    pub coverage: Vec<f64>,
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
    /// P151 price step clip of the storable commodities (`Goods::storable`), in log points per
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
    /// P05 non-tariff border friction nu per good, ad valorem equivalent (ADRs 0092, 0276): the
    /// default is the prior's start; a calibrated pack carries the values that match the ICIO
    /// 2000 cross-border share of each friction group, a theta class or, in class 3, a good
    /// (`ceteris_calibrate::border`), which `EconInputs::trade_params` puts here. Local goods
    /// never cross a border.
    pub border_nu: Vec<f64>,
    /// P43 ad valorem cost of a day in transit, lambda_g.
    pub time_cost: Vec<f64>,
    /// P44 route-choice dispersion theta_r (ADR 0017): edge factors `a = t^-theta_r`, route
    /// costs `tau = [(I - A)^-1]^(-1/theta_r)`. Prior 41 to 182 (Fan, Lu and Luo 2021: 111.5,
    /// s.e. 35.4), value its point estimate. Unused in P2: route choice is deferred (ADR 0017),
    /// since no value in the prior brings the spectral radius below 1 on the P1 graph, and
    /// `Network::route_taus` stays `None`.
    pub theta_r: f64,
    /// P154 bulk freight rate (ADR 0247): multiplier on the sea cost per tonne-km (P04c, sea and
    /// canal edges) for the legs of each theta class. 1 for classes 0 to 2; class 3 (fuels and
    /// mining) 0.017, the slope of 2000 tanker and dry-bulk voyage rates per tonne against sea
    /// km, 2.05e-4 USD (UNCTAD Review of Maritime Transport 2001 and 2002; trade SPEC,
    /// Parameters), over P04c's 0.0121 (ADR 0269, proposed). On by default by owner ruling (2026-10-30). It reaches the kernel only through
    /// `Network::bulk`.
    pub bulk_sea: [f64; N_CLASSES],
    /// P155 bulk freight rate (ADR 0247): multiplier on the listed and minor port fees (P04m)
    /// per theta class. 1 for classes 0 to 2; class 3 0.40, half the fixed part of the same
    /// rates over the listed-port fee (on by default, as P154).
    pub bulk_port: [f64; N_CLASSES],
    /// P56 physical storage loss delta_g per month at the origin markets of the storable goods
    /// (ADR 0091; `Goods::storable`, 0 elsewhere). Competitive storage is off by default (owner
    /// ruling 2026-10-31: the approved values destabilize the fixture, ADR 0091 As built), so
    /// the default is 0; [`TradeParams::with_storage`] sets the approved values: agriculture 0.003, a world blend of commercial
    /// grain shrink (Hofstrand, ISU A2-33, 1995) and smallholder post-harvest losses (APHLIS);
    /// fuels 0.0005 and coal and mining 0.001 by judgement; chemicals and metals 0 (base metals do
    /// not deteriorate, Cafiero et al. 2011). Owner approval 2026-10-30; refit in P9.
    pub storage_loss: Vec<f64>,
    /// P56 money cost k_g of holding a unit for a month, interest 0.0047 (FRED TB3MS, 2000 mean
    /// 5.82% a year) plus physical storage: 0.009, 0.011, 0.007, 0.0067 (DESIGN 17, P56; weak).
    pub storage_cost: Vec<f64>,
    /// P144 speculation intensity psi_g in months (ADR 0091), 0 by default (storage off); the
    /// approved values ([`TradeParams::with_storage`]) are 20, 1.0, 3.571, 5.714, derived as
    /// lambda s_g / (zeta (k + delta)) from the stock-persistence target lambda = 1 - rho of
    /// Gorton, Hayashi and Rouwenhorst (2013, Table 2); not estimated (DESIGN 17). Good 5's
    /// 5.714 is the source sheet's 40 at zeta 0.5 rescaled to zeta 3.5 (40 x 0.5 / 3.5), where
    /// the formula gives 5.54; approved as rescaled.
    pub speculation: Vec<f64>,
    /// P145 convenience-yield elasticity zeta_g (ADR 0091): 1.8, 13, 3.5, 3.5, the slope of the
    /// basis in inventory of Gorton, Hayashi and Rouwenhorst (2013, Table 3) over k + delta, 3.5
    /// for goods 2 and 5 by owner choice (DESIGN 17).
    pub convenience: Vec<f64>,
    /// P146 gain g_s of each stored good's market expectation pi^e_og per month (ADR 0091): 0.006,
    /// P45's start, the constant learning gain of Milani (2007), 0.0183 a quarter (DESIGN 17;
    /// owner approval 2026-10-31).
    pub storage_gain: f64,
}

impl TradeParams {
    /// Refuses a per-good field without one entry per good of `goods`.
    pub fn check_goods(&self) -> Result<(), String> {
        let n = self.goods.n();
        let lens = [
            ("theta", self.theta.len()),
            ("theta_class", self.theta_class.len()),
            ("coverage", self.coverage.len()),
            ("border_nu", self.border_nu.len()),
            ("time_cost", self.time_cost.len()),
            ("storage_loss", self.storage_loss.len()),
            ("storage_cost", self.storage_cost.len()),
            ("speculation", self.speculation.len()),
            ("convenience", self.convenience.len()),
        ];
        match lens.iter().find(|(_, l)| *l != n) {
            Some((field, l)) => Err(format!("trade parameter {field} has {l} goods, not {n}")),
            None => Ok(()),
        }
    }

    /// The border friction of good `g`.
    pub fn border_nu_of(&self, g: usize) -> f64 {
        self.border_nu[g]
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
        assert_eq!(
            self.goods,
            Goods::p2(),
            "the approved storage values are for the P2 goods"
        );
        Self {
            storage_loss: vec![0.003, 0.0005, 0.001, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            speculation: vec![20.0, 1.0, 3.571, 0.0, 0.0, 5.714, 0.0, 0.0, 0.0, 0.0],
            ..self
        }
    }

    /// Whether good `g`'s origin markets hold stock by competitive storage (ADR 0091): a
    /// storable good with a positive speculation intensity or storage loss. Otherwise the
    /// stock rule applies, kappa2 term included.
    pub fn stores(&self, g: usize) -> bool {
        self.goods.storable[g] && (self.speculation[g] > 0.0 || self.storage_loss[g] > 0.0)
    }

    /// Refuses a storable good with a storage loss and no speculation (P56 delta > 0, P144 psi
    /// = 0): its stock would spoil with nothing targeting it, a slowly growing root of the
    /// storage loop (trade SPEC, Competitive storage). Refuses a stored good without a positive
    /// cover (P02), convenience elasticity (P145) or carrying cost k + delta (P56): the order's
    /// adjustment speed and target stock divide by them (ADR 0280).
    pub fn check_storage(&self) -> Result<(), String> {
        if let Some(g) = (0..self.goods.n()).find(|&g| {
            self.goods.storable[g] && self.storage_loss[g] > 0.0 && self.speculation[g] <= 0.0
        }) {
            return Err(format!(
                "good {g} has a storage loss {} and no speculation intensity: an untargeted \
                 stock that spoils; set P144 psi or P56 delta 0",
                self.storage_loss[g]
            ));
        }
        for g in (0..self.goods.n()).filter(|&g| self.stores(g)) {
            let carry = self.storage_cost[g] + self.storage_loss[g];
            for (name, x) in [
                ("cover s (P02)", self.coverage[g]),
                ("convenience elasticity zeta (P145)", self.convenience[g]),
                ("carrying cost k + delta (P56)", carry),
            ] {
                if x.is_nan() || x <= 0.0 {
                    return Err(format!(
                        "good {g} is stored with {name} {x}: the storage order divides by it"
                    ));
                }
            }
        }
        Ok(())
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

    /// The merchants' adjustment speed lambda_g toward the target stock, per month (ADR 0280):
    /// psi zeta (k + delta) / s_g, the stock-persistence rate 1 - rho of Gorton, Hayashi and
    /// Rouwenhorst (2013) from which P144 psi is derived; 0.144, 0.15, 0.05 and 0.067 for goods
    /// 0, 1, 2 and 5 at the approved values.
    pub fn storage_adjustment(&self, g: usize) -> f64 {
        let carry = self.storage_cost[g] + self.storage_loss[g];
        self.speculation[g] * self.convenience[g] * carry / self.coverage[g]
    }

    /// The stock S* merchants want to hold at buyers' orders `d` and expectation `pi` (ADR
    /// 0280): s_g D max(0, 1 + pi / (zeta (k + delta))), the cover at which ADR 0091's expected
    /// net return pi + (k + delta)(z^-zeta - 1) is zero, to first order in pi (Working 1949). At
    /// pi = 0 it is P02 cover; with no buyers it is 0.
    pub fn storage_target(&self, g: usize, d: f64, pi: f64) -> f64 {
        let carry = self.storage_cost[g] + self.storage_loss[g];
        self.coverage[g] * d * (1.0 + pi / (self.convenience[g] * carry)).max(0.0)
    }

    /// The price step clip of good `g`: P151 for storable commodities, else P03c.
    pub fn price_clip(&self, g: usize) -> f64 {
        if self.goods.storable[g] {
            self.c_max_storable
        } else {
            self.c_max
        }
    }
}

impl Default for TradeParams {
    fn default() -> Self {
        Self {
            goods: Goods::p2(),
            theta: vec![7.68, 16.91, 16.91, 2.91, 7.68, 2.91, 4.14, 0.0, 0.0, 0.0],
            theta_class: vec![2, 3, 3, 0, 2, 0, 1, 0, 0, 0],
            theta_scale: 1.0,
            coverage: vec![3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0],
            kappa1: 0.05,
            gain_share: 0.5,
            kappa2: 0.0,
            c_max: 0.1,
            c_max_storable: 0.3,
            expectation_gain: 0.006,
            border_nu: vec![0.3; p2::N],
            time_cost: vec![
                0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010, 0.0, 0.0, 0.0,
            ],
            theta_r: 111.5,
            bulk_sea: [1.0, 1.0, 1.0, 0.017],
            bulk_port: [1.0, 1.0, 1.0, 0.40],
            storage_loss: vec![0.0; p2::N],
            storage_cost: vec![0.009, 0.011, 0.007, 0.0, 0.0, 0.0067, 0.0, 0.0, 0.0, 0.0],
            speculation: vec![0.0; p2::N],
            convenience: vec![1.8, 13.0, 3.5, 0.0, 0.0, 3.5, 0.0, 0.0, 0.0, 0.0],
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
            Some(([false, false, false, true], 0.017, 0.40))
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

    /// A stored good needs a positive cover, convenience elasticity and carrying cost: the
    /// order's adjustment speed psi zeta (k + delta) / s and target s D (1 + pi / (zeta (k +
    /// delta))) divide by them (ADR 0280). The approved values and storage off pass.
    #[test]
    fn check_storage_refuses_a_stored_good_without_cover_zeta_or_carry() {
        let on = TradeParams::default().with_storage();
        assert!(TradeParams::default().check_storage().is_ok());
        assert!(on.check_storage().is_ok());
        let mut cover = on.clone();
        cover.coverage[1] = 0.0;
        let mut zeta = on.clone();
        zeta.convenience[2] = 0.0;
        let mut carry = on.clone();
        (carry.storage_cost[5], carry.storage_loss[5]) = (0.0, 0.0);
        for (name, p, good) in [("cover", cover, 1), ("zeta", zeta, 2), ("carry", carry, 5)] {
            let err = p.check_storage().expect_err(name);
            assert!(err.starts_with(&format!("good {good} ")), "{name}: {err}");
        }
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
