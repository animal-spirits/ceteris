//! An injected halt writes a repro bundle that `ceteris replay` reproduces (ADR 0214).

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

fn ceteris(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(args)
        .output()
        .unwrap()
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ceteris-halt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn injected_halt_writes_a_bundle_that_replays() {
    let out = scratch("run");
    let o = ceteris(&[
        "run",
        "--fixture",
        "--ticks",
        "24",
        "--inject-halt",
        "7",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert!(!o.status.success());
    let bundle = out.join("bundle");
    let text = fs::read_to_string(bundle.join("bundle.txt")).unwrap();
    assert!(text.contains("halt-tick=7\n") && text.contains("halt-invariant=injected\n"));
    assert!(!text.contains("--out"));

    let r = ceteris(&["replay", bundle.to_str().unwrap()]);
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(String::from_utf8_lossy(&r.stdout).contains("reproduced"));
}

#[test]
fn replay_of_a_run_that_no_longer_halts_fails() {
    let out = scratch("tampered");
    ceteris(&[
        "run",
        "--fixture",
        "--ticks",
        "24",
        "--inject-halt",
        "7",
        "--out",
        out.to_str().unwrap(),
    ]);
    let file = out.join("bundle/bundle.txt");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(
        &file,
        text.replace("arg=--inject-halt\narg=7\n", "")
            .replace("arg=--inject-halt", ""),
    )
    .unwrap();
    let r = ceteris(&["replay", out.join("bundle").to_str().unwrap()]);
    assert!(!r.status.success());
}
