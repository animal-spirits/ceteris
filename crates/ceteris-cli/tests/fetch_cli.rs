//! `ceteris pack fetch` on a cache that is complete, so no request is made.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const CONFIG: &str = "python/ceteris/packs";

fn setup(root: &Path, fetch_toml: &str) {
    let config = root.join(CONFIG);
    fs::create_dir_all(&config).unwrap();
    fs::create_dir_all(root.join("data/cache")).unwrap();
    fs::write(config.join("sources.toml"), "[[source]]\nid = \"a\"\n").unwrap();
    fs::write(config.join("validation.toml"), "").unwrap();
    fs::write(config.join("fetch.toml"), fetch_toml).unwrap();
}

fn fetch(root: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(["pack", "fetch"])
        .args(flags)
        .current_dir(root)
        .output()
        .unwrap()
}

fn entry(name: &str, pin: Option<(&str, usize)>, extra: &str) -> String {
    let pin = pin.map_or(String::new(), |(sha, n)| {
        format!("sha256 = \"{sha}\"\nbytes = {n}\n")
    });
    format!(
        "[[file]]\nsource = \"a\"\nname = \"{name}\"\nurl = \"http://127.0.0.1:9/{name}\"\n{pin}{extra}\n"
    )
}

#[test]
fn complete_pinned_cache_succeeds_without_network() {
    let root = tempfile::tempdir().unwrap();
    let sha = format!("{:x}", Sha256::digest(b"abc"));
    setup(root.path(), &entry("a.bin", Some((&sha, 3)), ""));
    fs::write(root.path().join("data/cache/a.bin"), b"abc").unwrap();
    let out = fetch(root.path(), &[]);
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
}

#[test]
fn unpinned_file_fails_unless_recorded() {
    let root = tempfile::tempdir().unwrap();
    setup(root.path(), &entry("a.bin", None, ""));
    fs::write(root.path().join("data/cache/a.bin"), b"abc").unwrap();
    let out = fetch(root.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "a.bin: not pinned (run with --record)\n"
    );

    let out = fetch(root.path(), &["--record"]);
    assert!(out.status.success());
    let sha = format!("{:x}", Sha256::digest(b"abc"));
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        format!("a.bin\nsha256 = \"{sha}\"\nbytes = 3\n\n")
    );
}

#[test]
fn wrong_pin_reports_the_found_checksum() {
    let root = tempfile::tempdir().unwrap();
    setup(root.path(), &entry("a.bin", Some(("00", 3)), ""));
    fs::write(root.path().join("data/cache/a.bin"), b"abc").unwrap();
    let out = fetch(root.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
    let sha = format!("{:x}", Sha256::digest(b"abc"));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!("a.bin: checksum mismatch, got {sha} (3 bytes)\n")
    );
}

#[test]
fn absent_optional_is_skipped_and_absent_manual_stops() {
    let root = tempfile::tempdir().unwrap();
    let both = entry("o.bin", Some(("00", 3)), "optional = true\nmanual = true\n")
        + &entry("m.bin", Some(("00", 3)), "manual = true\n");
    setup(root.path(), &both);
    let out = fetch(root.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.starts_with(
        "m.bin: the host rejects scripted downloads; save http://127.0.0.1:9/m.bin into "
    ));
    assert!(!stderr.contains("o.bin"));

    // A cached file must match its pin; the absent optional one is still not asked for.
    let sha = format!("{:x}", Sha256::digest(b"abc"));
    fs::write(root.path().join("data/cache/m.bin"), b"abc").unwrap();
    let out = fetch(root.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        format!("m.bin: checksum mismatch, got {sha} (3 bytes)\n")
    );

    // Once cached, the optional file is verified too.
    fs::write(root.path().join("data/cache/o.bin"), b"abc").unwrap();
    let out = fetch(root.path(), &[]);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains(&format!("o.bin: checksum mismatch, got {sha} (3 bytes)\n"))
    );
}
