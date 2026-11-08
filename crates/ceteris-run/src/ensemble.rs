//! Member draws of an ensemble (ADR 0322, `specs/workbench/SPEC.md`, Ensembles and common
//! random numbers). Member k draws each uncertain parameter from its register prior with a
//! stream keyed by (seed k, `PARAM_DRAW`, parameter id), so a draw depends on nothing but k
//! and the parameter: a branch and its baseline with the same k share it, and adding a
//! parameter to the table moves no other draw. The draws come out as tick-0 `Param` edits, so
//! they are in the log.

use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;

use crate::scenario::{Edit, EditKind, EditSource};

const PARAM_DRAW: SystemId = SystemId(0xFFF3);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Prior {
    Uniform {
        lo: f64,
        hi: f64,
    },
    /// Uniform in the logarithm.
    LogUniform {
        lo: f64,
        hi: f64,
    },
}

impl Prior {
    /// The value at quantile `u` in [0, 1).
    pub fn quantile(self, u: f64) -> f64 {
        match self {
            Self::Uniform { lo, hi } => lo + u * (hi - lo),
            Self::LogUniform { lo, hi } => {
                libm::exp(libm::log(lo) + u * (libm::log(hi) - libm::log(lo)))
            }
        }
    }
}

/// One drawn parameter. `id` keys the random stream and is never reused; `target` is the
/// edit target the draw is applied to.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamPrior {
    pub id: u32,
    pub target: String,
    pub prior: Prior,
}

/// The dynamic parameters drawn at present (`specs/workbench/SPEC.md`, stream ids 1 to 4).
pub fn register_priors() -> Vec<ParamPrior> {
    let p = |id, target: &str, prior| ParamPrior {
        id,
        target: target.into(),
        prior,
    };
    vec![
        p(1, "trade.kappa1", Prior::LogUniform { lo: 0.05, hi: 0.5 }),
        p(2, "trade.gain_share", Prior::Uniform { lo: 0.0, hi: 1.0 }),
        p(3, "econ.kappa_w", Prior::Uniform { lo: 0.05, hi: 0.5 }),
        p(4, "econ.eta", Prior::Uniform { lo: 0.1, hi: 1.0 }),
    ]
}

/// Member `k`'s draw of `priors` as tick-0 edits, in table order.
pub fn member_edits(seed: Seed, priors: &[ParamPrior]) -> Vec<Edit> {
    priors
        .iter()
        .map(|p| {
            let u = seed
                .stream(0, PARAM_DRAW, Kind(0), EntityId(p.id))
                .uniform();
            Edit {
                tick: 0,
                source: EditSource::Draw,
                god: false,
                kind: EditKind::Param,
                target: p.target.clone(),
                value: p.prior.quantile(u),
                query: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Vec<ParamPrior> {
        vec![
            ParamPrior {
                id: 1,
                target: "kappa1".into(),
                prior: Prior::LogUniform { lo: 0.05, hi: 0.5 },
            },
            ParamPrior {
                id: 2,
                target: "eta".into(),
                prior: Prior::Uniform { lo: 0.1, hi: 1.0 },
            },
        ]
    }

    #[test]
    fn draws_lie_inside_their_priors_and_vary_with_k() {
        let draws: Vec<_> = (0..200).map(|k| member_edits(Seed(k), &table())).collect();
        for d in &draws {
            assert!((0.05..=0.5).contains(&d[0].value));
            assert!((0.1..=1.0).contains(&d[1].value));
            assert!(
                d.iter()
                    .all(|e| e.tick == 0 && e.source == EditSource::Draw)
            );
        }
        assert_ne!(draws[0], draws[1]);
        let mean = draws.iter().map(|d| d[1].value).sum::<f64>() / 200.0;
        assert!((mean - 0.55).abs() < 0.06, "mean {mean}");
    }

    /// The draw is a function of (k, parameter), not of the branch: a branch and its baseline
    /// build the same edits.
    #[test]
    fn member_draws_are_shared_by_branch_and_baseline() {
        for k in 0..20 {
            assert_eq!(
                member_edits(Seed(k), &table()),
                member_edits(Seed(k), &table())
            );
        }
    }

    #[test]
    fn adding_a_parameter_moves_no_other_draw() {
        let mut longer = table();
        longer.insert(
            0,
            ParamPrior {
                id: 9,
                target: "rho".into(),
                prior: Prior::Uniform { lo: 0.0, hi: 1.0 },
            },
        );
        for k in 0..20 {
            let (a, b) = (
                member_edits(Seed(k), &table()),
                member_edits(Seed(k), &longer),
            );
            assert_eq!(a, b[1..]);
        }
    }
}
