//! `ceteris pack check-sources` output streams and exit codes.

use std::fs;
use std::process::{Command, Output};

use ceteris_pack::sources::CONFIG_DIR;

fn check(sources: &str, args: &[&str]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(CONFIG_DIR);
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("sources.toml"), sources).unwrap();
    fs::write(config.join("fetch.toml"), "").unwrap();
    fs::write(config.join("validation.toml"), "").unwrap();
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(["pack", "check-sources"])
        .args(args)
        .current_dir(dir.path())
        .output()
        .unwrap()
}

#[test]
fn a_clean_manifest_prints_the_summary_and_exits_zero() {
    let out = check(
        "[project]\ndistribution = \"closed\"\n",
        &["--profile", "research"],
    );
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "closed, research pack: 0 sources, 0 shippable, 0 closed-source-only, 0 problems\n"
    );
    assert!(out.stderr.is_empty());
}

#[test]
fn problems_go_to_stderr_and_exit_one() {
    let out = check("", &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "project.distribution must be one of ['closed', 'open']\n"
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "None: 0 sources, 0 shippable, 0 closed-source-only, 1 problems\n"
    );
}

#[test]
fn an_unknown_profile_is_a_usage_error() {
    let out = check("", &["--profile", "other"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--profile must be research or release"));
}
