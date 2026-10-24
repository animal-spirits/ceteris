//! Economic parameters with their register IDs (`specs/econ/SPEC.md`, Parameters).

use ceteris_trade::Rationing;

#[derive(Clone, Debug, PartialEq)]
pub struct EconParams {
    /// P06 goods substitution sigma in final demand, prior 0.3 to 0.9.
    pub sigma_demand: f64,
    /// P08 capital-labour (F-L) substitution sigma_FL, prior 0.5 to 1.2.
    pub sigma_fl: f64,
    /// P39 wage adjustment speed kappa_w, per month.
    pub kappa_w: f64,
    /// P40 downward wage rigidity delta_w.
    pub delta_w: f64,
    /// P41 employment adjustment speed eta, per month.
    pub eta: f64,
    /// P42 education efficiency e_k: below secondary, secondary, tertiary.
    pub edu_eff: [f64; 3],
    /// Walrasian wage, eta = 1 and delta_w = 0 (V1 equilibrium nesting).
    pub frictionless: bool,
    /// S11 sharing of an origin shortfall between buyer classes: intermediate users first
    /// (`Priority`, default, DESIGN 8.2) or one fill ratio (`Proportional`).
    pub rationing: Rationing,
}

impl Default for EconParams {
    fn default() -> Self {
        Self {
            sigma_demand: 0.6,
            sigma_fl: 0.8,
            kappa_w: 0.1,
            delta_w: 0.5,
            eta: 0.3,
            edu_eff: [1.0, 1.5, 2.3],
            frictionless: false,
            rationing: Rationing::Priority,
        }
    }
}

impl EconParams {
    pub fn frictionless() -> Self {
        Self {
            frictionless: true,
            eta: 1.0,
            delta_w: 0.0,
            ..Self::default()
        }
    }
}
