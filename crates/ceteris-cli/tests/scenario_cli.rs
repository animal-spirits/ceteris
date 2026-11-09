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

#[test]
fn rebase_and_cherry_pick_rewrite_the_scenario_and_report_skips() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("s.toml");
    let f = file.to_str().unwrap();
    for args in [
        vec!["new", f, "--pack-hash", "abc", "--engine", "t+1"],
        vec!["fork", f, "--from", "main", "--at", "12", "--id", "a"],
        vec!["fork", f, "--from", "main", "--at", "12", "--id", "b"],
        vec![
            "edit", f, "--branch", "a", "--at", "13", "--kind", "policy", "--target", "x",
            "--value", "1",
        ],
        vec![
            "edit", f, "--branch", "a", "--at", "20", "--kind", "policy", "--target", "y",
            "--value", "2",
        ],
    ] {
        assert!(ceteris(&args).0, "{args:?}");
    }
    let (ok, out) = ceteris(&[
        "cherry-pick",
        f,
        "--from",
        "a",
        "--onto",
        "b",
        "--edits",
        "0,1",
    ]);
    assert!(ok);
    assert!(out.is_empty());
    let (ok, out) = ceteris(&["rebase", f, "--branch", "a", "--onto", "b", "--at", "15"]);
    assert!(ok);
    assert_eq!(out.trim(), "skipped BeforeFork: 13 Policy x = 1");
    let s = ceteris_run::Scenario::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(s.branch("a").unwrap().parent.as_deref(), Some("b"));
    assert_eq!(s.branch("a").unwrap().edits.len(), 1);
    assert_eq!(s.branch("b").unwrap().edits.len(), 2);
    assert!(!ceteris(&["rebase", f, "--branch", "a", "--onto", "nope", "--at", "1"]).0);
}

#[test]
fn explain_writes_channel_shares_for_every_outcome() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("s.toml");
    let f = file.to_str().unwrap();
    let out = dir.path().join("out");
    let o = out.to_str().unwrap();
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
        vec!["fork", f, "--from", "main", "--at", "3", "--id", "tariff"],
        vec![
            "edit",
            f,
            "--branch",
            "tariff",
            "--at",
            "4",
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
    let (ok, stdout) = ceteris(&[
        "explain",
        f,
        "--branch",
        "tariff",
        "--baseline",
        "main",
        "--ticks",
        "8",
        "--fixture",
        "--out",
        o,
    ]);
    assert!(ok);
    assert_eq!(stdout.lines().count(), 8, "{stdout}");
    assert!(stdout.contains("world_price_index prices:"));
    assert!(
        std::fs::metadata(out.join("channel_shares.arrow"))
            .unwrap()
            .len()
            > 0
    );
}

#[test]
fn blame_ranks_the_edits_of_a_branch() {
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
            "1",
        ],
        vec!["fork", f, "--from", "main", "--at", "2", "--id", "b"],
        vec![
            "edit",
            f,
            "--branch",
            "b",
            "--at",
            "2",
            "--kind",
            "param",
            "--target",
            "econ.kappa_w",
            "--value",
            "0.2501",
        ],
        vec![
            "edit",
            f,
            "--branch",
            "b",
            "--at",
            "3",
            "--kind",
            "policy",
            "--target",
            "tariff.0.1.0",
            "--value",
            "0.4",
        ],
    ] {
        assert!(ceteris(&args).0, "{args:?}");
    }
    let (ok, out) = ceteris(&[
        "blame",
        f,
        "--branch",
        "b",
        "--baseline",
        "main",
        "--ticks",
        "8",
        "--fixture",
    ]);
    assert!(ok);
    let first = out.lines().next().unwrap();
    assert!(first.starts_with("world_price_index"), "{out}");
    assert!(first.contains("edit 1:"), "{out}");
    assert_eq!(out.lines().count(), 4, "{out}");
}

#[test]
fn explain_reference_reports_the_long_run_gap() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("s.toml");
    let f = file.to_str().unwrap();
    let out = dir.path().join("out");
    let o = out.to_str().unwrap();
    for args in [
        vec![
            "new",
            f,
            "--pack-hash",
            "abc",
            "--engine",
            "test+1",
            "--seeds",
            "1",
        ],
        vec!["fork", f, "--from", "main", "--at", "3", "--id", "tariff"],
        vec![
            "edit",
            f,
            "--branch",
            "tariff",
            "--at",
            "4",
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
    let (ok, stdout) = ceteris(&[
        "explain",
        f,
        "--branch",
        "tariff",
        "--baseline",
        "main",
        "--ticks",
        "8",
        "--fixture",
        "--out",
        o,
        "--reference",
    ]);
    assert!(ok, "{stdout}");
    let refs: Vec<&str> = stdout
        .lines()
        .filter(|l| l.starts_with("reference "))
        .collect();
    assert_eq!(refs.len(), 2, "{stdout}");
    assert!(refs[0].contains("world_price_index") && refs[0].contains("gap"));
}

#[test]
fn fetch_recomputes_state_hashes() {
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
        vec!["fork", f, "--from", "main", "--at", "3", "--id", "tariff"],
        vec![
            "edit",
            f,
            "--branch",
            "tariff",
            "--at",
            "4",
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
    let hash = |seed| ceteris(&["share", f, "--branch", "tariff", "--seed", seed]).1;
    let fetch = |h: &str| {
        ceteris(&[
            "fetch",
            h.trim(),
            "--scenario",
            f,
            "--ticks",
            "6",
            "--fixture",
        ])
    };
    let (ok, a) = fetch(&hash("1"));
    assert!(ok);
    assert!(a.starts_with("tariff seed 1 tick 6 state "), "{a}");
    assert_eq!(a, fetch(&hash("1")).1, "recomputation is deterministic");
    assert_ne!(a, fetch(&hash("2")).1, "members differ");
    assert!(!fetch("00").0, "unknown hash is an error");
}

#[test]
fn api_schema_lists_every_table() {
    let (ok, out) = ceteris(&["api", "schema"]);
    assert!(ok);
    for t in ["outcomes(", "effects(", "channel_shares("] {
        assert!(out.contains(t), "{out}");
    }
}
