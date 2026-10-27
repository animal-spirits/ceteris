//! `ceteris pack compare` exit codes, filtering and usage errors.

use std::path::Path;
use std::process::{Command, Output};
use std::sync::Arc;

use ceteris_pack::table::write;

use arrow_array::{ArrayRef, Float64Array, RecordBatch};

fn put(dir: &Path, name: &str, x: &[f64]) {
    let col: ArrayRef = Arc::new(Float64Array::from(x.to_vec()));
    let batch = RecordBatch::try_from_iter([("x", col)]).unwrap();
    write(
        &dir.join(format!("{name}.arrow")),
        &batch,
        &Default::default(),
    )
    .unwrap();
}

fn compare(args: &[&Path], flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(["pack", "compare"])
        .args(flags)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn equal_tables_exit_zero() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "t", &[1.0, 2.0]);
    put(b.path(), "t", &[1.0, 2.0]);
    let out = compare(&[a.path(), b.path()], &[]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("t.x: ok"));
}

#[test]
fn float_beyond_tolerance_exits_one_and_iterative_loosens() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "t", &[1.0, 2.0]);
    put(b.path(), "t", &[1.0, 2.0 * (1.0 + 5e-12)]);
    let out = compare(&[a.path(), b.path()], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).contains("t.x: FAIL"));
    assert!(
        compare(&[a.path(), b.path()], &["--iterative"])
            .status
            .success()
    );
}

#[test]
fn table_filter_limits_the_comparison() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "good", &[1.0]);
    put(b.path(), "good", &[1.0]);
    put(a.path(), "bad", &[1.0]);
    put(b.path(), "bad", &[2.0]);
    assert_eq!(compare(&[a.path(), b.path()], &[]).status.code(), Some(1));
    let out = compare(&[a.path(), b.path()], &["--table", "good"]);
    assert!(out.status.success());
    assert!(!String::from_utf8_lossy(&out.stdout).contains("bad"));
    assert_eq!(
        compare(&[a.path(), b.path()], &["--table", "nope"])
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn usage_errors_exit_one() {
    let a = tempfile::tempdir().unwrap();
    put(a.path(), "t", &[1.0]);
    let unknown = compare(&[a.path(), a.path()], &["--bogus"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown argument"));
    let missing = compare(&[a.path()], &[]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("two directories"));
}

#[test]
fn directories_without_tables_exit_one() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let out = compare(&[a.path(), b.path()], &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no .arrow tables"));
}
