//! Applies the edit log (Section 14.2) to an [`EconWorld`] at a tick boundary.
//!
//! Targets at P2: scalar economic and trade parameters, tariffs, wages per province, and the
//! frictionless switch. Event injection, decision override and replay entries have no targets
//! until later phases and are rejected by name.

use std::fmt;

use ceteris_run::{Edit, EditKind};

use crate::world::EconWorld;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoTarget(EditKind),
    UnknownTarget(String),
    OutOfRange(String),
    SwitchAfterStart { tick: u32 },
    WrongTick { edit: u32, tick: u32 },
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoTarget(k) => write!(f, "edit: `{k:?}` edits have no targets yet"),
            Self::UnknownTarget(t) => write!(f, "edit: unknown target `{t}`"),
            Self::OutOfRange(t) => write!(f, "edit: target `{t}` is out of range"),
            Self::SwitchAfterStart { tick } => {
                write!(f, "edit: module switches are ex ante, got tick {tick}")
            }
            Self::WrongTick { edit, tick } => {
                write!(f, "edit: edit for tick {edit} applied at tick {tick}")
            }
        }
    }
}

impl std::error::Error for EditError {}

/// Applies the edits due at `tick` in (source, sequence) order, sequence being the position
/// in `edits`; edits for other ticks are skipped. Returns the number applied.
pub fn apply_due(w: &mut EconWorld, edits: &[Edit], tick: u32) -> Result<usize, EditError> {
    let due = ceteris_run::due(edits, tick);
    for e in &due {
        apply_edit(w, e)?;
    }
    Ok(due.len())
}

/// Applies one edit at its own tick boundary.
pub fn apply_edit(w: &mut EconWorld, e: &Edit) -> Result<(), EditError> {
    let target = e.target.as_str();
    match e.kind {
        EditKind::Param => set_param(w, target, e.value),
        EditKind::Policy => set_tariff(w, target, e.value),
        EditKind::State => set_state(w, target, e.value),
        EditKind::Switch if e.tick > 0 => Err(EditError::SwitchAfterStart { tick: e.tick }),
        EditKind::Switch => match target {
            "frictionless" => {
                let mut p = w.params().clone();
                p.frictionless = e.value != 0.0;
                w.set_params(p);
                Ok(())
            }
            _ => Err(EditError::UnknownTarget(e.target.clone())),
        },
        kind @ (EditKind::Event | EditKind::Override | EditKind::Replay) => {
            Err(EditError::NoTarget(kind))
        }
    }
}

fn set_param(w: &mut EconWorld, target: &str, v: f64) -> Result<(), EditError> {
    let unknown = || EditError::UnknownTarget(target.to_string());
    match target.split_once('.').ok_or_else(unknown)? {
        ("econ", name) => {
            let mut p = w.params().clone();
            *match name {
                "sigma_demand" => &mut p.sigma_demand,
                "sigma_fl" => &mut p.sigma_fl,
                "kappa_w" => &mut p.kappa_w,
                "delta_w" => &mut p.delta_w,
                "eta" => &mut p.eta,
                "write_off_months" => &mut p.write_off_months,
                "drift_half_life" => &mut p.drift_half_life,
                _ => return Err(unknown()),
            } = v;
            w.set_params(p);
        }
        ("trade", name) => {
            let mut t = w.trade().clone();
            *match name {
                "theta_scale" => &mut t.theta_scale,
                "kappa1" => &mut t.kappa1,
                "gain_share" => &mut t.gain_share,
                _ => return Err(unknown()),
            } = v;
            w.set_trade_params(t);
        }
        _ => return Err(unknown()),
    }
    Ok(())
}

/// `tariff.<importer>.<exporter>.<group>`, country indices and the tariff group.
fn set_tariff(w: &mut EconWorld, target: &str, v: f64) -> Result<(), EditError> {
    let idx = indices::<3>(target, "tariff")?;
    let mut wedges = w.inputs().wedges.clone();
    let n = wedges.n_countries();
    if idx[0] >= n || idx[1] >= n || idx[2] >= 2 {
        return Err(EditError::OutOfRange(target.to_string()));
    }
    wedges.tariff[(idx[0] * n + idx[1]) * 2 + idx[2]] = v as f32;
    w.set_wedges(wedges);
    Ok(())
}

/// `wage.<province>`, god mode.
fn set_state(w: &mut EconWorld, target: &str, v: f64) -> Result<(), EditError> {
    let [p] = indices::<1>(target, "wage")?;
    *w.wage
        .get_mut(p)
        .ok_or_else(|| EditError::OutOfRange(target.to_string()))? = v;
    Ok(())
}

fn indices<const N: usize>(target: &str, head: &str) -> Result<[usize; N], EditError> {
    let unknown = || EditError::UnknownTarget(target.to_string());
    let mut parts = target.split('.');
    if parts.next() != Some(head) {
        return Err(unknown());
    }
    let mut out = [0; N];
    for slot in &mut out {
        *slot = parts
            .next()
            .and_then(|s| s.parse().ok())
            .ok_or_else(unknown)?;
    }
    if parts.next().is_some() {
        return Err(unknown());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use ceteris_core::rng::Seed;
    use ceteris_run::store::ChunkStore;
    use ceteris_run::{Branch, EditSource, ReplaySel, Scenario, Workbench};
    use ceteris_trade::TradeParams;

    use super::*;
    use crate::fixture::small_world;
    use crate::params::EconParams;

    fn world() -> EconWorld {
        EconWorld::new(
            Arc::new(small_world()),
            EconParams::default(),
            TradeParams::default(),
        )
    }

    fn edit(tick: u32, source: EditSource, kind: EditKind, target: &str, value: f64) -> Edit {
        Edit {
            tick,
            source,
            god: kind == EditKind::State,
            kind,
            target: target.to_string(),
            value,
        }
    }

    #[test]
    fn edit_applies_at_its_tick() {
        let mut w = world();
        let edits = [
            edit(3, EditSource::User, EditKind::Param, "econ.kappa_w", 0.2),
            edit(3, EditSource::User, EditKind::Param, "trade.kappa1", 0.5),
            edit(3, EditSource::User, EditKind::Policy, "tariff.0.1.0", 0.25),
            edit(3, EditSource::User, EditKind::State, "wage.0", 7.0),
        ];
        assert_eq!(apply_due(&mut w, &edits, 2), Ok(0));
        assert_ne!(w.params().kappa_w, 0.2);
        let before = w.digest();
        assert_eq!(apply_due(&mut w, &edits, 3), Ok(4));
        assert_eq!(w.params().kappa_w, 0.2);
        assert_eq!(w.trade().kappa1, 0.5);
        assert_eq!(w.inputs().wedges.rate(0, 1, 0), 0.25);
        assert_eq!(w.wage[0], 7.0);
        assert_ne!(w.digest(), before);
    }

    #[test]
    fn edits_at_one_tick_apply_in_source_then_sequence_order() {
        let log = [
            edit(1, EditSource::Cli, EditKind::Param, "econ.eta", 0.1),
            edit(1, EditSource::User, EditKind::Param, "econ.eta", 0.2),
            edit(1, EditSource::Cli, EditKind::Param, "econ.eta", 0.3),
        ];
        let mut w = world();
        apply_due(&mut w, &log, 1).unwrap();
        // User first, then Cli in log order: the last Cli edit wins.
        assert_eq!(w.params().eta, 0.3);
        let mut reversed = log.clone();
        reversed.reverse();
        let mut w2 = world();
        apply_due(&mut w2, &reversed, 1).unwrap();
        assert_eq!(w2.params().eta, 0.1);
    }

    #[test]
    fn switch_edit_after_t0_is_rejected() {
        let mut w = world();
        let late = edit(1, EditSource::User, EditKind::Switch, "frictionless", 1.0);
        assert_eq!(
            apply_edit(&mut w, &late),
            Err(EditError::SwitchAfterStart { tick: 1 })
        );
        let early = edit(0, EditSource::User, EditKind::Switch, "frictionless", 1.0);
        apply_edit(&mut w, &early).unwrap();
        assert!(w.params().frictionless);
    }

    #[test]
    fn event_and_override_have_no_targets_yet() {
        let mut w = world();
        for kind in [EditKind::Event, EditKind::Override, EditKind::Replay] {
            let e = edit(0, EditSource::User, kind, "x", 0.0);
            assert_eq!(apply_edit(&mut w, &e), Err(EditError::NoTarget(kind)));
        }
    }

    #[test]
    fn unknown_and_out_of_range_targets_are_named() {
        let mut w = world();
        let bad = edit(0, EditSource::User, EditKind::Param, "econ.nope", 1.0);
        assert!(matches!(
            apply_edit(&mut w, &bad),
            Err(EditError::UnknownTarget(_))
        ));
        let far = edit(0, EditSource::User, EditKind::Policy, "tariff.99.0.0", 1.0);
        assert!(matches!(
            apply_edit(&mut w, &far),
            Err(EditError::OutOfRange(_))
        ));
    }

    fn apply(w: &mut EconWorld, e: &Edit) -> Result<(), String> {
        apply_edit(w, e).map_err(|e| e.to_string())
    }

    #[test]
    fn null_intervention_reproduces_parent_on_the_p2_fixture() {
        let branch = |id: &str, parent: Option<(&str, u32)>, edits| Branch {
            id: id.to_string(),
            parent: parent.map(|(p, _)| p.to_string()),
            fork_tick: parent.map(|(_, t)| t),
            edits,
        };
        let scenario = Scenario {
            pack_hash: "fixture".into(),
            engine: "test+1".into(),
            replay: ReplaySel::Off,
            seeds: vec![1],
            params: Default::default(),
            branches: vec![
                branch(
                    "base",
                    None,
                    vec![edit(
                        4,
                        EditSource::User,
                        EditKind::Policy,
                        "tariff.0.1.0",
                        0.2,
                    )],
                ),
                branch("null", Some(("base", 12)), vec![]),
                branch(
                    "slow_wages",
                    Some(("base", 12)),
                    vec![edit(
                        13,
                        EditSource::User,
                        EditKind::Param,
                        "econ.kappa_w",
                        0.01,
                    )],
                ),
            ],
        };
        let dir = tempfile::tempdir().unwrap();
        let store = ChunkStore::open(dir.path().join("cache"), "test+1").unwrap();
        let mut wb = Workbench::new(
            scenario,
            Seed(1),
            |_| {
                EconWorld::sim(
                    Arc::new(small_world()),
                    EconParams::default(),
                    TradeParams::default(),
                )
            },
            apply,
            store,
        );
        let parent = wb.at("base", 18).unwrap().state_hash();
        assert_eq!(wb.at("null", 18).unwrap().state_hash(), parent);
        assert_ne!(wb.at("slow_wages", 18).unwrap().state_hash(), parent);
    }

    #[test]
    fn tariff_edit_survives_snapshot_restore() {
        use ceteris_core::snapshot::{canonical, decode_canonical_with};

        let mut edited = world();
        let before = canonical(0, &edited);
        let e = edit(0, EditSource::User, EditKind::Policy, "tariff.0.1.0", 0.25);
        apply_edit(&mut edited, &e).unwrap();
        let bytes = canonical(0, &edited);
        assert_ne!(bytes, before, "the state hash sees tariffs");
        let fresh = world();
        let (_, restored) =
            decode_canonical_with(&bytes, |d| ceteris_core::sim::State::restore(&fresh, d))
                .unwrap();
        assert_eq!(restored.inputs().wedges.rate(0, 1, 0), 0.25);
        assert_eq!(restored.digest(), edited.digest());
    }
}
