//! The moving reference of ADR 0243: the frictionless equilibrium at each date of the trend
//! path. Country i's effective labour is A_i L_i ((1 + g_i)(1 + n_i))^t; prices and wages are
//! detrended, so the price trend pi* does not enter. Each solve starts from the previous root.

use crate::io::Inputs;
use crate::model::Model;
use crate::solve::{self, Equilibrium, SolveError, TOL};

/// `inp` at month `t` of the path: labour supply grows at (1 + n_i)^t, enterprise productivity
/// at (1 + g_i)^t. Zero trends return `inp` unchanged.
pub fn at_month(inp: &Inputs, t: u32) -> Inputs {
    let mut out = inp.clone();
    if inp.trends.is_zero() {
        return out;
    }
    let t = f64::from(t);
    for (p, l) in out.labour_supply.iter_mut().enumerate() {
        *l *= (1.0 + inp.trends.n[inp.province_country[p]]).powf(t);
    }
    // pops' effective labour shares of their province's wages are unchanged by a common scale
    for (e, a) in out.ent_a.iter_mut().enumerate() {
        let c = inp.province_country[inp.ent_province[e]];
        *a *= (1.0 + inp.trends.g[c]).powf(t);
    }
    out
}

/// The equilibria at `months`, ascending, each solved from the previous root.
pub fn solve(inp: &Inputs, months: &[u32]) -> Result<Vec<Equilibrium>, SolveError> {
    let mut out: Vec<Equilibrium> = Vec::with_capacity(months.len());
    for &t in months {
        let at = at_month(inp, t);
        let m = Model::new(&at).map_err(SolveError::NotSteady)?;
        let x0 = out.last().map(|e| e.x.as_slice());
        out.push(solve::equilibrium(&m, x0, TOL, None)?);
    }
    Ok(out)
}
