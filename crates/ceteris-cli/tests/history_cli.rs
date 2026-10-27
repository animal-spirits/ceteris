//! `ceteris run --history` adds the history files and leaves the run files as they are.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn run(name: &str, extra: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ceteris-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let status = Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(["run", "--fixture", "--ticks", "24"])
        .args(extra)
        .arg("--out")
        .arg(&dir)
        .status()
        .unwrap();
    assert!(status.success());
    dir
}

fn names(dir: &PathBuf) -> Vec<String> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    v.sort();
    v
}

#[test]
fn history_adds_files_and_keeps_the_run_files() {
    let (plain, with) = (run("plain", &[]), run("history", &["--history"]));
    let base = names(&plain);
    assert!(base.iter().all(|n| !n.contains("history")));
    for name in &base {
        assert_eq!(
            fs::read(plain.join(name)).unwrap(),
            fs::read(with.join(name)).unwrap(),
            "{name}"
        );
    }
    let mut extra: Vec<_> = names(&with)
        .into_iter()
        .filter(|n| !base.contains(n))
        .collect();
    extra.sort();
    assert_eq!(
        extra,
        [
            "run_history_markets.arrow",
            "run_history_provinces.arrow",
            "run_history_world.arrow"
        ]
    );
    for d in [plain, with] {
        fs::remove_dir_all(d).unwrap();
    }
}

#[test]
fn sampling_interval_needs_history() {
    let dir = std::env::temp_dir().join(format!("ceteris-cli-{}-bad", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args([
            "run",
            "--fixture",
            "--ticks",
            "5",
            "--history-every",
            "2",
            "--out",
        ])
        .arg(&dir)
        .status()
        .unwrap();
    assert!(!status.success());
}

#[test]
fn history_format_csv_writes_csv_tables() {
    let dir = run("csv", &["--history", "--history-format", "csv"]);
    let mut csv: Vec<_> = names(&dir)
        .into_iter()
        .filter(|n| n.ends_with(".csv"))
        .collect();
    csv.sort();
    assert_eq!(
        csv,
        [
            "run_history_markets.csv",
            "run_history_provinces.csv",
            "run_history_world.csv"
        ]
    );
    assert!(
        names(&dir)
            .iter()
            .all(|n| !n.contains("history") || n.ends_with(".csv"))
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn history_format_needs_history() {
    let dir = std::env::temp_dir().join(format!("ceteris-cli-{}-fmt", std::process::id()));
    let status = Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args([
            "run",
            "--fixture",
            "--ticks",
            "5",
            "--history-format",
            "csv",
            "--out",
        ])
        .arg(&dir)
        .status()
        .unwrap();
    assert!(!status.success());
}
