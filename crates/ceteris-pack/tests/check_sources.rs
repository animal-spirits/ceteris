//! The source manifest check: profiles, validation files, and one mutation per manifest rule.

use std::fs;
use std::path::Path;

use ceteris_pack::sources::{self, CONFIG_DIR, Profile};

const PACK_INPUT: &str = r#"
[[source]]
id = "pack-input"
name = "P"
provider = "P"
version = "1"
role = "test"
licence = "CC-BY-4.0"
licence_url = "https://example.org/licence"
attribution = "P"
commercial_use = true
share_alike = false
use = "primary"
policy = "fetch"
reason = "test"
checked = 2026-10-25
confidence = "high"
"#;

const SAMPLE: &str = r#"
[[source]]
id = "sample"
name = "S"
provider = "S"
version = "1"
role = "test"
licence = "LicenseRef-all-rights-reserved"
licence_url = "https://example.org/terms"
attribution = "S"
commercial_use = false
share_alike = false
use = "{use}"
policy = "fetch"
reason = "test"
checked = 2026-10-25
confidence = "medium"
closed_source_only = true
"#;

struct Config {
    dir: tempfile::TempDir,
}

impl Config {
    /// A manifest with the given source blocks and the file lists naming them.
    fn new(sources: &str, fetch: &[&str], validation: &[&str]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join(CONFIG_DIR);
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("sources.toml"),
            format!("[project]\ndistribution = \"closed\"\n{sources}"),
        )
        .unwrap();
        for (name, ids) in [("fetch.toml", fetch), ("validation.toml", validation)] {
            let files: String = ids
                .iter()
                .map(|i| format!("[[file]]\nsource = \"{i}\"\nname = \"{i}.csv\"\nurl = \"u\"\n\n"))
                .collect();
            fs::write(config.join(name), files).unwrap();
        }
        Self { dir }
    }

    fn single(source: &str) -> Self {
        Self::new(source, &["pack-input"], &[])
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn lines(&self, profile: Option<Profile>) -> Vec<String> {
        sources::problems(self.path(), profile)
            .unwrap()
            .iter()
            .map(ToString::to_string)
            .collect()
    }
}

fn synthetic(use_: &str, sample_in: &str) -> Config {
    let sample = SAMPLE.replace("{use}", use_);
    let (fetch, validation): (&[&str], &[&str]) = match sample_in {
        "fetch" => (&["pack-input", "sample"], &[]),
        _ => (&["pack-input"], &["sample"]),
    };
    Config::new(&format!("{PACK_INPUT}{sample}"), fetch, validation)
}

fn mutated(from: &str, to: &str) -> Config {
    assert!(PACK_INPUT.contains(from), "{from} not in the base source");
    Config::single(&PACK_INPUT.replacen(from, to, 1))
}

fn without(line_start: &str) -> Config {
    let kept: String = PACK_INPUT
        .lines()
        .filter(|l| !l.starts_with(line_start))
        .map(|l| format!("{l}\n"))
        .collect();
    Config::single(&kept)
}

#[test]
fn research_profile_allows_non_commercial_and_closed_source_only() {
    let config = synthetic("validation", "fetch");
    assert!(config.lines(Some(Profile::Research)).is_empty());
}

#[test]
fn release_profile_rejects_non_commercial_closed_and_low_confidence() {
    let sample = SAMPLE
        .replace("{use}", "primary")
        .replace("\"medium\"", "\"low\"");
    let config = Config::new(&sample, &["sample"], &[]);
    assert_eq!(
        config.lines(Some(Profile::Release)),
        [
            "sample: release pack reads a source without commercial use",
            "sample: release pack reads closed-source-only data",
            "sample: release pack reads a source on a low-confidence licence reading",
        ]
    );
}

#[test]
fn release_profile_accepts_a_commercial_confident_source() {
    let config = Config::single(PACK_INPUT);
    assert!(config.lines(Some(Profile::Release)).is_empty());
}

#[test]
fn no_profile_reads_an_excluded_source() {
    let excluded = PACK_INPUT
        .replace("\"primary\"", "\"rejected\"")
        .replace("\"fetch\"", "\"exclude\"");
    let config = Config::single(&excluded);
    for (profile, name) in [
        (Profile::Research, "research"),
        (Profile::Release, "release"),
    ] {
        assert_eq!(
            config.lines(Some(profile)),
            [format!(
                "pack-input: excluded source read by the {name} pack"
            )]
        );
    }
}

#[test]
fn pack_sources_are_in_the_manifest_and_the_current_pack_passes_both_profiles() {
    let root = sources::workspace_root();
    assert!(!sources::pack_sources(&root).unwrap().is_empty());
    for profile in [Profile::Research, Profile::Release] {
        assert_eq!(sources::problems(&root, Some(profile)).unwrap(), []);
    }
}

#[test]
fn a_validation_sample_outside_the_pack_passes_the_release_profile() {
    let config = synthetic("validation", "validation");
    let packed = sources::pack_sources(config.path()).unwrap();
    assert_eq!(packed.into_iter().collect::<Vec<_>>(), ["pack-input"]);
    for profile in [Profile::Research, Profile::Release] {
        assert!(config.lines(Some(profile)).is_empty());
    }
}

#[test]
fn the_same_sample_read_by_the_pack_fails_only_the_release_profile() {
    let config = synthetic("validation", "fetch");
    assert!(config.lines(Some(Profile::Research)).is_empty());
    assert_eq!(config.lines(Some(Profile::Release)).len(), 2);
}

#[test]
fn validation_files_must_belong_to_validation_sources() {
    let config = synthetic("complement", "validation");
    assert_eq!(
        config.lines(None),
        ["sample: in validation.toml but its use is 'complement', not validation"]
    );
}

#[test]
fn summary_counts_sources_and_names_the_profile() {
    let config = synthetic("validation", "validation");
    let report = sources::check(config.path(), Some(Profile::Release)).unwrap();
    assert_eq!(
        report.summary(),
        "closed, release pack: 2 sources, 0 shippable, 1 closed-source-only, 0 problems"
    );
    let report = sources::check(config.path(), None).unwrap();
    assert_eq!(
        report.summary(),
        "closed: 2 sources, 0 shippable, 1 closed-source-only, 0 problems"
    );
}

#[test]
fn a_missing_required_field_is_reported_and_stops_the_other_checks() {
    for (field, kind) in [
        ("id", "str"),
        ("name", "str"),
        ("provider", "str"),
        ("version", "str"),
        ("role", "str"),
        ("licence", "str"),
        ("licence_url", "str"),
        ("attribution", "str"),
        ("commercial_use", "bool"),
        ("share_alike", "bool"),
        ("use", "str"),
        ("policy", "str"),
        ("reason", "str"),
        ("checked", "date"),
        ("confidence", "str"),
    ] {
        let sid = if field == "id" { "?" } else { "pack-input" };
        let found = without(&format!("{field} = ")).lines(None);
        assert!(
            found.contains(&format!("{sid}: field '{field}' missing or not {kind}")),
            "{field}: {found:?}"
        );
    }
}

#[test]
fn a_field_of_the_wrong_type_is_reported() {
    for (from, to, message) in [
        (
            "commercial_use = true",
            "commercial_use = \"yes\"",
            "field 'commercial_use' missing or not bool",
        ),
        (
            "checked = 2026-10-25",
            "checked = \"2026-10-25\"",
            "field 'checked' missing or not date",
        ),
        (
            "version = \"1\"",
            "version = 1",
            "field 'version' missing or not str",
        ),
    ] {
        assert_eq!(
            mutated(from, to).lines(None),
            [format!("pack-input: {message}")]
        );
    }
}

#[test]
fn value_rules_report_their_own_line() {
    for (from, to, message) in [
        (
            "policy = \"fetch\"",
            "policy = \"bogus\"",
            "unknown policy 'bogus'",
        ),
        (
            "use = \"primary\"",
            "use = \"bogus\"",
            "unknown use 'bogus'",
        ),
        (
            "use = \"primary\"",
            "use = \"rejected\"",
            "rejected sources must have policy exclude",
        ),
        (
            "confidence = \"high\"",
            "confidence = \"bogus\"",
            "unknown confidence 'bogus'",
        ),
        (
            "https://example.org",
            "http://example.org",
            "licence_url must be https",
        ),
        (
            "checked = 2026-10-25",
            "checked = 2026-10-25\nreplaces = \"nope\"",
            "replaces names unknown source 'nope'",
        ),
        (
            "checked = 2026-10-25",
            "checked = 2026-10-25\nreplaced_by = \"nope\"",
            "replaced_by names unknown source 'nope'",
        ),
    ] {
        assert_eq!(
            mutated(from, to).lines(None),
            [format!("pack-input: {message}")]
        );
    }
}

#[test]
fn shipped_sources_are_held_to_the_commercial_rules() {
    let shipped = |from: &str, to: &str| {
        let base = PACK_INPUT.replace("\"fetch\"", "\"bundle\"");
        Config::single(&base.replacen(from, to, 1)).lines(None)
    };
    assert_eq!(
        shipped("commercial_use = true", "commercial_use = false"),
        ["pack-input: shipped but commercial use not allowed"]
    );
    assert_eq!(
        shipped("confidence = \"high\"", "confidence = \"low\""),
        ["pack-input: shipped on a low-confidence licence reading"]
    );
    assert_eq!(
        shipped("share_alike = false", "share_alike = true"),
        ["pack-input: share-alike data must set shipped_under = licence"]
    );
    assert!(
        shipped(
            "share_alike = false",
            "share_alike = true\nshipped_under = \"CC-BY-4.0\""
        )
        .is_empty()
    );
    assert_eq!(
        shipped(
            "confidence = \"high\"",
            "confidence = \"high\"\nclosed_source_only = true"
        ),
        ["pack-input: closed-source-only data cannot be shipped"]
    );
}

#[test]
fn manifest_level_rules() {
    let config = Config::new(&format!("{PACK_INPUT}{PACK_INPUT}"), &["pack-input"], &[]);
    assert_eq!(config.lines(None), ["duplicate id 'pack-input'"]);

    let config = Config::single(PACK_INPUT);
    let manifest = config.path().join(CONFIG_DIR).join("sources.toml");
    let body = fs::read_to_string(&manifest).unwrap();
    let bad = "project.distribution must be one of ['closed', 'open']";
    fs::write(&manifest, body.replace("\"closed\"", "\"bogus\"")).unwrap();
    assert_eq!(config.lines(None), [bad]);
    fs::write(&manifest, body.replace("distribution = \"closed\"", "")).unwrap();
    assert_eq!(config.lines(None), [bad]);

    let closed = Config::new(&SAMPLE.replace("{use}", "primary"), &["sample"], &[]);
    let manifest = closed.path().join(CONFIG_DIR).join("sources.toml");
    let body = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, body.replace("\"closed\"", "\"open\"")).unwrap();
    assert_eq!(
        closed.lines(None),
        ["sample: closed-source-only data in an open distribution"]
    );
}

#[test]
fn pack_and_validation_files_must_name_manifest_sources() {
    let config = Config::new(PACK_INPUT, &["pack-input", "nowhere"], &["elsewhere"]);
    assert_eq!(
        config.lines(Some(Profile::Research)),
        [
            "elsewhere: named in validation.toml but missing from sources.toml",
            "nowhere: read by the pack but missing from sources.toml",
        ]
    );
}

#[test]
fn the_county_lock_adds_geoboundaries_to_the_pack_sources() {
    let config = Config::single(PACK_INPUT);
    fs::write(
        config.path().join(CONFIG_DIR).join("counties.lock.toml"),
        "",
    )
    .unwrap();
    assert_eq!(
        config.lines(Some(Profile::Research)),
        ["geoboundaries: read by the pack but missing from sources.toml"]
    );
}

const HELD_OUT: &str = "holdout = \"{class}\"\n";

/// A manifest whose `sample` source is held out, with the validation file written by hand.
fn held_out(class: &str, validation_file: &str, extra_files: &[(&str, &str)]) -> Config {
    let sample = SAMPLE.replace("{use}", "validation").replace(
        "closed_source_only",
        &format!("{}closed_source_only", HELD_OUT.replace("{class}", class)),
    );
    let config = Config::new(&format!("{PACK_INPUT}{sample}"), &["pack-input"], &[]);
    let dir = config.path().join(CONFIG_DIR);
    fs::write(dir.join("validation.toml"), validation_file).unwrap();
    for (path, body) in extra_files {
        let full = config.path().join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, body).unwrap();
    }
    config
}

const READ_FILE: &str = "[[file]]\nsource = \"sample\"\nname = \"sample.csv\"\nsha256 = \"abc\"\nurl = \"u\"\nread_by = [\"crates/x/tests/score.rs\"]\n";
const PREREG: &str =
    "[[entry]]\nsource = \"sample\"\nmetric = \"m\"\ntolerance = \"t\"\ntarget_sha256 = \"abc\"\n";

fn has(lines: &[String], needle: &str) -> bool {
    lines.iter().any(|l| l.contains(needle))
}

#[test]
fn a_registered_held_out_source_read_only_by_its_scorer_passes() {
    let config = held_out(
        "H",
        READ_FILE,
        &[
            ("crates/x/tests/score.rs", "const F: &str = \"sample.csv\";"),
            (&format!("{CONFIG_DIR}/prereg.toml"), PREREG),
        ],
    );
    assert!(config.lines(None).is_empty(), "{:?}", config.lines(None));
}

#[test]
fn a_held_out_source_cannot_be_in_the_pack_build() {
    let sample = SAMPLE.replace("{use}", "validation").replace(
        "closed_source_only",
        &format!("{}closed_source_only", HELD_OUT.replace("{class}", "R")),
    );
    let config = Config::new(
        &format!("{PACK_INPUT}{sample}"),
        &["pack-input", "sample"],
        &[],
    );
    assert!(has(
        &config.lines(None),
        "held out but read by the pack build"
    ));
}

#[test]
fn a_held_out_file_needs_read_by() {
    let file = READ_FILE.replace("read_by = [\"crates/x/tests/score.rs\"]\n", "");
    let config = held_out("R", &file, &[]);
    assert!(has(&config.lines(None), "needs read_by"));
}

#[test]
fn another_file_opening_a_held_out_file_is_reported() {
    let config = held_out(
        "R",
        READ_FILE,
        &[
            ("crates/x/tests/score.rs", "\"sample.csv\""),
            (
                "crates/ceteris-calibrate/src/fit.rs",
                "open(\"sample.csv\")",
            ),
        ],
    );
    let lines = config.lines(None);
    assert!(has(
        &lines,
        "crates/ceteris-calibrate/src/fit.rs opens held-out sample.csv"
    ));
    assert!(!has(&lines, "crates/x/tests/score.rs opens"));
}

#[test]
fn a_scored_h_source_needs_a_preregistration_with_its_hash() {
    let none = held_out("H", READ_FILE, &[]);
    assert!(has(&none.lines(None), "prereg.toml has no entry"));
    let wrong = held_out(
        "H",
        READ_FILE,
        &[(
            &format!("{CONFIG_DIR}/prereg.toml"),
            &PREREG.replace("abc", "def"),
        )],
    );
    assert!(has(&wrong.lines(None), "prereg.toml has no entry"));
    let incomplete = held_out(
        "H",
        READ_FILE,
        &[(
            &format!("{CONFIG_DIR}/prereg.toml"),
            &PREREG.replace("tolerance = \"t\"\n", ""),
        )],
    );
    assert!(has(&incomplete.lines(None), "lacks 'tolerance'"));
    let r_source = held_out(
        "R",
        READ_FILE,
        &[(&format!("{CONFIG_DIR}/prereg.toml"), PREREG)],
    );
    assert!(has(&r_source.lines(None), "not H"));
}

#[test]
fn holdout_class_and_use_are_checked() {
    let bad = held_out("X", READ_FILE, &[]);
    assert!(has(&bad.lines(None), "holdout must be 'H' or 'R'"));
    let primary = Config::single(&PACK_INPUT.replacen(
        "use = \"primary\"",
        "use = \"primary\"\nholdout = \"H\"",
        1,
    ));
    assert!(has(&primary.lines(None), "not validation"));
}

#[test]
fn leaving_the_held_out_set_names_the_replacement() {
    let moved = format!("{PACK_INPUT}was_holdout = true\n");
    assert!(has(
        &Config::single(&moved).lines(None),
        "replaces_holdout must name an H source"
    ));
}

#[test]
fn the_current_manifest_passes_the_holdout_register() {
    let lines: Vec<String> = sources::problems(&sources::workspace_root(), None)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(lines.is_empty(), "{lines:?}");
}
