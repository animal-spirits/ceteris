//! Internal migration logit (P4 Branch D1, ADR 0109, `specs/pop/SPEC.md`, Migration). A share
//! `mu_move` of each origin cell reconsiders; destinations are drawn by
//! `mu_od = exp((beta V_d - m_od)/nu) / sum_k exp((beta V_k - m_ok)/nu)`, with values `V` from one
//! Caliendo-Dvorkin-Parro Bellman update per tick. Flows are integers by Hamilton apportionment.
//! Kernel only: the pack costs, the cell state and tick system 227 are not built yet, so nothing
//! calls this.

use crate::admission::hamilton;

/// Log-sum-exp of `x`, stable against large magnitudes.
fn logsumexp(x: &[f64]) -> f64 {
    let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    max + libm::log(x.iter().map(|&v| libm::exp(v - max)).sum::<f64>())
}

/// Scaled utilities `(beta V_d - m_od)/nu` over destinations for origin `o`; `cost` is row-major
/// with `cost[o * n + d]`, zero on the diagonal.
fn scaled(v: &[f64], cost: &[f64], o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let n = v.len();
    (0..n)
        .map(|d| (beta * v[d] - cost[o * n + d]) / nu)
        .collect()
}

/// Destination shares `mu_od` for origin `o`, summing to one.
pub fn shares(v: &[f64], cost: &[f64], o: usize, beta: f64, nu: f64) -> Vec<f64> {
    let s = scaled(v, cost, o, beta, nu);
    let lse = logsumexp(&s);
    s.iter().map(|&x| libm::exp(x - lse)).collect()
}

/// One value update: `V_o = u_o + nu ln sum_d exp((beta V_d - m_od)/nu)`.
pub fn update_values(
    v: &[f64],
    flow_utility: &[f64],
    cost: &[f64],
    beta: f64,
    nu: f64,
) -> Vec<f64> {
    (0..v.len())
        .map(|o| flow_utility[o] + nu * logsumexp(&scaled(v, cost, o, beta, nu)))
        .collect()
}

/// Integer flows out of origin `o` (including the diagonal, who stay) for `stock` workers of
/// which a share `mu_move` reconsider. The movers never exceed `stock` and sum exactly.
pub fn flows(
    stock: u64,
    mu_move: f64,
    v: &[f64],
    cost: &[f64],
    o: usize,
    beta: f64,
    nu: f64,
) -> Vec<u64> {
    let movers = libm::round(mu_move * stock as f64) as u64;
    let exact: Vec<f64> = shares(v, cost, o, beta, nu)
        .iter()
        .map(|&s| s * movers as f64)
        .collect();
    hamilton(&exact, &vec![movers; v.len()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn shares_sum_to_one_and_favour_high_value() {
        let v = [1.0, 2.0, 3.0];
        let cost = [0.0; 9];
        let s = shares(&v, &cost, 0, 1.0, 1.0);
        assert!((s.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(s[2] > s[1] && s[1] > s[0]);
    }

    #[test]
    fn infinite_costs_off_the_diagonal_keep_everyone_home() {
        let v = [1.0, 5.0];
        let cost = [0.0, 1e9, 1e9, 0.0];
        assert_eq!(flows(1000, 0.5, &v, &cost, 0, 1.0, 1.0), vec![500, 0]);
    }

    #[test]
    fn zero_mobility_moves_nobody() {
        let v = [1.0, 5.0];
        let cost = [0.0; 4];
        assert_eq!(flows(1000, 0.0, &v, &cost, 0, 1.0, 1.0), vec![0, 0]);
    }

    #[test]
    fn value_update_is_a_contraction_for_beta_below_one() {
        let cost = [0.0, 0.5, 0.5, 0.0];
        let u = [1.0, 2.0];
        let mut v = vec![0.0, 0.0];
        for _ in 0..400 {
            v = update_values(&v, &u, &cost, 0.9, 1.0);
        }
        let next = update_values(&v, &u, &cost, 0.9, 1.0);
        assert!(v.iter().zip(&next).all(|(a, b)| (a - b).abs() < 1e-9));
    }

    proptest! {
        #[test]
        fn flows_sum_to_movers(
            stock in 0u64..1_000_000,
            mu in 0.0f64..1.0,
            v in proptest::collection::vec(-5.0f64..5.0, 4),
            c in proptest::collection::vec(0.0f64..5.0, 16),
        ) {
            let f = flows(stock, mu, &v, &c, 1, 0.95, 0.7);
            prop_assert_eq!(f.iter().sum::<u64>(), libm::round(mu * stock as f64) as u64);
        }
    }
}
