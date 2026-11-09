//! First-run setup (ADR 0215, RFC 0333): what the cache holds per source, the disk check, the
//! downloads, and the manual files the user drops in.

use std::fs;
use std::io::Write;
use std::path::Path;

use toml::Value;

use crate::fetch::{self, Entry, FetchError, Status};
use crate::sources::{CONFIG_DIR, read_table};

/// Space the pack build needs beyond the downloads. An estimate until measured on the Mac
/// (RFC 0333, open points).
pub const BUILD_BYTES: u64 = 4 << 30;

/// One source file and what the cache holds for it.
pub struct Row {
    pub entry: Entry,
    pub licence: String,
    pub status: Status,
}

impl Row {
    fn present(&self) -> bool {
        matches!(self.status, Status::Verified | Status::Unpinned)
    }
}

fn licences(root: &Path) -> Result<Vec<(String, String)>, String> {
    let manifest = read_table(&root.join(CONFIG_DIR).join("sources.toml"))?;
    Ok(manifest
        .get("source")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let text = |k: &str| s.get(k).and_then(Value::as_str);
            Some((text("id")?.to_owned(), text("licence")?.to_owned()))
        })
        .collect())
}

/// One row per required pack source (optional entries are left out unless cached).
pub fn plan(root: &Path, cache: &Path) -> Result<Vec<Row>, String> {
    let licences = licences(root)?;
    let mut rows = Vec::new();
    for entry in fetch::entries(root)? {
        if entry.optional && !cache.join(&entry.name).exists() {
            continue;
        }
        let status = fetch::status(&entry, cache).map_err(|e| format!("{}: {e}", entry.name))?;
        let licence = licences
            .iter()
            .find(|(id, _)| *id == entry.source)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        rows.push(Row {
            entry,
            licence,
            status,
        });
    }
    Ok(rows)
}

/// Bytes still to download, plus the build space.
pub fn bytes_needed(rows: &[Row], build_bytes: u64) -> u64 {
    let download: u64 = rows
        .iter()
        .filter(|r| !r.present())
        .map(|r| {
            let kept = if let Status::Partial(n) = r.status {
                n
            } else {
                0
            };
            r.entry.bytes.unwrap_or(0).saturating_sub(kept)
        })
        .sum();
    download + build_bytes
}

/// Free bytes on the volume holding `path`, which must exist.
pub fn free_bytes(path: &Path) -> Result<u64, String> {
    let stat = rustix::fs::statvfs(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(stat.f_bavail * stat.f_frsize)
}

fn mib(bytes: u64) -> u64 {
    bytes.div_ceil(1 << 20)
}

fn state(status: &Status) -> String {
    match status {
        Status::Verified => "ok".into(),
        Status::Unpinned => "present, not pinned".into(),
        Status::Wrong { sha256, bytes } => format!("WRONG FILE ({bytes} bytes, sha256 {sha256})"),
        Status::Partial(n) => format!("partial, {} MiB kept", mib(*n)),
        Status::Missing => "missing".into(),
    }
}

/// Runs setup: lists the sources, checks the disk, downloads what is missing (upstream, then the
/// mirror), and accepts matching manual files found in `from`. Returns whether every required
/// file is now in the cache and verified. Safe to repeat: finished files are kept, interrupted
/// downloads resume.
pub fn run(
    root: &Path,
    from: Option<&Path>,
    build_bytes: u64,
    out: &mut impl Write,
) -> Result<bool, String> {
    let w = |e: std::io::Error| e.to_string();
    let cache = root.join(fetch::CACHE_DIR);
    fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let cache = cache.canonicalize().map_err(|e| e.to_string())?;
    let mut rows = plan(root, &cache)?;
    for r in &rows {
        let size = r
            .entry
            .bytes
            .map_or("?".into(), |b| format!("{} MiB", mib(b)));
        let kind = if r.entry.manual { "manual" } else { "fetch" };
        writeln!(
            out,
            "{:<48} {:>9}  {:<6} {:<28} {}",
            r.entry.name,
            size,
            kind,
            r.licence,
            state(&r.status)
        )
        .map_err(w)?;
    }
    let (needed, free) = (bytes_needed(&rows, build_bytes), free_bytes(&cache)?);
    writeln!(
        out,
        "disk: need about {} MiB, {} MiB free",
        mib(needed),
        mib(free)
    )
    .map_err(w)?;
    if free < needed {
        return Err(format!(
            "not enough disk space in {}: need about {} MiB, {} MiB free",
            cache.display(),
            mib(needed),
            mib(free)
        ));
    }
    for r in &mut rows {
        if r.present() {
            continue;
        }
        let path = from.map(|d| d.join(&r.entry.name)).filter(|p| p.exists());
        let result = match (&path, r.entry.manual) {
            (Some(p), true) => fetch::accept(&r.entry, p, &cache).map(|_| ()),
            (None, true) => Err(manual_hint(&r.entry, &cache)),
            (_, false) => {
                if let Status::Wrong { .. } = r.status {
                    fs::remove_file(cache.join(&r.entry.name)).map_err(|e| e.to_string())?;
                }
                fetch::fetch(&r.entry, &cache)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        };
        match result {
            Ok(()) => {
                r.status = fetch::status(&r.entry, &cache).map_err(|e| e.to_string())?;
                writeln!(out, "{}: {}", r.entry.name, state(&r.status)).map_err(w)?;
            }
            Err(message) => writeln!(out, "{message}").map_err(w)?,
        }
    }
    let todo: Vec<&Row> = rows.iter().filter(|r| !r.present()).collect();
    writeln!(
        out,
        "{} of {} files ready",
        rows.len() - todo.len(),
        rows.len()
    )
    .map_err(w)?;
    if !todo.is_empty() {
        writeln!(
            out,
            "rerun `ceteris setup` to retry; manual files can go in {} or a --from folder",
            cache.display()
        )
        .map_err(w)?;
    }
    Ok(todo.is_empty())
}

fn manual_hint(entry: &Entry, cache: &Path) -> String {
    FetchError::Manual {
        name: entry.name.clone(),
        url: entry.url.clone(),
        cache: cache.to_owned(),
        bytes: entry.bytes,
    }
    .to_string()
}
