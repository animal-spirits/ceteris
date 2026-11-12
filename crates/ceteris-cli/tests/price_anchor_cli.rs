#![allow(clippy::disallowed_methods)] // tests are not hashed
//! `ceteris run --price-anchor off|e1|e3` (the free-moment rerun and the RFC 0286 screen):
//! each rule reaches the run, `off` is the default, and storage comes from the calibration.

use std::path::Path;
use std::process::{Command, Output};

fn run(out: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["run", "--fixture", "--ticks", "24", "--out"];
    args.push(out.to_str().unwrap());
    args.extend(extra);
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(&args)
        .output()
        .unwrap()
}

fn prices(extra: &[&str]) -> Vec<u8> {
    let tmp = tempfile::tempdir().unwrap();
    let o = run(tmp.path(), extra);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    std::fs::read(tmp.path().join("run_prices.arrow")).unwrap()
}

#[test]
fn price_anchor_reaches_the_run() {
    let default = prices(&[]);
    let off = prices(&["--price-anchor", "off"]);
    let e1 = prices(&["--price-anchor", "e1"]);
    let e3 = prices(&["--price-anchor", "e3"]);
    assert_eq!(default, off);
    assert_ne!(off, e1);
    assert_ne!(e1, e3);
}

#[test]
fn bad_price_anchor_and_run_storage_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let o = run(tmp.path(), &["--price-anchor", "e2"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("choose off, e1 or e3"));
    let o = run(tmp.path(), &["--storage"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("--storage belongs to export"));
}
