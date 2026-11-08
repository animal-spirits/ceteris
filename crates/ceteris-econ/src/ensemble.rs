//! Ensemble runner (`specs/workbench/SPEC.md`, Ensembles and common random numbers): member k
//! draws its dynamic parameters from the register priors, and runs as a lone world to the
//! horizon, one member after another, each with the full thread pool.

use std::sync::Arc;

use ceteris_core::invariant::Violation;
use ceteris_core::rng::Seed;
use ceteris_core::snapshot::Hash;
use ceteris_run::Edit;
use ceteris_run::ensemble::{ParamPrior, member_edits};
use ceteris_trade::TradeParams;

use crate::edit::{EditError, apply_due, member_params};
use crate::inputs::EconInputs;
use crate::params::EconParams;
use crate::world::EconWorld;

#[derive(Debug)]
pub enum MemberError {
    Edit(EditError),
    Invariant(Violation),
}

/// One finished member: its draws (the tick-0 edits, for the log), the state hash at the
/// horizon and the caller's outcome read from the final world.
#[derive(Debug)]
pub struct Member<T> {
    pub seed: u64,
    pub edits: Vec<Edit>,
    pub state_hash: Hash,
    pub outcome: T,
}

/// Runs member `seed` of the ensemble to `ticks` over `base` with its drawn parameters, applying
/// the branch's `log` (edits of the branch and its ancestors) at their ticks.
pub fn run_member<T>(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    priors: &[ParamPrior],
    log: &[Edit],
    seed: u64,
    ticks: u32,
    outcome: impl FnOnce(&EconWorld) -> T,
) -> Result<Member<T>, MemberError> {
    let edits = member_edits(Seed(seed), priors);
    let (p, t) = member_params(base, &edits).map_err(MemberError::Edit)?;
    let mut sim = EconWorld::sim(Arc::clone(inputs), p, t);
    while sim.tick() < ticks {
        let t = sim.tick();
        apply_due(sim.world_mut(), log, t).map_err(MemberError::Edit)?;
        sim.step().map_err(MemberError::Invariant)?;
    }
    Ok(Member {
        seed,
        edits,
        state_hash: sim.state_hash(),
        outcome: outcome(sim.world()),
    })
}

/// Members for `seeds`, one after another.
pub fn run<T>(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    priors: &[ParamPrior],
    log: &[Edit],
    seeds: impl IntoIterator<Item = u64>,
    ticks: u32,
    outcome: impl Fn(&EconWorld) -> T,
) -> Result<Vec<Member<T>>, MemberError> {
    seeds
        .into_iter()
        .map(|seed| run_member(inputs, base, priors, log, seed, ticks, &outcome))
        .collect()
}

/// A branch and its baseline (logs given as `(baseline, branch)`) over the same seeds: member k of both shares the draw (CRN).
#[derive(Debug)]
pub struct Pair<T> {
    pub baseline: Vec<Member<T>>,
    pub branch: Vec<Member<T>>,
}

pub fn run_pair<T>(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    priors: &[ParamPrior],
    (baseline_log, branch_log): (&[Edit], &[Edit]),
    seeds: &[u64],
    ticks: u32,
    outcome: impl Fn(&EconWorld) -> T,
) -> Result<Pair<T>, MemberError> {
    let go = |log| {
        run(
            inputs,
            base,
            priors,
            log,
            seeds.iter().copied(),
            ticks,
            &outcome,
        )
    };
    Ok(Pair {
        baseline: go(baseline_log)?,
        branch: go(branch_log)?,
    })
}

/// The declared outcomes read from a final world, by name.
pub fn outcomes(w: &EconWorld) -> Vec<(&'static str, f64)> {
    let n = w.wage.len() as f64;
    vec![
        ("world_price_index", w.world_price_index()),
        ("mean_wage", w.wage.iter().sum::<f64>() / n),
    ]
}

#[cfg(test)]
mod tests {
    use ceteris_run::ensemble::Prior;
    use ceteris_run::{EditKind, EditSource};

    use super::*;
    use crate::fixture::small_world;

    fn priors() -> Vec<ParamPrior> {
        vec![
            ParamPrior {
                id: 1,
                target: "trade.kappa1".into(),
                prior: Prior::LogUniform { lo: 0.05, hi: 0.5 },
            },
            ParamPrior {
                id: 3,
                target: "econ.kappa_w".into(),
                prior: Prior::Uniform { lo: 0.05, hi: 0.5 },
            },
        ]
    }

    fn base() -> (EconParams, TradeParams) {
        (EconParams::default(), TradeParams::default())
    }

    #[test]
    fn member_k_equals_a_lone_run_with_its_draws() {
        let inputs = Arc::new(small_world());
        let members = run(&inputs, &base(), &priors(), &[], 0..3, 8, |_| ()).unwrap();
        for m in &members {
            let (p, t) = member_params(&base(), &m.edits).unwrap();
            let mut lone = EconWorld::sim(Arc::clone(&inputs), p, t);
            lone.run_until(8).unwrap();
            assert_eq!(m.state_hash, lone.state_hash(), "member {}", m.seed);
        }
        assert_ne!(members[0].state_hash, members[1].state_hash);
    }

    #[test]
    fn same_seed_gives_the_same_member_and_no_draws_give_identical_members() {
        let inputs = Arc::new(small_world());
        let a = run_member(&inputs, &base(), &priors(), &[], 5, 6, |_| ()).unwrap();
        let b = run_member(&inputs, &base(), &priors(), &[], 5, 6, |_| ()).unwrap();
        assert_eq!(a.state_hash, b.state_hash);
        let none = run(&inputs, &base(), &[], &[], 0..3, 6, |_| ()).unwrap();
        assert_eq!(none[0].state_hash, none[1].state_hash);
        assert_eq!(none[1].state_hash, none[2].state_hash);
    }

    fn edit(tick: u32, kind: EditKind, target: &str, value: f64) -> Edit {
        Edit {
            tick,
            source: EditSource::User,
            god: false,
            kind,
            target: target.into(),
            value,
            query: None,
        }
    }

    #[test]
    fn pair_shares_draws_and_null_branch_equals_baseline() {
        let inputs = Arc::new(small_world());
        let pair = run_pair(
            &inputs,
            &base(),
            &priors(),
            (&[], &[]),
            &[0, 1, 2],
            6,
            |w| w.world_price_index(),
        )
        .unwrap();
        for (a, b) in pair.baseline.iter().zip(&pair.branch) {
            assert_eq!(a.edits, b.edits);
            assert_eq!(a.state_hash, b.state_hash);
            assert_eq!(a.outcome, b.outcome);
        }
    }

    #[test]
    fn a_param_edit_moves_every_member_off_its_baseline() {
        let inputs = Arc::new(small_world());
        let log = [edit(3, EditKind::Param, "econ.kappa_w", 0.4)];
        let pair = run_pair(&inputs, &base(), &priors(), (&[], &log), &[0, 1], 8, |_| ()).unwrap();
        for (a, b) in pair.baseline.iter().zip(&pair.branch) {
            assert_ne!(a.state_hash, b.state_hash, "member {}", a.seed);
        }
    }
}
