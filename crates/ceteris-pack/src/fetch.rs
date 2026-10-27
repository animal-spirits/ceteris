//! Downloads the pack's source files into the cache and verifies their checksums (Section 16.1).
//!
//! `fetch.toml` lists the files the pack build reads, the county lock adds one geoBoundaries file
//! per country, and `validation.toml` lists files only tests read. An entry with `manual = true`
//! is saved into the cache by hand (its host rejects scripted downloads) and is only verified. An
//! entry with `optional = true` is skipped unless it is in the cache.

use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use toml::{Table, Value};

use crate::sources::{CONFIG_DIR, read_table, workspace_root};

/// The cache directory, relative to the workspace root.
pub const CACHE_DIR: &str = "data/cache";

const USER_AGENT: &str = "ceteris-packs";
const CHUNK: usize = 1 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub source: String,
    pub name: String,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub url: String,
    pub optional: bool,
    pub manual: bool,
}

#[derive(Debug)]
pub enum FetchError {
    /// A manual entry is absent from the cache.
    Manual {
        name: String,
        url: String,
        cache: PathBuf,
    },
    /// The downloaded file does not match the pinned digest or size; it was discarded.
    Mismatch {
        name: String,
        sha256: String,
        bytes: u64,
    },
    Other(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Manual { name, url, cache } => write!(
                f,
                "{name}: the host rejects scripted downloads; save {url} into {} in a browser",
                cache.display()
            ),
            Self::Mismatch {
                name,
                sha256,
                bytes,
            } => write!(f, "{name}: checksum mismatch, got {sha256} ({bytes} bytes)"),
            Self::Other(message) => f.write_str(message),
        }
    }
}

fn io(path: &Path, e: std::io::Error) -> FetchError {
    FetchError::Other(format!("{}: {e}", path.display()))
}

pub fn path_of(name: &str) -> PathBuf {
    workspace_root().join(CACHE_DIR).join(name)
}

fn entry_of(file: &Table) -> Entry {
    let text = |key: &str| file.get(key).and_then(Value::as_str);
    let flag = |key: &str| file.get(key).and_then(Value::as_bool).unwrap_or(false);
    Entry {
        source: text("source").unwrap_or_default().to_owned(),
        name: text("name").unwrap_or_default().to_owned(),
        sha256: text("sha256").map(str::to_owned),
        bytes: file
            .get("bytes")
            .and_then(Value::as_integer)
            .and_then(|b| u64::try_from(b).ok()),
        url: text("url").unwrap_or_default().to_owned(),
        optional: flag("optional"),
        manual: flag("manual"),
    }
}

fn files(table: &Table) -> Vec<Table> {
    table
        .get("file")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| f.as_table().cloned())
        .collect()
}

/// Entries must name a source in `sources.toml`; the error lists the unknown ones as a Python list.
fn known(root: &Path, entries: Vec<Entry>, file: &str) -> Result<Vec<Entry>, String> {
    let manifest = read_table(&root.join(CONFIG_DIR).join("sources.toml"))?;
    let ids: Vec<&str> = manifest
        .get("source")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("id").and_then(Value::as_str))
        .collect();
    let unknown: Vec<String> = entries
        .iter()
        .filter(|e| !ids.contains(&e.source.as_str()))
        .map(|e| format!("'{}'", e.name))
        .collect();
    if unknown.is_empty() {
        Ok(entries)
    } else {
        Err(format!(
            "{file} names sources missing from sources.toml: [{}]",
            unknown.join(", ")
        ))
    }
}

/// Files the pack build reads: `fetch.toml` then the county lock.
pub fn entries(root: &Path) -> Result<Vec<Entry>, String> {
    let config = root.join(CONFIG_DIR);
    let mut found: Vec<Entry> = files(&read_table(&config.join("fetch.toml"))?)
        .iter()
        .map(entry_of)
        .collect();
    let lock = config.join("counties.lock.toml");
    if lock.exists() {
        let table = read_table(&lock)?;
        let countries = table.get("country").and_then(Value::as_array);
        for country in countries.into_iter().flatten().filter_map(Value::as_table) {
            let text = |key: &str| country.get(key).and_then(Value::as_str).unwrap_or_default();
            let mut entry = entry_of(country);
            entry.source = "geoboundaries".into();
            entry.name = format!("geoboundaries/{}-{}.geojson", text("iso"), text("level"));
            found.push(entry);
        }
    }
    known(root, found, "fetch.toml")
}

/// Files only tests and diagnostics read (ADR 0050); never fetched by a pack build.
pub fn validation_entries(root: &Path) -> Result<Vec<Entry>, String> {
    let table = read_table(&root.join(CONFIG_DIR).join("validation.toml"))?;
    known(
        root,
        files(&table).iter().map(entry_of).collect(),
        "validation.toml",
    )
}

/// The digest and size of a file.
pub fn digest(path: &Path) -> std::io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; CHUNK];
    let mut size = 0;
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            return Ok((format!("{:x}", hasher.finalize()), size));
        }
        hasher.update(&buffer[..n]);
        size += n as u64;
    }
}

fn download(entry: &Entry, part: &Path) -> Result<(String, u64), FetchError> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| FetchError::Other(e.to_string()))?;
    let mut response = client
        .get(&entry.url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| FetchError::Other(format!("{}: {e}", entry.url)))?;
    let mut out = File::create(part).map_err(|e| io(part, e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; CHUNK];
    let mut size = 0;
    loop {
        let n = response
            .read(&mut buffer)
            .map_err(|e| FetchError::Other(format!("{}: {e}", entry.url)))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        out.write_all(&buffer[..n]).map_err(|e| io(part, e))?;
        size += n as u64;
    }
    out.flush().map_err(|e| io(part, e))?;
    Ok((format!("{:x}", hasher.finalize()), size))
}

/// Ensures the entry is in `cache` and returns its path. A file already there is returned
/// unverified; a download is checked against the pinned digest and size before it is renamed
/// into place. Nothing is downloaded for an optional or manual entry.
pub fn fetch(entry: &Entry, cache: &Path) -> Result<PathBuf, FetchError> {
    let path = cache.join(&entry.name);
    if path.exists() || entry.optional {
        return Ok(path);
    }
    if entry.manual {
        return Err(FetchError::Manual {
            name: entry.name.clone(),
            url: entry.url.clone(),
            cache: cache.to_owned(),
        });
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    }
    let mut part = path.clone().into_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    eprintln!("fetching {}", entry.url);
    let (sha256, bytes) = download(entry, &part).inspect_err(|_| {
        let _ = fs::remove_file(&part);
    })?;
    if let Some(pinned) = &entry.sha256
        && (pinned != &sha256 || entry.bytes != Some(bytes))
    {
        let _ = fs::remove_file(&part);
        return Err(FetchError::Mismatch {
            name: entry.name.clone(),
            sha256,
            bytes,
        });
    }
    fs::rename(&part, &path).map_err(|e| io(&path, e))?;
    Ok(path)
}

/// `pack fetch [--record]`: fetches and verifies every entry. Returns whether all were pinned and
/// matching; with `record`, unpinned entries print their checksums to stdout instead of failing.
pub fn run(root: &Path, record: bool) -> Result<bool, String> {
    let mut all = entries(root)?;
    all.extend(validation_entries(root)?);
    // The manual-download message names the cache by its absolute path.
    let cache = root
        .canonicalize()
        .unwrap_or(root.to_owned())
        .join(CACHE_DIR);
    let mut bad = 0;
    for entry in &all {
        if entry.optional && !cache.join(&entry.name).exists() {
            continue;
        }
        let path = match fetch(entry, &cache) {
            Ok(path) => path,
            Err(e @ FetchError::Mismatch { .. }) => {
                eprintln!("{e}");
                bad += 1;
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let (sha256, bytes) = digest(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        match &entry.sha256 {
            None if record => println!("{}\nsha256 = \"{sha256}\"\nbytes = {bytes}\n", entry.name),
            None => {
                eprintln!("{}: not pinned (run with --record)", entry.name);
                bad += 1;
            }
            Some(pinned) if (pinned, Some(bytes)) != (&sha256, entry.bytes) => {
                eprintln!(
                    "{}: checksum mismatch, got {sha256} ({bytes} bytes)",
                    entry.name
                );
                bad += 1;
            }
            Some(_) => {}
        }
    }
    Ok(bad == 0)
}
