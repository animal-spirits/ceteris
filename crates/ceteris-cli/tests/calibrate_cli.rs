//! `ceteris calibrate`: the calibration of an export directory, written and printed, and the
//! stability table.

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
        (
            vec![
                "calibrate",
                "--pack",
                path(&fixture),
                "--out",
                path(&fixture),
            ],
            "--out is the pack",
        ),
    ] {
        let run = ceteris(&args);
        assert!(!run.status.success(), "{args:?}");
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}

/// The stability table of the fixture.
#[test]
fn calibrate_stability_prints_the_fixture_table() {
    let run = ceteris(&["calibrate", "stability", path(&fixture())]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(
        String::from_utf8(run.stdout).unwrap(),
        "\
| 0 Agriculture | 10.57 | 3 | 0.168 / 0.149 | 0.104 / 0.096 | 0.130 / 0.126 |
| 1 Crude oil and gas | 10.57 | 1 | 0.169 / 0.151 | 0.048 / 0.074 | 0.016 / 0.095 |
| 2 Coal and other mining | 10.57 | 2 | 0.168 / 0.149 | 0.097 / 0.089 | 0.040 / 0.116 |
| 3 Food | 2.62 | 2 | 0.508 / 0.368 | 0.392 / 0.347 | 0.040 / 0.380 |
| 4 Light manufacturing | 10.57 | 2 | 0.168 / 0.149 | 0.097 / 0.089 | 0.040 / 0.116 |
| 5 Chemicals, refined fuels, metals | 10.57 | 2 | 0.166 / 0.141 | 0.096 / 0.085 | 0.031 / 0.114 |
| 6 Machinery, electronics, vehicles | 5.19 | 2 | 0.307 / 0.250 | 0.214 / 0.195 | 0.040 / 0.225 |
"
    );
}
