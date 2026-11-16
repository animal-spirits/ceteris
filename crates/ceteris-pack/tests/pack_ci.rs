//! The committed CI pack (`tests/fixtures/pack-ci`, RFC 0353): table hashes against
//! `tests/golden/pack-ci.txt`, the size cap, and no share-alike or validation-only sources.
//! A golden move is a reviewed event: regenerate with `ceteris pack subset` and edit both.

use std::fmt::Write;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

const CAP_BYTES: u64 = 25 * 1_000_000;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> PathBuf {
    root().join("tests/fixtures/pack-ci")
}

#[test]
fn ci_pack_tables_match_golden() {
    let mut names: Vec<_> = std::fs::read_dir(fixture())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".arrow"))
        .collect();
    names.sort();
    let mut actual =
        String::from("# CI pack subset (ADR 0370): table sha256 of tests/fixtures/pack-ci\n");
    for n in names {
        let digest = Sha256::digest(std::fs::read(fixture().join(&n)).unwrap());
        writeln!(actual, "{n} {digest:x}").unwrap();
    }
    let golden = std::fs::read_to_string(root().join("tests/golden/pack-ci.txt")).unwrap();
    assert_eq!(golden, actual);
}

#[test]
fn ci_pack_is_under_cap_and_excludes_restricted_sources() {
    let size: u64 = std::fs::read_dir(fixture())
        .unwrap()
        .map(|e| e.unwrap().metadata().unwrap().len())
        .sum();
    assert!(size <= CAP_BYTES, "{size} bytes over the cap");
    let manifest = std::fs::read_to_string(fixture().join("manifest.json")).unwrap();
    let sources = manifest.split("\"sources\"").nth(1).unwrap().to_lowercase();
    for banned in ["epr", "exiobase"] {
        assert!(
            !sources.contains(&format!("\"source\": \"{banned}")),
            "{banned} in manifest"
        );
    }
}
