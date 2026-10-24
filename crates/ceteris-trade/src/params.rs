//! Trade parameters with their register IDs (`specs/trade/SPEC.md`, Parameters).

use crate::goods::N_GOODS;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeParams {
    /// P01 trade elasticity theta_g per good, local goods 0 (unused). Prior 2.62 to 12.55, the
    /// range of the class values (ADR 0018). Each traded good carries its class's theta
    /// (ADR 0023): Caliendo and Parro (2015, Table 1, 99% sample) sector estimates averaged
    /// over the HS lines of the good with BACI 2000 trade weights (`ceteris.packs.econ.theta`):
    /// agriculture 9.11, crude oil and gas 13.53, coal and other mining 13.53, food 2.62, light
    /// manufacturing 6.73, chemicals, refined fuels and metals 12.64, machinery, electronics and
    /// vehicles 5.33; grouped into 3 classes within a factor 1.5, each at the trade-weighted
    /// mean of its goods, to two decimals.
    pub theta: [f64; N_GOODS],
    /// P01 theta class of each good (ADR 0023), 0 for the lowest theta; local goods 0
    /// (unused). Kernels stay per good, since weight classes are goods in P2 (ADR 0018); the
    /// map lets goods of one class share kernels once weight classes merge.
    pub theta_class: [u8; N_GOODS],
    /// P02 inventory target coverage s_g in months; local goods 0.
    pub coverage: [f64; N_GOODS],
    /// P03a price response to the demand-supply gap, per month. Prior 0.01 to 0.5 (widened
    /// from 0.05 by owner decision, 2026-10-24, for stability), default 0.05 (owner decision,
    /// 2026-10-24, MIGRATION Task 6: at 0.02 the expectation loop of ADR 0019 breaks V1 for
    /// every gain in the P45 prior). The derived stability region (`specs/trade/SPEC.md`,
    /// Stability region) bounds it at 0.124 to 0.145 for the theta-12.55 goods, 0.230 to 0.278
    /// at theta 5.88 and 0.368 to 0.507 for food. Growth factor per tick of a small deviation
    /// from the fixture's GE equilibrium between ticks 300 and 600 at the per-good thetas,
    /// kappa2 = 0 (MIGRATION Task 6; frictionless / default): at the defaults (pi^e gain 0.006)
    /// 0.9963 / 0.9969; with pi^e = 0, kappa1 0.02: 0.9954 / 0.9954; 0.05: 0.9862 / 0.9871;
    /// 0.1: 0.9718 / 0.9756; 0.2: 1.0000 / 1.0000 (oscillates), the same with every crossing
    /// lag at 3 months (the full pack's cap).
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
    /// P03c price step clip in log points per month, also the wage step clip; every tick it
    /// binds is counted per market (`Markets::clip_ticks`) and per wage.
    pub c_max: f64,
    /// P45 expectation gain g: pi^e <- pi^e + g (ln(I_t / I_{t-1}) - pi^e) per month, I the
    /// world price index (ADR 0019). Prior 0.004 to 0.65: Poledna et al. (2023) learn an AR(1)
    /// on inflation by OLS over an expanding sample (gain 1/n, about 0.004 to 0.006 per month
    /// over their reference quarters); the adaptive heuristic of Anufriev and Hommes (2012,
    /// Table 1) weights the last observation 0.65 per period. Start 0.006, the Poledna end
    /// (owner decision, 2026-10-24, MIGRATION Task 6).
    pub expectation_gain: f64,
    /// P05 non-tariff border friction nu, ad valorem equivalent.
    pub border_nu: f64,
    /// P43 ad valorem cost of a day in transit, lambda_g.
    pub time_cost: [f64; N_GOODS],
    /// P44 route-choice dispersion theta_r (ADR 0017): edge factors `a = t^-theta_r`, route
    /// costs `tau = [(I - A)^-1]^(-1/theta_r)`. Prior 41 to 182 (Fan, Lu and Luo 2021: 111.5,
    /// s.e. 35.4), value its point estimate. Unused in P2: route choice is deferred (ADR 0017),
    /// since no value in the prior brings the spectral radius below 1 on the P1 graph, and
    /// `Network::route_taus` stays `None`.
    pub theta_r: f64,
}

impl Default for TradeParams {
    fn default() -> Self {
        Self {
            theta: [12.55, 12.55, 12.55, 2.62, 5.88, 12.55, 5.88, 0.0, 0.0, 0.0],
            theta_class: [2, 2, 2, 0, 1, 2, 1, 0, 0, 0],
            coverage: [3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0],
            kappa1: 0.05,
            kappa2: 0.0,
            c_max: 0.1,
            expectation_gain: 0.006,
            border_nu: 0.3,
            time_cost: [
                0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010, 0.0, 0.0, 0.0,
            ],
            theta_r: 111.5,
        }
    }
}
