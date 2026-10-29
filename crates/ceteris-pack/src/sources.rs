//! The data-source licence manifest check (Sections 2.5, 16, 22; ADRs 0005, 0050).
//!
//! `sources.toml` lists every source the pipeline may read. With a profile, the check also
//! covers the sources the pack build reads (`fetch.toml`, plus geoBoundaries when the county lock
//! exists): a research pack may read non-commercial and closed-source-only sources; a release
//! pack may read only sources whose licence allows commercial use on a confident reading. No
//! profile reads an excluded source. Files that only tests read (`validation.toml`) are not pack
//! sources; each must name a source with `use = "validation"`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

/// Where the pack configuration lives, relative to the workspace root.
pub const CONFIG_DIR: &str = "crates/ceteris-pack/config";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Research,
    Release,
}

impl Profile {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "research" => Some(Self::Research),
            "release" => Some(Self::Release),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Release => "release",
        }
    }
}

/// One line of the check's output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem(pub String);

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Report {
    pub problems: Vec<Problem>,
    summary: String,
}

impl Report {
    /// The one-line stdout summary.
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Str,
    Bool,
    Date,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Str => "str",
            Self::Bool => "bool",
            Self::Date => "date",
        }
    }

    fn matches(self, value: &Value) -> bool {
        match (self, value) {
            (Self::Str, Value::String(_)) | (Self::Bool, Value::Boolean(_)) => true,
            (Self::Date, Value::Datetime(d)) => d.date.is_some(),
            _ => false,
        }
    }
}

const REQUIRED: [(&str, Kind); 15] = [
    ("id", Kind::Str),
    ("name", Kind::Str),
    ("provider", Kind::Str),
    ("version", Kind::Str),
    ("role", Kind::Str),
    ("licence", Kind::Str),
    ("licence_url", Kind::Str),
    ("attribution", Kind::Str),
    ("commercial_use", Kind::Bool),
    ("share_alike", Kind::Bool),
    ("use", Kind::Str),
    ("policy", Kind::Str),
    ("reason", Kind::Str),
    ("checked", Kind::Date),
    ("confidence", Kind::Str),
];
const POLICIES: [&str; 4] = ["bundle", "bundle-aggregates", "fetch", "exclude"];
const DISTRIBUTIONS: [&str; 2] = ["closed", "open"];
// primary: feeds the pack; complement: fills gaps; fallback: used where no open source is
// adequate; validation: scoring only; rejected: audited and not used.
const USES: [&str; 5] = [
    "primary",
    "complement",
    "fallback",
    "validation",
    "rejected",
];
const CONFIDENCE: [&str; 3] = ["high", "medium", "low"];
const SHIPPED: [&str; 2] = ["bundle", "bundle-aggregates"];

fn text<'a>(source: &'a Table, key: &str) -> Option<&'a str> {
    source.get(key).and_then(Value::as_str)
}

fn flag(source: &Table, key: &str) -> bool {
    match source.get(key) {
        None => false,
        Some(Value::Boolean(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Integer(i)) => *i != 0,
        Some(Value::Float(f)) => *f != 0.0,
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Table(t)) => !t.is_empty(),
        Some(Value::Datetime(_)) => true,
    }
}

fn shown(value: Option<&str>) -> &str {
    value.unwrap_or("None")
}

fn is_shipped(source: &Table) -> bool {
    text(source, "policy").is_some_and(|p| SHIPPED.contains(&p))
}

fn source_problems(source: &Table, ids: &BTreeSet<&str>) -> Vec<String> {
    let sid = shown(text(source, "id").or(Some("?")));
    let mut found: Vec<String> = REQUIRED
        .iter()
        .filter(|(field, kind)| !source.get(*field).is_some_and(|v| kind.matches(v)))
        .map(|(field, kind)| format!("{sid}: field '{field}' missing or not {}", kind.name()))
        .collect();
    if !found.is_empty() {
        return found;
    }
    let get = |key: &str| text(source, key).unwrap_or_default();
    let (policy, usage, confidence) = (get("policy"), get("use"), get("confidence"));
    if !POLICIES.contains(&policy) {
        found.push(format!("{sid}: unknown policy '{policy}'"));
    }
    if !USES.contains(&usage) {
        found.push(format!("{sid}: unknown use '{usage}'"));
    }
    if usage == "rejected" && policy != "exclude" {
        found.push(format!("{sid}: rejected sources must have policy exclude"));
    }
    if !CONFIDENCE.contains(&confidence) {
        found.push(format!("{sid}: unknown confidence '{confidence}'"));
    }
    if !get("licence_url").starts_with("https://") {
        found.push(format!("{sid}: licence_url must be https"));
    }
    if SHIPPED.contains(&policy) {
        // Anything we redistribute must allow the commercial use our MIT/Apache licence permits.
        if !flag(source, "commercial_use") {
            found.push(format!("{sid}: shipped but commercial use not allowed"));
        }
        if confidence == "low" {
            found.push(format!(
                "{sid}: shipped on a low-confidence licence reading"
            ));
        }
        if flag(source, "share_alike") && source.get("shipped_under") != source.get("licence") {
            found.push(format!(
                "{sid}: share-alike data must set shipped_under = licence"
            ));
        }
    }
    if flag(source, "closed_source_only") && SHIPPED.contains(&policy) {
        found.push(format!("{sid}: closed-source-only data cannot be shipped"));
    }
    for link in ["replaces", "replaced_by"] {
        if let Some(target) = source.get(link) {
            let known = target.as_str().is_some_and(|t| ids.contains(t));
            if !known {
                let name = target
                    .as_str()
                    .map_or_else(|| target.to_string(), str::to_owned);
                found.push(format!("{sid}: {link} names unknown source '{name}'"));
            }
        }
    }
    found
}

pub(crate) fn read_table(path: &Path) -> Result<Table, String> {
    let body = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    body.parse().map_err(|e| format!("{}: {e}", path.display()))
}

fn file_sources(table: &Table) -> impl Iterator<Item = &str> {
    table
        .get("file")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| f.get("source").and_then(Value::as_str))
}

/// Ids of the sources the pack build reads.
pub fn pack_sources(root: &Path) -> Result<BTreeSet<String>, String> {
    let config = root.join(CONFIG_DIR);
    let mut used: BTreeSet<String> = file_sources(&read_table(&config.join("fetch.toml"))?)
        .map(str::to_owned)
        .collect();
    if config.join("counties.lock.toml").exists() {
        used.insert("geoboundaries".into());
    }
    Ok(used)
}

/// Files outside the pack must belong to validation-only sources, so the file list cannot hide a
/// pack input.
fn validation_problems(root: &Path, by_id: &BTreeMap<&str, &Table>) -> Result<Vec<String>, String> {
    let table = read_table(&root.join(CONFIG_DIR).join("validation.toml"))?;
    Ok(file_sources(&table)
        .filter_map(|id| match by_id.get(id) {
            None => Some(format!(
                "{id}: named in validation.toml but missing from sources.toml"
            )),
            Some(s) if text(s, "use") != Some("validation") => Some(format!(
                "{id}: in validation.toml but its use is '{}', not validation",
                shown(text(s, "use"))
            )),
            Some(_) => None,
        })
        .collect())
}

fn profile_problems(source: &Table, profile: Profile) -> Vec<String> {
    let sid = text(source, "id").unwrap_or_default();
    if text(source, "policy") == Some("exclude") {
        return vec![format!(
            "{sid}: excluded source read by the {} pack",
            profile.name()
        )];
    }
    if profile == Profile::Research {
        return Vec::new();
    }
    let mut found = Vec::new();
    if !flag(source, "commercial_use") {
        found.push(format!(
            "{sid}: release pack reads a source without commercial use"
        ));
    }
    if flag(source, "closed_source_only") {
        found.push(format!("{sid}: release pack reads closed-source-only data"));
    }
    if text(source, "confidence") == Some("low") {
        found.push(format!(
            "{sid}: release pack reads a source on a low-confidence licence reading"
        ));
    }
    found
}

/// Checks `sources.toml` under `root`, and the pack's sources too when a profile is given.
pub fn check(root: &Path, profile: Option<Profile>) -> Result<Report, String> {
    let manifest = read_table(&root.join(CONFIG_DIR).join("sources.toml"))?;
    let distribution = manifest
        .get("project")
        .and_then(|p| p.get("distribution"))
        .and_then(Value::as_str);
    let sources: Vec<&Table> = manifest
        .get("source")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_table)
        .collect();
    let id_list: Vec<Option<&str>> = sources.iter().map(|s| text(s, "id")).collect();
    let ids: BTreeSet<&str> = id_list.iter().flatten().copied().collect();

    let mut errors: Vec<String> = ids
        .iter()
        .filter(|id| id_list.iter().filter(|i| i == &&Some(**id)).count() > 1)
        .map(|id| format!("duplicate id '{id}'"))
        .collect();
    if !distribution.is_some_and(|d| DISTRIBUTIONS.contains(&d)) {
        errors.push("project.distribution must be one of ['closed', 'open']".into());
    }
    for source in &sources {
        errors.extend(source_problems(source, &ids));
        // Opening the project requires every closed-source fallback to be resolved (ADR 0005).
        if distribution == Some("open") && flag(source, "closed_source_only") {
            errors.push(format!(
                "{}: closed-source-only data in an open distribution",
                shown(text(source, "id"))
            ));
        }
    }
    let by_id: BTreeMap<&str, &Table> = sources
        .iter()
        .filter_map(|s| text(s, "id").map(|id| (id, *s)))
        .collect();
    errors.extend(validation_problems(root, &by_id)?);
    if let Some(profile) = profile {
        for sid in pack_sources(root)? {
            match by_id.get(sid.as_str()) {
                None => errors.push(format!(
                    "{sid}: read by the pack but missing from sources.toml"
                )),
                Some(s) if source_problems(s, &ids).is_empty() => {
                    errors.extend(profile_problems(s, profile));
                }
                Some(_) => {}
            }
        }
    }

    let shipped = sources.iter().filter(|s| is_shipped(s)).count();
    let closed = sources
        .iter()
        .filter(|s| flag(s, "closed_source_only"))
        .count();
    let pack = profile.map_or(String::new(), |p| format!(", {} pack", p.name()));
    let summary = format!(
        "{}{pack}: {} sources, {shipped} shippable, {closed} closed-source-only, {} problems",
        shown(distribution),
        sources.len(),
        errors.len()
    );
    Ok(Report {
        problems: errors.into_iter().map(Problem).collect(),
        summary,
    })
}

/// The problems `check` finds.
pub fn problems(root: &Path, profile: Option<Profile>) -> Result<Vec<Problem>, String> {
    check(root, profile).map(|r| r.problems)
}

/// The workspace root: the current directory when it holds the configuration, else the checkout
/// this crate was built from.
pub fn workspace_root() -> PathBuf {
    let here = PathBuf::from(".");
    if here.join(CONFIG_DIR).join("sources.toml").exists() {
        return here;
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
