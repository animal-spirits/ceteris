//! Golden scenario for P1 (Section 21.2): least costs and hours from the probe provinces
//! over the 2000 pack. Any change must say why in its commit; regenerate deliberately with
//! `CETERIS_BLESS=1 cargo test -p ceteris-geo --test golden`. Skipped without a pack.

mod common;

use ceteris_geo::paths::shortest;
use common::{PROBES, pack, province};

const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/p1-geo.txt");
const HEADER: &str = "# P1 geography: 2000 pack, default transport parameters. Lines: source blake3(cost and hours bits, all nodes)";

#[test]
fn p1_costs_match_golden_hashes() {
    let Some(pack) = pack() else { return };
    let mut actual = String::from(HEADER);
    for id in PROBES {
        let t = shortest(&pack.graph, province(&pack, id));
        let mut h = blake3::Hasher::new();
        for (c, hr) in t.cost.iter().zip(&t.hours) {
            h.update(&c.to_bits().to_le_bytes());
            h.update(&hr.to_bits().to_le_bytes());
        }
        actual.push_str(&format!("\n{id} {}", h.finalize().to_hex()));
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
