//! W1 (specs/p3/PLAN.md Branch W): the app drives the branch tree only through registry lines,
//! and the hash it shows at a cursor is the hash a headless workbench gives for the same
//! scenario, branch and tick.

use std::sync::Arc;

use ceteris_app::econ::{Layout, View};
use ceteris_app::session::Session;
use ceteris_app::workbench::Host;
use ceteris_core::rng::Seed;
use ceteris_core::sim::Sim;
use ceteris_econ::edit::apply_edit;
use ceteris_econ::fixture::small_world;
use ceteris_econ::{EconParams, EconWorld};
use ceteris_run::commands::user_edit;
use ceteris_run::store::ChunkStore;
use ceteris_run::{Branch, EditKind, Scenario, Workbench};
use ceteris_trade::TradeParams;

fn scenario() -> Scenario {
    let mut s = ceteris_run::commands::new_scenario("fixture", "test+1", vec![1]);
    s.branches.push(Branch {
        id: "tariff".into(),
        parent: Some("main".into()),
        fork_tick: Some(12),
        edits: vec![
            user_edit(13, EditKind::Policy, "tariff.0.1.0", 0.2, None),
            user_edit(20, EditKind::State, "pop.0", 1.0, None),
        ],
    });
    s
}

fn bench(s: Scenario) -> Workbench<EconWorld> {
    let dir = tempfile::tempdir().unwrap().keep();
    let store = ChunkStore::open(dir.join("cache"), "test+1").unwrap();
    Workbench::new(
        s,
        Seed(1),
        |_| {
            EconWorld::sim(
                Arc::new(small_world()),
                EconParams::default(),
                TradeParams::default(),
            )
        },
        |w, e| apply_edit(w, e).map_err(|e| e.to_string()),
        store,
    )
}

fn view(sim: &Sim<EconWorld>) -> View {
    View::new(sim, &Arc::new(Layout::new(sim.world())))
}

fn headless(branch: &str, tick: u32) -> String {
    let hash = bench(scenario()).at(branch, tick).unwrap().state_hash();
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn registry_lines_give_the_headless_hashes_and_the_edit_log() {
    let host = Host::spawn(|| Ok(Session::new(scenario(), bench)), view, || {});
    let next = |line: Option<&str>| {
        line.inspect(|l| host.send(l));
        host.wait().expect("host alive")
    };

    let s = next(None);
    assert_eq!((s.branch.as_str(), s.cursor), ("main", 0));
    assert_eq!(s.hash, headless("main", 0));
    assert_eq!(
        s.rows
            .iter()
            .map(|r| (r.id.as_str(), r.depth))
            .collect::<Vec<_>>(),
        [("main", 0), ("tariff", 1)]
    );

    let s = next(Some("time.run-to 12"));
    assert_eq!(s.hash, headless("main", 12));
    assert_eq!(
        s.view.as_ref().map(|v| v.tick),
        Some(12),
        "the map view follows the cursor"
    );

    let s = next(Some("branch.switch tariff"));
    assert_eq!((s.branch.as_str(), s.cursor), ("tariff", 12));
    assert_eq!(
        s.hash,
        headless("main", 12),
        "the fork starts as its parent"
    );

    let s = next(Some("time.run-to 18"));
    assert_eq!(s.hash, headless("tariff", 18));
    assert_ne!(s.hash, headless("main", 18));
    assert_eq!(s.log.len(), 2);
    assert!(s.log[0].starts_with("t13 [cli] tariff.0.1.0 = 0.2"));
    assert!(s.log[1].contains("GOD"), "state edits carry the god flag");
    assert!(!s.log[0].contains("GOD"));

    let s = next(Some("time.seek 3"));
    assert_eq!(s.cursor, 12, "seek never goes before the fork tick");

    let s = next(Some("branch.fork again"));
    assert_eq!((s.branch.as_str(), s.cursor), ("again", 12));
    assert_eq!(s.rows.len(), 3);

    let s = next(Some("branch.fork again"));
    assert!(s.error.is_some(), "a duplicate id is refused");
    let s = next(Some("branch.explode"));
    assert!(s.error.is_some_and(|e| e.contains("not a branch command")));
}

#[test]
fn a_session_that_cannot_build_reports_it() {
    let host = Host::spawn(|| Err("the economy did not load".into()), view, || {});
    let s = host.wait().unwrap();
    assert_eq!(s.error.as_deref(), Some("the economy did not load"));
    assert!(s.rows.is_empty());
}
