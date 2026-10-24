//! Trade parameters with their register IDs (`specs/trade/SPEC.md`, Parameters).

use crate::goods::N_GOODS;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeParams {
    /// P01 trade elasticity theta, prior 4 to 8.
    pub theta: f64,
    /// P02 inventory target coverage s_g in months; local goods 0.
    pub coverage: [f64; N_GOODS],
    /// P03a price response to the demand-supply gap, per month. Prior 0.01 to 0.5 (widened
    /// from 0.05 by owner decision, 2026-10-24, for stability), default 0.02. Growth factor
    /// per tick of a small deviation from the fixture's GE equilibrium between ticks 300 and
    /// 600 (MIGRATION Task 3: origin clearing, buyers at q_d, order-up-to with transit lead;
    /// kappa2 = 0, frictionless / default): kappa1 0.02: 0.9956 / 0.9958; 0.05: 0.9864 /
    /// 0.9873; 0.1: 0.9723 / 0.9759; 0.2: 1.0000 / 0.9942 (frictionless oscillates). The
    /// same with every crossing lag at 3 months (the full pack's cap) up to kappa1 0.1;
    /// 0.2: 1.0001 / 1.0014.
    pub kappa1: f64,
    /// P03b price response to the inventory gap, per month. Prior 0 to 0.5 (widened from 0.05
    /// by owner decision, 2026-10-24), default 0: the inventory term is off and inventories
    /// are untargeted in P2. kappa2 = 0.05 is unstable in the default mode at kappa1 0.02
    /// (1.0023 as built, 1.0082 at lag 3) and 0.05 (1.0054), decays at 0.1 (0.9886) and 0.2
    /// (0.9914) as built but not at lag 3 (1.0043, 1.0000); frictionless it decays up to
    /// kappa1 0.1 as built (0.9979, 0.9736, 0.9943).
    pub kappa2: f64,
    /// P03c price step clip in log points per month.
    pub c_max: f64,
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
            theta: 5.0,
            coverage: [3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0],
            kappa1: 0.02,
            kappa2: 0.0,
            c_max: 0.1,
            border_nu: 0.3,
            time_cost: [
                0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010, 0.0, 0.0, 0.0,
            ],
            theta_r: 111.5,
        }
    }
}
