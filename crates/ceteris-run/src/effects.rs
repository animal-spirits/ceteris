//! Paired effects over seeds (Section 14.5): the per-seed difference of a declared outcome
//! between a branch and its baseline, with deterministic bootstrap intervals, quantiles, event
//! probabilities and the sign-robustness mask of a per-province effect.

use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;

/// Stream of the bootstrap resampling, apart from every system's.
const BOOTSTRAP: SystemId = SystemId(0xFFF1);

/// Outcome of the branch minus outcome of the baseline, member by member.
pub fn paired(branch: &[f64], baseline: &[f64]) -> Vec<f64> {
    assert_eq!(
        branch.len(),
        baseline.len(),
        "one baseline member per branch member"
    );
    branch.iter().zip(baseline).map(|(b, a)| b - a).collect()
}

pub fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
}

/// Linear-interpolated quantile `q` in [0, 1] of `x` (any order).
pub fn quantile(x: &[f64], q: f64) -> f64 {
    let mut s = x.to_vec();
    s.sort_by(f64::total_cmp);
    let at = q * (s.len() - 1) as f64;
    let (i, frac) = (at.floor() as usize, at.fract());
    if i + 1 < s.len() {
        s[i] + frac * (s[i + 1] - s[i])
    } else {
        s[i]
    }
}

/// Percentile bootstrap of the mean: `reps` resamples with replacement, drawn from the counter
/// RNG keyed by `seed`, so the interval is a pure function of the data and the seed. `level` is
/// the coverage, e.g. 0.9.
pub fn bootstrap_mean(x: &[f64], seed: Seed, reps: u32, level: f64) -> Interval {
    let n = x.len();
    let means: Vec<f64> = (0..reps)
        .map(|r| {
            let mut s = seed.stream(r, BOOTSTRAP, Kind(0), EntityId(0));
            let sum: f64 = (0..n).map(|_| x[(s.next_u64() % n as u64) as usize]).sum();
            sum / n as f64
        })
        .collect();
    let tail = (1.0 - level) / 2.0;
    Interval {
        mean: mean(x),
        lo: quantile(&means, tail),
        hi: quantile(&means, 1.0 - tail),
    }
}

/// Share of members with `outcome` true.
pub fn probability(outcome: &[bool]) -> f64 {
    outcome.iter().filter(|&&b| b).count() as f64 / outcome.len() as f64
}

/// Whether the share of members agreeing with the mean's sign reaches `threshold`; a zero mean
/// is never robust.
pub fn sign_robust(diffs: &[f64], threshold: f64) -> bool {
    let m = mean(diffs);
    if m == 0.0 {
        return false;
    }
    let agree = diffs
        .iter()
        .filter(|d| d.signum() == m.signum() && **d != 0.0)
        .count();
    agree as f64 / diffs.len() as f64 >= threshold
}

/// Share of members whose difference has the mean's sign (zero when the mean is zero).
pub fn sign_share(diffs: &[f64]) -> f64 {
    let m = mean(diffs);
    if m == 0.0 {
        return 0.0;
    }
    let agree = diffs
        .iter()
        .filter(|d| **d != 0.0 && d.signum() == m.signum())
        .count();
    agree as f64 / diffs.len() as f64
}

/// One row of the `effects` table: the paired effect of one outcome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect {
    pub interval: Interval,
    pub p05: f64,
    pub p50: f64,
    pub p95: f64,
    pub sign_share: f64,
}

pub const BOOTSTRAP_REPS: u32 = 1000;
pub const BOOTSTRAP_LEVEL: f64 = 0.9;

/// The paired effect of `branch` against `baseline` (member by member), with the bootstrap
/// keyed by `seed`.
pub fn effect(branch: &[f64], baseline: &[f64], seed: Seed) -> Effect {
    let d = paired(branch, baseline);
    Effect {
        interval: bootstrap_mean(&d, seed, BOOTSTRAP_REPS, BOOTSTRAP_LEVEL),
        p05: quantile(&d, 0.05),
        p50: quantile(&d, 0.5),
        p95: quantile(&d, 0.95),
        sign_share: sign_share(&d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_row_agrees_with_its_parts() {
        let base = [1.0, 2.0, 3.0, 4.0];
        let branch = [2.0, 3.0, 4.0, 3.0];
        let e = effect(&branch, &base, Seed(3));
        assert_eq!(e.interval.mean, 0.5);
        assert_eq!(e.p50, 1.0);
        assert_eq!(e.sign_share, 0.75);
        assert_eq!(e, effect(&branch, &base, Seed(3)));
    }

    #[test]
    fn paired_effect_of_null_edit_is_exactly_zero() {
        let run = [1.5, 2.25, 3.125];
        let d = paired(&run, &run);
        assert!(d.iter().all(|&x| x == 0.0));
        let i = bootstrap_mean(&d, Seed(1), 200, 0.9);
        assert_eq!((i.mean, i.lo, i.hi), (0.0, 0.0, 0.0));
        assert!(!sign_robust(&d, 0.8));
    }

    #[test]
    fn bootstrap_is_deterministic() {
        let x = [0.4, 1.1, -0.2, 0.9, 0.7, 1.3];
        let a = bootstrap_mean(&x, Seed(9), 500, 0.9);
        assert_eq!(a, bootstrap_mean(&x, Seed(9), 500, 0.9));
        assert_ne!(a, bootstrap_mean(&x, Seed(10), 500, 0.9));
        assert!(a.lo < a.mean && a.mean < a.hi);
    }

    #[test]
    fn pairing_removes_the_shared_member_noise() {
        // member noise common to branch and baseline (CRN) cancels in the paired effect
        let noise = [5.0, -3.0, 8.0, -6.0, 2.0, 0.5];
        let baseline: Vec<f64> = noise.iter().map(|n| 10.0 + n).collect();
        let branch: Vec<f64> = noise.iter().map(|n| 11.0 + n).collect();
        let crn = bootstrap_mean(&paired(&branch, &baseline), Seed(2), 300, 0.9);
        let shifted: Vec<f64> = noise.iter().rev().map(|n| 10.0 + n).collect();
        let independent = bootstrap_mean(&paired(&branch, &shifted), Seed(2), 300, 0.9);
        assert_eq!((crn.lo, crn.hi), (1.0, 1.0));
        assert!(independent.hi - independent.lo > crn.hi - crn.lo);
    }

    #[test]
    fn quantiles_interpolate() {
        let x = [4.0, 1.0, 3.0, 2.0];
        assert_eq!(quantile(&x, 0.0), 1.0);
        assert_eq!(quantile(&x, 1.0), 4.0);
        assert_eq!(quantile(&x, 0.5), 2.5);
    }

    #[test]
    fn probability_counts_members() {
        assert_eq!(probability(&[true, false, true, true]), 0.75);
    }

    #[test]
    fn sign_mask_needs_enough_agreement() {
        let d = [1.0, 2.0, 1.5, -0.5];
        assert!(sign_robust(&d, 0.75));
        assert!(!sign_robust(&d, 0.8));
        assert!(sign_robust(&[-1.0, -2.0], 1.0));
    }
}
