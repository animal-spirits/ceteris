//! Golden scenario for P2: the fixture world from the data state with default parameters.
//! Any change to these hashes must say why in its commit; regenerate deliberately with
//! `CETERIS_BLESS=1 cargo test -p ceteris-econ --release --test golden`.

use std::sync::Arc;

use ceteris_core::snapshot::hex;
use ceteris_econ::EconParams;
use ceteris_econ::EconWorld;
use ceteris_econ::fixture::small_world;
use ceteris_trade::TradeParams;

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/golden/p2-econ.txt"
);
const HEADER: &str =
    "# P2 fixture: 4 countries, 25 provinces, seed 42, 1200 ticks. Lines: tick state-hash";

#[test]
fn p2_fixture_matches_golden_hashes() {
    let mut sim = EconWorld::sim(
        Arc::new(small_world()),
        EconParams::default(),
        TradeParams::default(),
    );
    sim.run_until(1200).unwrap_or_else(|v| panic!("{v}"));
    let mut actual = String::from(HEADER);
    for (tick, hash) in sim.snapshot_hashes() {
        actual.push_str(&format!("\n{tick} {}", hex(&hash)));
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
