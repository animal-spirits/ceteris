//! Balanced Sinkhorn on a dense kernel: the S02 full solve and the cross-check of destination
//! sourcing on small worlds (`specs/trade/SPEC.md`, Equilibrium nesting; ADR 0016). With both
//! marginals hard, flows have the form a_o K_od b_d; sourcing has that form (a_o = w_o,
//! b_d = E_d / Phi_d), so at the equilibrium the two coincide.

use crate::io::{Inputs, Mat};

pub const MAX_ITER: usize = 100_000;
pub const TOL: f64 = 1e-14;

/// (P, P) K_od of traded good `g`: 1 at home, the direct leg within a country, and the sum over
/// gateway pairs of K_oa K_ab K_bd across borders. `net_of_tariff` divides each crossing by its
/// 1 + t: the kernel of what origins receive rather than what buyers pay.
pub fn dense_kernel(inp: &Inputs, g: usize, net_of_tariff: bool) -> Mat {
    let (n_p, n_n) = (inp.n_provinces(), inp.n_gateways());
    let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
    let mut k = Mat::identity(n_p);
    for i in (0..dom.good.len()).filter(|&i| dom.good[i] == g) {
        k[(dom.row[i], dom.col[i])] = dom.k[i];
    }
    let mut access = Mat::zeros(n_p, n_n);
    for i in (0..acc.good.len()).filter(|&i| acc.good[i] == g) {
        access[(acc.row[i], acc.col[i])] = acc.k[i];
    }
    let mut crossing = Mat::zeros(n_n, n_n);
    for i in (0..cro.good.len()).filter(|&i| cro.good[i] == g) {
        crossing[(cro.row[i], cro.col[i])] = if net_of_tariff {
            cro.k[i] * cro.inv_duty[i]
        } else {
            cro.k[i]
        };
    }
    let routed = access.matmul(&crossing).matmul(&access.transpose());
    for (v, r) in k.data.iter_mut().zip(&routed.data) {
        *v += r;
    }
    k
}

fn divide_where_positive(num: &[f64], den: &[f64]) -> Vec<f64> {
    num.iter()
        .zip(den)
        .map(|(&n, &d)| if d > 0.0 { n / d } else { 0.0 })
        .collect()
}

/// Flows a_o K_od b_d with rows summing to `supply` and columns to `demand` (equal totals); zero
/// rows and columns stay zero. Iterates until both marginals hold to [`TOL`] relative.
pub fn balanced(k: &Mat, supply: &[f64], demand: &[f64]) -> Result<Mat, String> {
    let kt = k.transpose();
    let mut b = vec![1.0; demand.len()];
    let total: f64 = supply.iter().sum();
    for _ in 0..MAX_ITER {
        let a = divide_where_positive(supply, &k.matvec(&b));
        b = divide_where_positive(demand, &kt.matvec(&a));
        let kb = k.matvec(&b);
        let gap = a
            .iter()
            .zip(&kb)
            .zip(supply)
            .map(|((a, kb), s)| (a * kb - s).abs())
            // NaN propagates, as numpy's max: a NaN gap never converges.
            .fold(0.0, |m: f64, v| if v > m || v.is_nan() { v } else { m });
        if gap <= TOL * total {
            return Ok(Mat::from_fn(k.rows, k.cols, |o, d| a[o] * k[(o, d)] * b[d]));
        }
    }
    Err(format!(
        "Sinkhorn did not converge in {MAX_ITER} iterations"
    ))
}
