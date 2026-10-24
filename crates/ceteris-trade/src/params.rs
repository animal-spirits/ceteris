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
    /// per tick of a small deviation from the fixture's GE equilibrium (Task 7b, capacity
    /// input orders, kappa2 = 0, frictionless / default): kappa1 0.2: 1.318 / 1.308;
    /// 0.1: 1.090 / 1.069; 0.05: 0.981 / 0.978; 0.02: 0.989 / 0.991. With every crossing lag
    /// at 3 months (the full pack's cap): 0.05: 1.059 / 1.058; 0.02: 0.987 / 0.997.
    pub kappa1: f64,
    /// P03b price response to the inventory gap, per month. Prior 0 to 0.5 (widened from 0.05
    /// by owner decision, 2026-10-24), default 0: the inventory term is off and inventories
    /// are untargeted in P2. Any kappa2 > 0 is unstable on the fixture: at kappa1 0.05, kappa2
    /// 0.01: 1.044 / 1.038, 0.05: 1.175 / 1.197 (also with inventory position control: 1.112
    /// / 1.121), because the stock integrates delayed routed supply and feeds the money and
    /// trade-balance loop.
    pub kappa2: f64,
    /// P03c price step clip in log points per month.
    pub c_max: f64,
    /// P05 non-tariff border friction nu, ad valorem equivalent.
    pub border_nu: f64,
    /// P41 ad valorem cost of a day in transit, lambda_g.
    pub time_cost: [f64; N_GOODS],
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
        }
    }
}
