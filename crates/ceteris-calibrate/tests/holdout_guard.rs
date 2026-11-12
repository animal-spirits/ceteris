//! C7 guard (RFC 0314): no held-out source is read by the pack build or by calibration code
//! outside its `read_by` scorer, and every scored H source is pre-registered.

use ceteris_pack::sources;

#[test]
fn holdout_guard_clean() {
    let problems = sources::problems(&sources::workspace_root(), None).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn iea_balances_never_calibrate() {
    let root = sources::workspace_root();
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap();
    let declared = read("crates/ceteris-pack/config/sources.toml");
    let entry = declared
        .split("[[source]]")
        .find(|s| s.contains("id = \"iea-balances\""))
        .unwrap();
    assert!(entry.contains("use = \"validation\"") && entry.contains("holdout = \"H\""));
    assert!(!read("crates/ceteris-pack/config/fetch.toml").contains("iea-balances"));
}
