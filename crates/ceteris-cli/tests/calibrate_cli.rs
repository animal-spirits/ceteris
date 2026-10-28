//! `ceteris calibrate`: the calibration of an export directory, as
//! `python -m ceteris.reference calibrate --inputs` writes and prints it.

use std::path::{Path, PathBuf};
use std::process::Command;

use ceteris_pack::compare::{self, Tolerance};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture")
}

fn ceteris(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(args)
        .output()
        .unwrap()
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// One line per pass, the summary as indented JSON, then where the tables went; the tables are
/// the committed fixture calibration within the tolerance of an iterative solve.
#[test]
fn calibrate_writes_and_reports_the_fixture_calibration() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("calibration");
    let run = ceteris(&[
        "calibrate",
        "--inputs",
        path(&fixture()),
        "--out",
        path(&out),
    ]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8(run.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    let open = lines.iter().position(|l| *l == "{").unwrap();
    assert!(open > 0 && lines[..open].iter().all(|l| l.starts_with("pass ")));
    assert_eq!(lines[lines.len() - 2], "}");
    assert_eq!(lines[lines.len() - 1], format!("wrote {}", out.display()));
    let summary: serde_json::Value =
        serde_json::from_str(&lines[open..lines.len() - 1].join("\n")).unwrap();
    assert_eq!(summary["passes"].as_u64().unwrap() as usize, open);
    for key in [
        "r",
        "pass_history",
        "local_va_gap_quantiles",
        "ln_a_quantiles_traded",
    ] {
        assert!(!summary[key].is_null(), "{key}");
    }
    let report =
        compare::tables(&fixture().join("calibration"), &out, Tolerance::ITERATIVE).unwrap();
    assert!(report.ok, "{:#?}", report.lines());
}

#[test]
fn calibrate_needs_one_input() {
    let fixture = fixture();
    for (args, message) in [
        (
            vec!["calibrate"],
            "choose one input with --pack or --inputs",
        ),
        (
            vec!["calibrate", "--inputs", path(&fixture)],
            "--inputs needs --out",
        ),
        (
            vec![
                "calibrate",
                "--inputs",
                path(&fixture),
                "--pack",
                path(&fixture),
            ],
            "choose one input with --pack or --inputs",
        ),
        (
            vec!["calibrate", "--ticks", "3"],
            "unknown argument '--ticks'",
        ),
    ] {
        let run = ceteris(&args);
        assert!(!run.status.success(), "{args:?}");
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}
