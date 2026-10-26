//! Economic parameters with their register IDs (`specs/econ/SPEC.md`, Parameters).

use ceteris_trade::Rationing;

/// The multi-sector PIGL of Alder, Boppart and Mueller (2022), eqs. (19), (21) and (22), over
/// the categories of `demand::CATEGORY`. Start values: their pooled PIGL estimates (Tables 3
/// and A3, column 3), `specs/econ/SPEC.md`, Final demand.
#[derive(Clone, Debug, PartialEq)]
pub struct Pigl {
    /// P46 substitution sigma_B of the aggregator B(P) of the homothetic term.
    pub sigma_b: f64,
    /// P46 substitution phi of the aggregator Dtilde(P); 0 is its Leontief limit, where the
    /// aggregator is linear, sum_c theta_c P_c.
    pub phi: f64,
    /// P46 curvature gamma of D(P) in Dtilde(P) / B(P).
    pub gamma: f64,
    /// P46 weights theta_c of Dtilde(P), categories A, M, S; they sum to 1.
    pub theta: [f64; 3],
    /// P47 Engel parameter eps.
    pub eps: f64,
}

impl Default for Pigl {
    fn default() -> Self {
        Self {
            sigma_b: 0.26,
            phi: 0.0,
            gamma: 0.71,
            theta: [0.634, 0.082, 0.284],
            eps: 0.71,
        }
    }
}

/// S09: which input coefficients enterprises run on (ADR 0020).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Technology {
    /// Each enterprise's own coefficients, `EntData::coef`, fixed by its capacity vintages.
    PuttyClay,
    /// Every enterprise on its country's ICIO column.
    FixedLeontief,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EconParams {
    /// P06 goods substitution sigma within a final demand category, prior 0.3 to 0.9.
    pub sigma_demand: f64,
    /// P46 and P47: PIGL across final demand categories (ADR 0021).
    pub pigl: Pigl,
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
    /// S11 sharing of a shortfall between buyer classes, at origin markets and at what the
    /// same-tick chain leaves of local output: `Priority` (default, DESIGN 8.2),
    /// `Proportional` or `Mixed` (`specs/trade/SPEC.md`, Deviations, rationing row).
    pub rationing: Rationing,
    /// S09 input coefficients: `PuttyClay` (default) or `FixedLeontief`.
    pub technology: Technology,
    /// P148 loss write-off threshold M (ADR 0098): an enterprise keeps retained losses up to M
    /// months of its base sales; the rest is written off. Province markets keep the signed
    /// rule. An infinite M turns the write-off off.
    pub write_off_months: f64,
}

impl Default for EconParams {
    fn default() -> Self {
        Self {
            sigma_demand: 0.6,
            pigl: Pigl::default(),
            sigma_fl: 0.8,
            kappa_w: 0.1,
            delta_w: 0.5,
            eta: 0.3,
            edu_eff: [1.0, 1.5, 2.3],
            frictionless: false,
            rationing: Rationing::Priority,
            technology: Technology::PuttyClay,
            write_off_months: 3.0,
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
