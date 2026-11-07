//! `ceteris new | fork | edit | diff` write and read scenario files; the run hash is stable.

use std::process::Command;

fn ceteris(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(args)
        .output()
        .expect("runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn new_fork_edit_diff_write_the_expected_scenario() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("s.toml");
    let f = file.to_str().unwrap();
    for args in [
        vec![
            "new",
            f,
            "--pack-hash",
            "abc",
            "--engine",
            "test+1",
            "--seeds",
            "1,2",
        ],
        vec!["fork", f, "--from", "main", "--at", "12", "--id", "tariff"],
        vec![
            "edit",
            f,
            "--branch",
            "tariff",
            "--at",
            "13",
            "--kind",
            "policy",
            "--target",
            "tariff.0.1.0",
            "--value",
            "0.25",
        ],
    ] {
        assert!(ceteris(&args).0, "{args:?}");
    }
    let text = std::fs::read_to_string(&file).unwrap();
    let s = ceteris_run::Scenario::parse(&text).unwrap();
    assert_eq!(s.seeds, [1, 2]);
    assert_eq!(s.branch("tariff").unwrap().edits.len(), 1);
    assert_eq!(text, s.to_toml(), "files are canonical");
    let (ok, out) = ceteris(&["diff", f, "main", "tariff"]);
    assert!(ok);
    assert_eq!(out.trim(), "+ 13 Policy tariff.0.1.0 = 0.25");
    let share = |branch, seed| ceteris(&["share", f, "--branch", branch, "--seed", seed]);
    let (ok, hash) = share("tariff", "1");
    assert!(ok);
    assert_eq!(hash.trim().len(), 64);
    assert_ne!(hash, share("main", "1").1);
    assert_ne!(hash, share("tariff", "2").1);
    assert_eq!(hash, share("tariff", "1").1);
    assert!(!share("nope", "1").0);
    assert!(!ceteris(&["fork", f, "--from", "nope", "--at", "1", "--id", "x"]).0);
}
