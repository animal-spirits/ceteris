//! Ensemble runner (`specs/workbench/SPEC.md`, Ensembles and common random numbers): member k
//! draws its dynamic parameters from the register priors, and runs as a lone world to the
//! horizon, one member after another, each with the full thread pool.

use std::sync::Arc;

use ceteris_core::invariant::Violation;
use ceteris_core::rng::Seed;
use ceteris_core::sim::Sim;
use ceteris_core::snapshot::Hash;
use ceteris_run::Edit;
use ceteris_run::attribution::shapley;
use ceteris_run::ensemble::{ParamPrior, member_edits};
use ceteris_trade::TradeParams;

use crate::channels::Channel;
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
    let sim = run_world(inputs, base, &edits, log, ticks, |_| ())?;
    Ok(Member {
        seed,
        edits,
        state_hash: sim.state_hash(),
        outcome: outcome(sim.world()),
    })
}

/// A world with the member's `draws` applied, `setup` run on it at tick 0, and `log` applied at
/// its ticks, stepped to `ticks`.
fn run_world(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    draws: &[Edit],
    log: &[Edit],
    ticks: u32,
    setup: impl FnOnce(&mut EconWorld),
) -> Result<Sim<EconWorld>, MemberError> {
    let (p, t) = member_params(base, draws).map_err(MemberError::Edit)?;
    let mut sim = EconWorld::sim(Arc::clone(inputs), p, t);
    setup(sim.world_mut());
    while sim.tick() < ticks {
        let t = sim.tick();
        apply_due(sim.world_mut(), log, t).map_err(MemberError::Edit)?;
        sim.step().map_err(MemberError::Invariant)?;
    }
    Ok(sim)
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

/// The effect of one branch on one outcome for one member, split over the channels by Shapley
/// value (14.7, ADR 0049): `v(S)` is the outcome with only the channels in `S` open (the rest
/// held at the baseline's paths) less the baseline's, so the shares add up to
/// `total - direct`, `direct` being what the branch does with every channel shut.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelShares {
    pub total: f64,
    pub direct: f64,
    pub shares: Vec<(Channel, f64)>,
}

pub const CHANNELS: [Channel; 3] = [Channel::Prices, Channel::Wages, Channel::TradeShares];

pub fn channel_shares(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    priors: &[ParamPrior],
    (baseline_log, branch_log): (&[Edit], &[Edit]),
    seed: u64,
    ticks: u32,
    outcomes: impl Fn(&EconWorld) -> Vec<f64>,
) -> Result<Vec<ChannelShares>, MemberError> {
    let draws = member_edits(Seed(seed), priors);
    let mut baseline = run_world(inputs, base, &draws, baseline_log, ticks, |w| {
        w.scratch.channels.record_from(0);
    })?;
    let reference = outcomes(baseline.world());
    let path = Arc::new(
        baseline
            .world_mut()
            .scratch
            .channels
            .take_record()
            .expect("recording was set up"),
    );
    let values = (0..1u32 << CHANNELS.len())
        .map(|open| {
            let shut: Vec<Channel> = CHANNELS
                .iter()
                .enumerate()
                .filter(|(i, _)| open >> i & 1 == 0)
                .map(|(_, &c)| c)
                .collect();
            let sim = run_world(inputs, base, &draws, branch_log, ticks, |w| {
                w.scratch.channels.shut(Arc::clone(&path), &shut);
            })?;
            let v = outcomes(sim.world());
            Ok(v.iter().zip(&reference).map(|(a, b)| a - b).collect())
        })
        .collect::<Result<Vec<Vec<f64>>, MemberError>>()?;
    Ok((0..reference.len())
        .map(|o| {
            let shapley = shapley(CHANNELS.len(), |s| values[s as usize][o], Seed(seed), 0);
            ChannelShares {
                total: values[values.len() - 1][o],
                direct: values[0][o],
                shares: CHANNELS.iter().copied().zip(shapley).collect(),
            }
        })
        .collect())
}

/// Leave-one-out effect of each edit of `log` for member `seed`: the outcomes with the whole
/// log less the outcomes with that edit removed, one row per edit in log order. The input of
/// `blame` (14.7, ADR 0116).
pub fn edit_effects(
    inputs: &Arc<EconInputs>,
    base: &(EconParams, TradeParams),
    priors: &[ParamPrior],
    log: &[Edit],
    seed: u64,
    ticks: u32,
    outcomes: impl Fn(&EconWorld) -> Vec<f64>,
) -> Result<Vec<Vec<f64>>, MemberError> {
    let draws = member_edits(Seed(seed), priors);
    let run = |log: &[Edit]| -> Result<Vec<f64>, MemberError> {
        let sim = run_world(inputs, base, &draws, log, ticks, |_| ())?;
        Ok(outcomes(sim.world()))
    };
    let full = run(log)?;
    (0..log.len())
        .map(|i| {
            let without: Vec<Edit> = log
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, e)| e.clone())
                .collect();
            let v = run(&without)?;
            Ok(full.iter().zip(&v).map(|(a, b)| a - b).collect())
        })
        .collect()
}

/// The declared outcomes read from a final world, by name.
pub fn outcomes(w: &EconWorld) -> Vec<(&'static str, f64)> {
    let n = w.wage.len() as f64;
    let values = [w.world_price_index(), w.wage.iter().sum::<f64>() / n];
    OUTCOME_NAMES.into_iter().zip(values).collect()
}

pub const OUTCOME_NAMES: [&str; 2] = ["world_price_index", "mean_wage"];

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

    #[test]
    fn crn_intervals_tighter_than_independent_on_fixture() {
        use ceteris_run::effects::effect;

        let inputs = Arc::new(small_world());
        let log = [edit(3, EditKind::Policy, "tariff.0.1.0", 0.25)];
        let k = 8;
        let seeds: Vec<u64> = (0..k).collect();
        let run_all = |log: &[Edit], seeds: &[u64]| {
            run(
                &inputs,
                &base(),
                &priors(),
                log,
                seeds.iter().copied(),
                10,
                outcomes,
            )
            .unwrap()
        };
        let baseline = run_all(&[], &seeds);
        let crn = run_all(&log, &seeds);
        let indep = run_all(&log, &(k..2 * k).collect::<Vec<_>>());
        let col = |m: &[Member<Vec<(&'static str, f64)>>], i: usize| -> Vec<f64> {
            m.iter().map(|m| m.outcome[i].1).collect()
        };
        for i in 0..2 {
            let width = |m| {
                let e = effect(&col(m, i), &col(&baseline, i), Seed(7)).interval;
                e.hi - e.lo
            };
            let (c, d) = (width(&crn), width(&indep));
            assert!(
                c < d,
                "outcome {i}: crn {c} not tighter than independent {d}"
            );
        }
    }

    fn tariff_log() -> [Edit; 1] {
        [edit(3, EditKind::Policy, "tariff.0.1.0", 0.25)]
    }

    #[test]
    fn recording_changes_no_hash() {
        let inputs = Arc::new(small_world());
        let plain = run_member(&inputs, &base(), &priors(), &[], 2, 8, |_| ()).unwrap();
        let (p, t) = member_params(&base(), &plain.edits).unwrap();
        let mut sim = EconWorld::sim(Arc::clone(&inputs), p, t);
        sim.world_mut().scratch.channels.record_from(0);
        sim.run_until(8).unwrap();
        assert_eq!(sim.state_hash(), plain.state_hash);
        let path = sim.world_mut().scratch.channels.take_record().unwrap();
        assert_eq!((path.wage.len(), path.price.len()), (8, 8));
    }

    #[test]
    fn shutting_every_channel_gives_zero_effect_on_prices_and_wages() {
        let inputs = Arc::new(small_world());
        let go = |log: &[Edit], shut: Option<Arc<crate::channels::Path>>| {
            let draws = member_edits(Seed(1), &priors());
            run_world(&inputs, &base(), &draws, log, 8, |w| match &shut {
                Some(p) => w.scratch.channels.shut(Arc::clone(p), &CHANNELS),
                None => w.scratch.channels.record_from(0),
            })
            .unwrap()
        };
        let mut baseline = go(&[], None);
        let path = Arc::new(baseline.world_mut().scratch.channels.take_record().unwrap());
        let open = go(&tariff_log(), None);
        let shut = go(&tariff_log(), Some(Arc::clone(&path)));
        assert_ne!(open.world().wage, baseline.world().wage);
        assert_eq!(shut.world().wage, baseline.world().wage);
        assert_eq!(shut.world().markets.price, baseline.world().markets.price);
    }

    #[test]
    fn null_branch_with_channels_shut_equals_baseline() {
        let inputs = Arc::new(small_world());
        let draws = member_edits(Seed(1), &priors());
        let mut baseline = run_world(&inputs, &base(), &draws, &[], 8, |w| {
            w.scratch.channels.record_from(0);
        })
        .unwrap();
        let path = Arc::new(baseline.world_mut().scratch.channels.take_record().unwrap());
        let replay = run_world(&inputs, &base(), &draws, &[], 8, |w| {
            w.scratch.channels.shut(Arc::clone(&path), &CHANNELS);
        })
        .unwrap();
        assert_eq!(replay.state_hash(), baseline.state_hash());
    }

    #[test]
    fn channel_shares_sum_to_total_less_direct() {
        let inputs = Arc::new(small_world());
        let s = channel_shares(
            &inputs,
            &base(),
            &priors(),
            (&[], &tariff_log()),
            1,
            10,
            |w| vec![w.world_price_index()],
        )
        .unwrap()
        .remove(0);
        let sum: f64 = s.shares.iter().map(|(_, v)| v).sum();
        assert!((sum - (s.total - s.direct)).abs() <= 1e-12 * s.total.abs().max(1.0));
        assert_ne!(s.total, 0.0);
    }

    #[test]
    fn blame_ranks_planted_edit_first() {
        use ceteris_run::attribution::blame;

        let inputs = Arc::new(small_world());
        let log = [
            edit(2, EditKind::Param, "econ.kappa_w", 0.2501),
            edit(3, EditKind::Policy, "tariff.0.1.0", 0.4),
        ];
        let rows = edit_effects(&inputs, &base(), &priors(), &log, 1, 10, |w| {
            vec![w.world_price_index()]
        })
        .unwrap();
        let effects: Vec<f64> = rows.iter().map(|r| r[0]).collect();
        assert_eq!(blame(&effects)[0], 1, "{effects:?}");
    }

    #[test]
    fn zero_rate_hazards_leave_hashes_unchanged() {
        use ceteris_run::commitments::{Commitment, Commitments};

        let inputs = Arc::new(small_world());
        let template = edit(0, EditKind::Policy, "tariff.0.1.0", 0.25);
        let log_of = |rate: f64| {
            let mut t = Commitments::default();
            let c = Commitment {
                id: 1,
                offset: 4,
                edit: template.clone(),
                rate,
            };
            t.add(c).unwrap();
            (0..10).flat_map(|k| t.due(Seed(3), k)).collect::<Vec<_>>()
        };
        let hash = |log: &[Edit]| {
            run_member(&inputs, &base(), &priors(), log, 2, 10, |_| ())
                .unwrap()
                .state_hash
        };
        let by_user = [edit(4, EditKind::Policy, "tariff.0.1.0", 0.25)];
        assert_eq!(hash(&log_of(0.0)), hash(&by_user));
        assert_eq!(
            hash(&log_of(1.0)),
            hash(&[]),
            "a cancelled commitment changes nothing"
        );
        assert_ne!(hash(&by_user), hash(&[]));
    }

    #[test]
    fn shut_trade_shares_hold_the_kernel_at_the_baseline() {
        let inputs = Arc::new(small_world());
        let draws = member_edits(Seed(1), &priors());
        let go = |shut: &[Channel]| {
            let path = Arc::new(crate::channels::Path::default());
            run_world(&inputs, &base(), &draws, &tariff_log(), 8, |w| {
                w.scratch.channels.shut(Arc::clone(&path), shut);
            })
            .unwrap()
        };
        let baseline = run_world(&inputs, &base(), &draws, &[], 8, |_| ()).unwrap();
        let held = go(&[Channel::TradeShares]);
        let open = go(&[]);
        assert_eq!(
            held.world().target.crossing_k(),
            baseline.world().target.crossing_k()
        );
        assert_ne!(
            open.world().target.crossing_k(),
            baseline.world().target.crossing_k()
        );
    }
}
