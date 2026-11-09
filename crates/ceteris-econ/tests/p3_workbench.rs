//! Golden scenario for P3 (specs/p3/PLAN.md J1): the fixture world with a fork, a tariff edit,
//! an edit by query and a 4-member ensemble. Lines: branch member tick state-hash, for the
//! ensemble members of each branch at the horizon. Any change to these hashes must say why in
//! its commit; regenerate deliberately with
//! `CETERIS_BLESS=1 cargo test -p ceteris-econ --release --test p3_workbench`.

use std::sync::Arc;

use ceteris_core::snapshot::hex;
use ceteris_econ::EconParams;
use ceteris_econ::ensemble::run;
use ceteris_econ::fixture::small_world;
use ceteris_run::ensemble::register_priors;
use ceteris_run::{Edit, EditKind, EditSource, Scenario};
use ceteris_trade::TradeParams;

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/golden/p3-workbench.txt"
);
const HEADER: &str = "# P3 workbench fixture: fork at 6, tariff at 7, wage edit by query at 8, 4 members, 24 ticks. Lines: branch member tick state-hash";
const TICKS: u32 = 24;

fn edit(tick: u32, kind: EditKind, target: &str, value: f64, query: Option<&str>) -> Edit {
    Edit {
        tick,
        source: EditSource::User,
        god: false,
        kind,
        target: target.into(),
        value,
        query: query.map(str::to_string),
    }
}

fn scenario() -> Scenario {
    let mut s = Scenario::parse(
        "pack_hash = \"fixture\"\nengine = \"test+1\"\nseeds = [0, 1, 2, 3]\n\n[[branch]]\nid = \"main\"\n",
    )
    .unwrap();
    s = ceteris_run::commands::fork(&s, "main", 6, "tariff").unwrap();
    let tariff = s.branches.iter_mut().find(|b| b.id == "tariff").unwrap();
    tariff
        .edits
        .push(edit(7, EditKind::Policy, "tariff.0.1.0", 0.25, None));
    tariff
        .edits
        .push(edit(8, EditKind::State, "wage", 1.1, Some("country == 0")));
    s.validate().unwrap();
    s
}

#[test]
fn p3_workbench_fixture_matches_golden_hashes() {
    let s = scenario();
    let inputs = Arc::new(small_world());
    let base = (EconParams::default(), TradeParams::default());
    let priors = register_priors();
    let mut actual = String::from(HEADER);
    for branch in ["main", "tariff"] {
        let log = s.edits_of(branch).unwrap();
        let members = run(
            &inputs,
            &base,
            &priors,
            &log,
            s.seeds.iter().copied(),
            TICKS,
            |_| (),
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
        for (k, m) in members.iter().enumerate() {
            actual.push_str(&format!("\n{branch} {k} {TICKS} {}", hex(&m.state_hash)));
        }
    }
    actual.push('\n');

    if std::env::var_os("CETERIS_BLESS").is_some() {
        std::fs::write(GOLDEN, &actual).unwrap();
        return;
    }
    let expected =
        std::fs::read_to_string(GOLDEN).expect("golden file missing; run with CETERIS_BLESS=1");
    let first_diff = expected.lines().zip(actual.lines()).find(|(e, a)| e != a);
    assert!(
        first_diff.is_none() && expected == actual,
        "golden mismatch, first differing line: {first_diff:?}"
    );
}

#[test]
fn the_golden_run_hashes_do_not_depend_on_the_thread_count() {
    let s = scenario();
    let inputs = Arc::new(small_world());
    let base = (EconParams::default(), TradeParams::default());
    let priors = register_priors();
    let log = s.edits_of("tariff").unwrap();
    let go = |threads: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| {
                run(&inputs, &base, &priors, &log, [2u64], TICKS, |_| ())
                    .unwrap()
                    .remove(0)
                    .state_hash
            })
    };
    assert_eq!(go(1), go(4));
}
