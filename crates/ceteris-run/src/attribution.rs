//! Attribution (14.7, ADR 0049): Shapley values of a set of interventions or channels over a
//! value function, exact up to [`EXACT_MAX`] players and sampled by permutations beyond, and
//! the ranking behind `blame`.

use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;

pub const EXACT_MAX: usize = 5;
const PERMUTATIONS: SystemId = SystemId(0xFFF2);

/// Shapley value of each of `n` players; `v` maps a coalition (bit i set: player i in) to its
/// value, with `v(0)` the empty coalition. Exact for `n <= EXACT_MAX`, otherwise the mean over
/// `samples` random orderings drawn from the counter RNG keyed by `seed`. Either way the values
/// sum to `v(all) - v(0)`.
pub fn shapley(n: usize, v: impl Fn(u32) -> f64, seed: Seed, samples: u32) -> Vec<f64> {
    assert!(n > 0 && n <= 31, "1 to 31 players");
    if n <= EXACT_MAX {
        exact(n, v)
    } else {
        sampled(n, v, seed, samples)
    }
}

fn exact(n: usize, v: impl Fn(u32) -> f64) -> Vec<f64> {
    let fact: Vec<f64> = (0..=n)
        .map(|k| (1..=k).map(|i| i as f64).product())
        .collect();
    (0..n)
        .map(|i| {
            (0..1u32 << n)
                .filter(|s| s >> i & 1 == 0)
                .map(|s| {
                    let k = s.count_ones() as usize;
                    fact[k] * fact[n - k - 1] / fact[n] * (v(s | 1 << i) - v(s))
                })
                .sum()
        })
        .collect()
}

fn sampled(n: usize, v: impl Fn(u32) -> f64, seed: Seed, samples: u32) -> Vec<f64> {
    let mut sum = vec![0.0; n];
    for r in 0..samples {
        let mut s = seed.stream(r, PERMUTATIONS, Kind(0), EntityId(0));
        let mut order: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            order.swap(i, (s.next_u64() % (i as u64 + 1)) as usize);
        }
        let (mut coalition, mut value) = (0u32, v(0));
        for &p in &order {
            coalition |= 1 << p;
            let next = v(coalition);
            sum[p] += next - value;
            value = next;
        }
    }
    sum.iter().map(|x| x / f64::from(samples)).collect()
}

/// Indices of `effects` from the largest absolute effect down, ties by index.
pub fn blame(effects: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..effects.len()).collect();
    order.sort_by(|&a, &b| {
        effects[b]
            .abs()
            .total_cmp(&effects[a].abs())
            .then(a.cmp(&b))
    });
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game with an interaction: players 0 and 1 only pay together.
    fn game(s: u32) -> f64 {
        let has = |i: u32| f64::from(s >> i & 1);
        3.0 * has(2) + 4.0 * has(0) * has(1) + has(3)
    }

    #[test]
    fn channel_shares_sum_to_total_within_shapley_tolerance() {
        let shares = shapley(4, game, Seed(1), 0);
        let total = game(0b1111) - game(0);
        assert!((shares.iter().sum::<f64>() - total).abs() < 1e-12);
        assert!((shares[0] - 2.0).abs() < 1e-12 && (shares[1] - 2.0).abs() < 1e-12);
        assert!((shares[2] - 3.0).abs() < 1e-12);
    }

    #[test]
    fn sampled_shares_sum_exactly_and_approach_the_exact_ones() {
        let v = |s: u32| {
            (0..8)
                .map(|i| f64::from(i + 1) * f64::from(s >> i & 1))
                .sum::<f64>()
                + 2.0 * f64::from(s & 3 == 3)
        };
        let shares = shapley(8, v, Seed(4), 400);
        let total = v(0xFF) - v(0);
        assert!((shares.iter().sum::<f64>() - total).abs() < 1e-9);
        // additive part exact; the interaction splits between players 0 and 1
        assert!((shares[5] - 6.0).abs() < 1e-9);
        assert!((shares[0] - 2.0).abs() < 0.3 && (shares[1] - 3.0).abs() < 0.3);
        assert_eq!(shares, shapley(8, v, Seed(4), 400));
    }

    #[test]
    fn shutting_every_channel_gives_zero_effect() {
        // the effect with every channel shut is the value of the empty coalition
        assert_eq!(game(0), 0.0);
        let shares = shapley(3, |s| f64::from(s.count_ones()) * 0.0, Seed(1), 0);
        assert!(shares.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn blame_ranks_planted_edit_first() {
        let effects = [0.01, -0.02, 0.9, 0.0, -0.3];
        assert_eq!(blame(&effects), [2, 4, 1, 0, 3]);
    }
}
