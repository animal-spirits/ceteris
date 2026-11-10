//! Applies the edit log (Section 14.2) to an [`EconWorld`] at a tick boundary.
//!
//! Targets at P2: scalar economic and trade parameters, border icebergs per good, tariffs, non-tariff barriers, wages per province, and the
//! frictionless switch. Event injection, decision override and replay entries have no targets
//! until later phases and are rejected by name.

use std::fmt;

use ceteris_run::{Edit, EditKind, Query, QueryError};

use ceteris_trade::{TradeParams, Wedges};

use crate::inputs::EconInputs;
use crate::params::EconParams;
use crate::world::EconWorld;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    NoTarget(EditKind),
    UnknownTarget(String),
    OutOfRange(String),
    SwitchAfterStart { tick: u32 },
    WrongTick { edit: u32, tick: u32 },
    Query(QueryError),
    QueryNeedsState,
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
            Self::Query(e) => e.fmt(f),
            Self::QueryNeedsState => write!(f, "edit: only state edits select by query"),
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
    if let Some(query) = &e.query {
        return match e.kind {
            EditKind::State => set_by_query(w, query, target, e.value),
            _ => Err(EditError::QueryNeedsState),
        };
    }
    match e.kind {
        EditKind::Param => set_param(w, target, e.value),
        EditKind::Policy if target.starts_with("ntb.") => set_ntb(w, target, e.value),
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
    let mut p = w.params().clone();
    let mut t = w.trade().clone();
    set_param_in(&mut p, &mut t, target, v)?;
    if p != *w.params() {
        w.set_params(p);
    }
    if t != *w.trade() {
        w.set_trade_params(t);
    }
    Ok(())
}

fn set_param_in(
    p: &mut EconParams,
    t: &mut TradeParams,
    target: &str,
    v: f64,
) -> Result<(), EditError> {
    let unknown = || EditError::UnknownTarget(target.to_string());
    match target.split_once('.').ok_or_else(unknown)? {
        ("econ", name) => {
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
        }
        ("trade", name) if name.starts_with("border_nu.") => {
            let [g] = indices::<1>(name, "border_nu")?;
            let nu = t.border_nu.get_mut(g).ok_or_else(unknown)?;
            if !(v.is_finite() && v >= 0.0) {
                return Err(EditError::OutOfRange(target.to_string()));
            }
            *nu = v;
        }
        ("trade", name) => {
            *match name {
                "theta_scale" => &mut t.theta_scale,
                "kappa1" => &mut t.kappa1,
                "gain_share" => &mut t.gain_share,
                _ => return Err(unknown()),
            } = v;
        }
        _ => return Err(unknown()),
    }
    Ok(())
}

/// The parameters of one ensemble member: `base` with the tick-0 `Param` edits applied. Other
/// edit kinds are not parameters of the member and are rejected.
pub fn member_params(
    base: &(EconParams, TradeParams),
    edits: &[Edit],
) -> Result<(EconParams, TradeParams), EditError> {
    let (mut p, mut t) = base.clone();
    for e in edits {
        match e.kind {
            EditKind::Param => set_param_in(&mut p, &mut t, &e.target, e.value)?,
            kind => return Err(EditError::NoTarget(kind)),
        }
    }
    Ok((p, t))
}

/// `tariff.<importer>.<exporter>.<group>`, country indices and the tariff group.
fn set_tariff(w: &mut EconWorld, target: &str, v: f64) -> Result<(), EditError> {
    let mut wedges = w.inputs().wedges.clone();
    set_tariff_in(&mut wedges, target, v)?;
    w.set_wedges(wedges);
    Ok(())
}

/// `ntb.<importer>.<exporter>.<group>`, an iceberg factor of at least 1 (ADR 0337).
fn set_ntb(w: &mut EconWorld, target: &str, v: f64) -> Result<(), EditError> {
    let mut wedges = w.inputs().wedges.clone();
    set_ntb_in(&mut wedges, target, v)?;
    w.set_wedges(wedges);
    Ok(())
}

fn set_ntb_in(wedges: &mut Wedges, target: &str, v: f64) -> Result<(), EditError> {
    let idx = indices::<3>(target, "ntb")?;
    let n = wedges.n_countries();
    if idx[0] >= n || idx[1] >= n || idx[2] >= 2 || !(v.is_finite() && v >= 1.0) {
        return Err(EditError::OutOfRange(target.to_string()));
    }
    if wedges.ntb.is_empty() {
        wedges.ntb = vec![1.0; wedges.tariff.len()];
    }
    wedges.ntb[(idx[0] * n + idx[1]) * 2 + idx[2]] = v as f32;
    Ok(())
}

fn set_tariff_in(wedges: &mut Wedges, target: &str, v: f64) -> Result<(), EditError> {
    let idx = indices::<3>(target, "tariff")?;
    let n = wedges.n_countries();
    if idx[0] >= n || idx[1] >= n || idx[2] >= 2 {
        return Err(EditError::OutOfRange(target.to_string()));
    }
    wedges.tariff[(idx[0] * n + idx[1]) * 2 + idx[2]] = v as f32;
    Ok(())
}

/// The inputs with every tariff edit of `log` in force from the start, whatever its tick: the
/// network the long-run counterfactual equilibrium is solved on (14.7). Other edits are not
/// inputs and are left out.
pub fn tariff_inputs(inputs: &EconInputs, log: &[Edit]) -> Result<EconInputs, EditError> {
    let mut out = inputs.clone();
    for e in log.iter().filter(|e| e.kind == EditKind::Policy) {
        if e.target.starts_with("tariff.") {
            set_tariff_in(&mut out.wedges, &e.target, e.value)?;
        } else if e.target.starts_with("ntb.") {
            set_ntb_in(&mut out.wedges, &e.target, e.value)?;
        }
    }
    Ok(out)
}

/// The province columns a query can name: the world's own tables at the edit's tick.
const PROVINCE_COLUMNS: [&str; 4] = ["country", "population", "w0", "wage"];

/// Sets the `wage` column of every province the query matches, evaluated now. Edit by query
/// (ADR 0155): the stored text is re-evaluated on replay at the edit's tick, so the edit list
/// stays the only causal record.
fn set_by_query(w: &mut EconWorld, query: &str, column: &str, v: f64) -> Result<(), EditError> {
    if column != "wage" {
        return Err(EditError::UnknownTarget(column.to_string()));
    }
    let q = Query::parse(query, &PROVINCE_COLUMNS).map_err(EditError::Query)?;
    let hits: Vec<usize> = {
        let provinces = &w.inputs().provinces;
        (0..w.wage.len())
            .filter(|&p| {
                q.matches(&|c| match c {
                    0 => f64::from(provinces.country[p]),
                    1 => provinces.population[p],
                    2 => provinces.w0[p],
                    _ => w.wage[p],
                })
            })
            .collect()
    };
    for p in hits {
        w.wage[p] = v;
    }
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
            query: None,
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
            start_year: 2000,
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

    const TAPE: &str = "date,family,target,value\n\
        2000-03,policy,tariff.0.1.0,0.1\n\
        2000-07,policy,tariff.0.1.0,0.3\n";

    fn replayed(start_year: u32, sel: ReplaySel) -> Scenario {
        let mut scenario = Scenario {
            pack_hash: "fixture".into(),
            engine: "test+1".into(),
            start_year,
            replay: sel,
            seeds: vec![1],
            params: Default::default(),
            branches: vec![Branch {
                id: "base".into(),
                parent: None,
                fork_tick: None,
                edits: vec![],
            }],
        };
        let edits = ceteris_run::replay_edits(sel, 2000, || Ok(TAPE.to_string())).unwrap();
        scenario.add_replay(&edits);
        scenario
    }

    /// The state hash and the base tariff 0 to 1 of the base branch at `tick`.
    fn state_hash_at(scenario: Scenario, tick: u32) -> (ceteris_core::snapshot::Hash, f32) {
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
        let mut sim = wb.at("base", tick).unwrap();
        let rate = sim.world_mut().inputs().wedges.rate(0, 1, 0);
        (sim.state_hash(), rate)
    }

    #[test]
    fn replay_all_reproduces_tape_lever_paths() {
        let rate = |tick| state_hash_at(replayed(2000, ReplaySel::All), tick).1;
        assert_eq!([rate(2), rate(3), rate(6), rate(7)], [0.04, 0.1, 0.1, 0.3]);
    }

    #[test]
    fn replay_edits_are_logged_like_user_edits() {
        let on = replayed(2000, ReplaySel::Policy);
        let log = &on.branch("base").unwrap().edits;
        assert_eq!(log.len(), 2);
        assert!(
            log.iter()
                .all(|e| e.source == EditSource::Replay && e.kind == EditKind::Policy)
        );
        let off = replayed(2000, ReplaySel::Off);
        assert_ne!(
            on.run_hash("base", 1).unwrap(),
            off.run_hash("base", 1).unwrap()
        );
        assert_ne!(state_hash_at(on, 12).0, state_hash_at(off, 12).0);
    }

    #[test]
    fn year_relabel_2000_to_2100_keeps_golden_hash() {
        let (a, b) = (
            replayed(2000, ReplaySel::Off),
            replayed(2100, ReplaySel::Off),
        );
        assert_eq!(
            a.run_hash("base", 1).unwrap(),
            b.run_hash("base", 1).unwrap()
        );
        assert_eq!(state_hash_at(a, 24).0, state_hash_at(b, 24).0);
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

    #[test]
    fn ntb_edit_sets_the_wedge_survives_snapshot_and_rejects_bad_values() {
        use ceteris_core::snapshot::{canonical, decode_canonical_with};

        let ntb = |target: &str, v: f64| edit(0, EditSource::User, EditKind::Policy, target, v);
        let mut edited = world();
        assert!(edited.inputs().wedges.ntb.is_empty());
        let before = canonical(0, &edited);
        apply_edit(&mut edited, &ntb("ntb.0.1.1", 1.25)).unwrap();
        assert_eq!(edited.inputs().wedges.ntb_factor(0, 1, 1), 1.25);
        assert_eq!(edited.inputs().wedges.ntb_factor(1, 0, 1), 1.0);
        let bytes = canonical(0, &edited);
        assert_ne!(bytes, before, "the state hash sees the NTB");
        let fresh = world();
        let (_, restored) =
            decode_canonical_with(&bytes, |d| ceteris_core::sim::State::restore(&fresh, d))
                .unwrap();
        assert_eq!(restored.inputs().wedges.ntb_factor(0, 1, 1), 1.25);
        assert_eq!(restored.digest(), edited.digest());
        for bad in [("ntb.0.1.1", 0.9), ("ntb.0.1.2", 1.1), ("ntb.99.0.0", 1.1)] {
            let r = apply_edit(&mut world(), &ntb(bad.0, bad.1));
            assert!(matches!(r, Err(EditError::OutOfRange(_))), "{bad:?}");
        }
        let log = [ntb("ntb.0.1.0", 1.5)];
        let inputs = tariff_inputs(world().inputs(), &log).unwrap();
        assert_eq!(inputs.wedges.ntb_factor(0, 1, 0), 1.5);
    }

    #[test]
    fn border_nu_edit_sets_one_good() {
        let mut w = world();
        let nu0 = w.trade().border_nu.clone();
        let e = edit(
            0,
            EditSource::User,
            EditKind::Param,
            "trade.border_nu.2",
            3.0,
        );
        apply_edit(&mut w, &e).unwrap();
        assert_eq!(w.trade().border_nu[2], 3.0);
        for g in (0..nu0.len()).filter(|&g| g != 2) {
            assert_eq!(w.trade().border_nu[g], nu0[g]);
        }
        for bad in [("trade.border_nu.99", 1.0), ("trade.border_nu.2", -1.0)] {
            let e = edit(0, EditSource::User, EditKind::Param, bad.0, bad.1);
            assert!(apply_edit(&mut world(), &e).is_err(), "{bad:?}");
        }
    }

    fn query_edit(tick: u32, query: &str, value: f64) -> Edit {
        Edit {
            query: Some(query.to_string()),
            ..edit(tick, EditSource::User, EditKind::State, "wage", value)
        }
    }

    #[test]
    fn edit_by_query_sets_the_matching_provinces_only() {
        let mut w = world();
        let before = w.wage.clone();
        let cut = w.inputs().provinces.population[0];
        let q = format!("population <= {cut} and country == 0");
        apply_edit(&mut w, &query_edit(0, &q, 9.0)).unwrap();
        let p = &w.inputs().provinces;
        for (i, (&now, &was)) in w.wage.iter().zip(&before).enumerate() {
            let hit = p.population[i] <= cut && p.country[i] == 0;
            assert_eq!(now, if hit { 9.0 } else { was }, "province {i}");
        }
        assert!(w.wage[0] == 9.0);
    }

    #[test]
    fn query_names_unknown_column_error() {
        let mut w = world();
        assert!(matches!(
            apply_edit(&mut w, &query_edit(0, "famine > 0", 1.0)),
            Err(EditError::Query(QueryError::UnknownColumn(_)))
        ));
        let mut policy = query_edit(0, "wage > 0", 1.0);
        policy.kind = EditKind::Policy;
        assert_eq!(apply_edit(&mut w, &policy), Err(EditError::QueryNeedsState));
    }

    #[test]
    fn edit_by_query_replays_to_same_cells_on_replay() {
        // The query reads the live wage column, which the run moves before the edit's tick, so
        // a fork's replay from a snapshot must select the same provinces as a continuous run.
        let branch = |id: &str, parent: Option<(&str, u32)>, edits| Branch {
            id: id.to_string(),
            parent: parent.map(|(p, _)| p.to_string()),
            fork_tick: parent.map(|(_, t)| t),
            edits,
        };
        let q = {
            let w = world();
            let mut wages = w.wage.clone();
            wages.sort_by(f64::total_cmp);
            format!("wage > {}", wages[wages.len() / 2])
        };
        let scenario = Scenario {
            pack_hash: "fixture".into(),
            engine: "test+1".into(),
            start_year: 2000,
            replay: ReplaySel::Off,
            seeds: vec![1],
            params: Default::default(),
            branches: vec![
                branch("base", None, vec![query_edit(14, &q, 0.5)]),
                branch("flat", None, vec![query_edit(14, &q, 0.5)]),
                branch("fork", Some(("base", 12)), vec![query_edit(14, &q, 0.5)]),
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
        wb.at("base", 18).unwrap();
        let fork = wb.at("fork", 18).unwrap();
        assert_eq!(fork.state_hash(), wb.at("flat", 18).unwrap().state_hash());
    }
}
