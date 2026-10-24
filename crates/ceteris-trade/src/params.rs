//! Trade parameters with their register IDs (`specs/trade/SPEC.md`, Parameters).

use crate::goods::N_GOODS;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeParams {
    /// P01 trade elasticity theta, prior 4 to 8.
    pub theta: f64,
    /// P02 inventory target coverage s_g in months; local goods 0.
    pub coverage: [f64; N_GOODS],
    /// P03a price response to the demand-supply gap, per month.
    pub kappa1: f64,
    /// P03b price response to the inventory gap, per month.
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
            kappa1: 0.2,
            kappa2: 0.1,
            c_max: 0.1,
            border_nu: 0.3,
            time_cost: [
                0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010, 0.0, 0.0, 0.0,
            ],
        }
    }
}
