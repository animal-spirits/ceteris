//! Golden scenario for P0 (Section 21.2). Any change to these hashes must say why in
//! its commit; regenerate deliberately with `CETERIS_BLESS=1 cargo test --test golden`.

use ceteris_core::rng::Seed;
use ceteris_core::snapshot::hex;
use ceteris_core::toy::Toy;

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/p0-toy.txt");
const HEADER: &str = "# P0 toy world: 10 provinces, seed 42, 1200 ticks. Lines: tick state-hash";

#[test]
fn p0_toy_matches_golden_hashes() {
    let mut sim = Toy::sim(10, Seed(42));
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
