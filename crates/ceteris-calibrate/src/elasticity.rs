//! The elasticities of each market's price loop at the calibrated point (ADR 0263; trade SPEC,
//! Stability region, Per-market gain): J = E + eta eps / (2 - eta), which the world combines with
//! the run's eta to cap the market's price gain.

use ceteris_reference::io::{Inputs, Mat};
use ceteris_reference::model::{Factors, Model, Sourcing, cells, good_major};

/// E per cell (P, G): for an active traded origin 1 + theta_g - (theta_g + 1) Pi, Pi the cell's
/// share of its buyers' purchases weighted by the units it sells to each, the own-price
/// elasticity of orders from buyers who need fixed delivered units (Leontief inputs, sigma_d 0)
/// at the buyer price Gamma P_d (ADR 0200); for an active local cell sigma (P06); 0 elsewhere.
/// `spend` (P, G) is spending by destination, `src` the sourcing at the calibrated prices. Pi
/// comes from two passes over the kernel legs, one destination sum of the spending and one with
/// squared leg factors, so a pair joined by several routes leaves out the cross terms between
/// them. A traded cell that reaches no buyer with spending has Pi 0, so E = 1 + theta_g, the
/// study's value and the limit of a vanishing share.
pub fn demand(m: &Model, src: &Sourcing, spend: &Mat) -> Mat {
    let inp = m.inp;
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let ef = good_major(spend);
    let traded_reach = |j: usize| inp.traded[j / n_p] && src.phi[j] > 0.0;
    let reach: Vec<f64> = (0..n_g * n_p)
        .map(|j| {
            if traded_reach(j) {
                ef[j] / src.phi[j]
            } else {
                0.0
            }
        })
        .collect();
    let (dom, acc, cro) = (&inp.domestic.k, &inp.access.k, &inp.crossing.k);
    let cro_net: Vec<f64> = cro
        .iter()
        .zip(&inp.crossing.inv_duty)
        .map(|(k, d)| k * d)
        .collect();
    let t = m.destination_sums(
        &reach,
        Factors {
            dom,
            acc,
            cro: &cro_net,
        },
    );
    let square = |k: &[f64]| -> Vec<f64> { k.iter().map(|k| k * k).collect() };
    let cro_sq: Vec<f64> = cro_net.iter().zip(cro).map(|(n, k)| n * k).collect();
    let reach_phi: Vec<f64> = (0..n_g * n_p)
        .map(|j| {
            if traded_reach(j) {
                reach[j] / src.phi[j]
            } else {
                0.0
            }
        })
        .collect();
    let t2 = m.destination_sums(
        &reach_phi,
        Factors {
            dom: &square(dom),
            acc: &square(acc),
            cro: &cro_sq,
        },
    );
    let e: Vec<f64> = (0..n_g * n_p)
        .map(|j| {
            let (g, p) = (j / n_p, j % n_p);
            if !inp.active[p][g] {
                0.0
            } else if !inp.traded[g] {
                inp.sigma_demand
            } else {
                let pi = if t[j] > 0.0 {
                    src.w[j] * t2[j] / t[j]
                } else {
                    0.0
                };
                1.0 + inp.theta[g] - (inp.theta[g] + 1.0) * pi
            }
        })
        .collect();
    cells(&e, n_p, n_g)
}

/// eps per cell (P, G): the largest over the cell's enterprises of s_L sigma_FL / ((1 - s_L)
/// a_VA), the elasticity of the labour target's output to the own price (ADR 0263, As built: the
/// maximum, not the output-weighted mean); 0 where no enterprise produces. `s_l` and `a_va` per
/// enterprise.
pub fn supply(inp: &Inputs, s_l: &[f64], a_va: &[f64]) -> Mat {
    let mut eps = Mat::zeros(inp.n_provinces(), inp.n_goods());
    for e in 0..inp.ent_good.len() {
        let x = s_l[e] * inp.sigma_fl / ((1.0 - s_l[e]) * a_va[e]);
        let cell = &mut eps[(inp.ent_province[e], inp.ent_good[e])];
        *cell = cell.max(x);
    }
    eps
}
