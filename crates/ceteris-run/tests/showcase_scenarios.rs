//! Every file of `scenarios/` parses, is described in the manifest with its pre-registered
//! question, edit, horizon, finding and check, and a pinned entry reproduces its run hashes.

use std::path::Path;

use ceteris_run::Scenario;

const UNPINNED: &str = "UNPINNED";
const FIELDS: [&str; 6] = [
    "question",
    "edit",
    "horizon_ticks",
    "finding",
    "check",
    "status",
];

fn dir() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios"))
}

fn manifest() -> Vec<toml::Value> {
    let text = std::fs::read_to_string(dir().join("manifest.toml")).unwrap();
    let value: toml::Value = toml::from_str(&text).unwrap();
    value["scenario"].as_array().unwrap().clone()
}

#[test]
fn manifest_and_files_agree() {
    let entries = manifest();
    let mut listed: Vec<String> = entries
        .iter()
        .map(|e| e["file"].as_str().unwrap().to_string())
        .collect();
    let mut on_disk: Vec<String> = std::fs::read_dir(dir())
        .unwrap()
        .map(|f| f.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.ends_with(".toml") && n != "manifest.toml")
        .collect();
    listed.sort();
    on_disk.sort();
    assert_eq!(listed, on_disk);
    assert!(entries.len() <= 8, "the set is 6 to 8 scenarios");
}

#[test]
fn each_entry_is_complete_and_its_file_is_canonical() {
    for e in manifest() {
        let id = e["id"].as_str().unwrap();
        for f in FIELDS {
            assert!(e.get(f).is_some(), "{id}: missing `{f}`");
        }
        let text = std::fs::read_to_string(dir().join(e["file"].as_str().unwrap())).unwrap();
        let s = Scenario::parse(&text).unwrap();
        assert_eq!(text, s.to_toml(), "{id}: file is not canonical");
        let horizon = e["horizon_ticks"].as_integer().unwrap();
        let last_edit = s
            .branches
            .iter()
            .flat_map(|b| &b.edits)
            .map(|x| i64::from(x.tick))
            .max()
            .unwrap_or(0);
        assert!(horizon > last_edit, "{id}: horizon precedes an edit");
    }
}

#[test]
fn pinned_entries_reproduce_their_hashes() {
    for e in manifest() {
        let id = e["id"].as_str().unwrap();
        let text = std::fs::read_to_string(dir().join(e["file"].as_str().unwrap())).unwrap();
        let s = Scenario::parse(&text).unwrap();
        let pinned = e["status"].as_str().unwrap() == "pinned";
        let unpinned = s.pack_hash == UNPINNED || s.engine == UNPINNED;
        assert_eq!(
            pinned, !unpinned,
            "{id}: status and pack/engine pins disagree"
        );
        if !pinned {
            continue;
        }
        let hashes = e["hashes"]
            .as_table()
            .unwrap_or_else(|| panic!("{id}: no hashes"));
        for b in &s.branches {
            for seed in &s.seeds {
                let key = format!("{}.{seed}", b.id);
                let want = hashes[&key].as_str().unwrap();
                let got = ceteris_run::commands::share(&s, &b.id, *seed).unwrap();
                assert_eq!(want, got, "{id}: {key}");
            }
        }
    }
}
