//! C7 guard (RFC 0314): no held-out source is read by the pack build or by calibration code
//! outside its `read_by` scorer, and every scored H source is pre-registered.

use ceteris_pack::sources;

#[test]
fn holdout_guard_clean() {
    let problems = sources::problems(&sources::workspace_root(), None).unwrap();
    assert!(problems.is_empty(), "{problems:?}");
}
