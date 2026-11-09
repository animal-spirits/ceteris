//! Fetching against a local HTTP server: download, verification, and the entries that are never
//! downloaded; and the repository's fetch configuration: which files are manual, optional or
//! validation-only.

use std::collections::BTreeSet;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use ceteris_pack::fetch::{self, Entry, FetchError};
use ceteris_pack::sources::workspace_root;
use sha2::{Digest, Sha256};

const BODY: &[u8] = b"cached bytes for the fetch test";

/// Serves `BODY` at `/file` and 404 elsewhere; returns the base URL and the User-Agent of each
/// request received.
fn serve() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                request.push(byte[0]);
            }
            let text = String::from_utf8_lossy(&request).into_owned();
            let agent = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("user-agent: ")
                        .map(str::to_owned)
                })
                .unwrap_or_default();
            log.lock().unwrap().push(agent);
            let head = if text.starts_with("GET /file ") {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    BODY.len()
                )
            } else {
                "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into()
            };
            let _ = stream.write_all(head.as_bytes());
            if text.starts_with("GET /file ") {
                let _ = stream.write_all(BODY);
            }
        }
    });
    (base, seen)
}

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn entry(url: &str) -> Entry {
    Entry {
        source: "test".into(),
        name: "sub/file.bin".into(),
        sha256: Some(hex(BODY)),
        bytes: Some(BODY.len() as u64),
        url: format!("{url}/file"),
        mirror: None,
        optional: false,
        manual: false,
    }
}

fn leftovers(cache: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(cache.join("sub"))
        .map(|d| {
            d.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn download_verifies_and_renames() {
    let (url, seen) = serve();
    let cache = tempfile::tempdir().unwrap();
    let path = fetch::fetch(&entry(&url), cache.path()).unwrap();
    assert_eq!(path, cache.path().join("sub/file.bin"));
    assert_eq!(fs::read(&path).unwrap(), BODY);
    assert_eq!(leftovers(cache.path()), ["file.bin"]);
    assert_eq!(*seen.lock().unwrap(), ["ceteris-packs"]);
}

#[test]
fn unpinned_download_is_kept() {
    let (url, _) = serve();
    let cache = tempfile::tempdir().unwrap();
    let unpinned = Entry {
        sha256: None,
        bytes: None,
        ..entry(&url)
    };
    fetch::fetch(&unpinned, cache.path()).unwrap();
    assert_eq!(leftovers(cache.path()), ["file.bin"]);
}

#[test]
fn digest_mismatch_discards_the_download() {
    let (url, _) = serve();
    let cache = tempfile::tempdir().unwrap();
    let wrong = Entry {
        sha256: Some(hex(b"other")),
        ..entry(&url)
    };
    let Err(FetchError::Mismatch { sha256, bytes, .. }) = fetch::fetch(&wrong, cache.path()) else {
        panic!("expected a mismatch");
    };
    assert_eq!((sha256, bytes), (hex(BODY), BODY.len() as u64));
    assert!(leftovers(cache.path()).is_empty());
}

#[test]
fn size_mismatch_discards_the_download() {
    let (url, _) = serve();
    let cache = tempfile::tempdir().unwrap();
    let wrong = Entry {
        bytes: Some(1),
        ..entry(&url)
    };
    assert!(matches!(
        fetch::fetch(&wrong, cache.path()),
        Err(FetchError::Mismatch { .. })
    ));
    assert!(leftovers(cache.path()).is_empty());
}

#[test]
fn http_error_leaves_nothing_behind() {
    let (url, _) = serve();
    let cache = tempfile::tempdir().unwrap();
    let missing = Entry {
        url: format!("{url}/absent"),
        ..entry(&url)
    };
    assert!(matches!(
        fetch::fetch(&missing, cache.path()),
        Err(FetchError::Other(_))
    ));
    assert!(leftovers(cache.path()).is_empty());
}

#[test]
fn present_file_is_not_downloaded() {
    let (url, seen) = serve();
    let cache = tempfile::tempdir().unwrap();
    fs::create_dir_all(cache.path().join("sub")).unwrap();
    fs::write(cache.path().join("sub/file.bin"), b"anything").unwrap();
    fetch::fetch(&entry(&url), cache.path()).unwrap();
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn manual_entry_is_never_fetched() {
    let (url, seen) = serve();
    let cache = tempfile::tempdir().unwrap();
    let manual = Entry {
        manual: true,
        ..entry(&url)
    };
    let error = fetch::fetch(&manual, cache.path()).unwrap_err();
    assert!(matches!(error, FetchError::Manual { .. }));
    assert!(
        error
            .to_string()
            .starts_with("sub/file.bin: the host rejects scripted downloads; save ")
    );
    assert!(seen.lock().unwrap().is_empty());
    assert!(leftovers(cache.path()).is_empty());

    fs::create_dir_all(cache.path().join("sub")).unwrap();
    fs::write(cache.path().join("sub/file.bin"), BODY).unwrap();
    let path = fetch::fetch(&manual, cache.path()).unwrap();
    assert_eq!(path, cache.path().join("sub/file.bin"));
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn missing_optional_source_is_tolerated() {
    let (url, seen) = serve();
    let cache = tempfile::tempdir().unwrap();
    let optional = Entry {
        optional: true,
        manual: true,
        ..entry(&url)
    };
    let path = fetch::fetch(&optional, cache.path()).unwrap();
    assert!(!path.exists());
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn entries_join_fetch_toml_and_the_county_lock() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join(ceteris_pack::sources::CONFIG_DIR);
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("sources.toml"),
        "[[source]]\nid = \"a\"\n[[source]]\nid = \"geoboundaries\"\n",
    )
    .unwrap();
    fs::write(
        config.join("fetch.toml"),
        "[[file]]\nsource = \"a\"\nname = \"x.zip\"\nurl = \"u\"\nmanual = true\nsha256 = \"s\"\nbytes = 3\n",
    )
    .unwrap();
    fs::write(
        config.join("counties.lock.toml"),
        "[[country]]\niso = \"AFG\"\nlevel = \"ADM2\"\nurl = \"v\"\nsha256 = \"t\"\nbytes = 4\n",
    )
    .unwrap();
    let found = fetch::entries(root.path()).unwrap();
    let names: Vec<&str> = found.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["x.zip", "geoboundaries/AFG-ADM2.geojson"]);
    assert!(found[0].manual && !found[0].optional);
    assert_eq!(found[1].bytes, Some(4));
}

#[test]
fn unknown_source_is_named() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join(ceteris_pack::sources::CONFIG_DIR);
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("sources.toml"), "[[source]]\nid = \"a\"\n").unwrap();
    fs::write(
        config.join("fetch.toml"),
        "[[file]]\nsource = \"b\"\nname = \"x\"\nurl = \"u\"\n",
    )
    .unwrap();
    assert_eq!(
        fetch::entries(root.path()).unwrap_err(),
        "fetch.toml names sources missing from sources.toml: ['x']"
    );
}

#[test]
fn derived_manual_entry_is_never_downloaded() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join(ceteris_pack::sources::CONFIG_DIR);
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("sources.toml"), "[[source]]\nid = \"a\"\n").unwrap();
    fs::write(
        config.join("fetch.toml"),
        "[[file]]\nsource = \"a\"\nname = \"band.tif\"\nmanual = true\nderived_from = \"all.nc\"\ncommand = \"convert\"\nsha256 = \"s\"\nbytes = 3\nurl = \"http://127.0.0.1:1/band.tif\"\n",
    )
    .unwrap();
    let found = fetch::entries(root.path()).unwrap();
    assert!(found[0].manual);
    let cache = tempfile::tempdir().unwrap();
    let error = fetch::fetch(&found[0], cache.path()).unwrap_err();
    assert!(matches!(error, FetchError::Manual { .. }));
    assert!(!cache.path().join("band.tif").exists());
    assert!(!cache.path().join("band.tif.part").exists());
}

fn icio_entries() -> Vec<Entry> {
    fetch::entries(&workspace_root())
        .unwrap()
        .into_iter()
        .filter(|e| e.source == "oecd-icio")
        .collect()
}

#[test]
fn icio_packages_are_manual_downloads() {
    let icio = icio_entries();
    assert_eq!(icio.len(), 10);
    assert!(
        icio.iter()
            .all(|e| e.manual && e.sha256.as_deref().is_some_and(|s| !s.is_empty()))
    );
}

#[test]
fn only_the_2000_icio_package_is_required() {
    let required: Vec<String> = icio_entries()
        .into_iter()
        .filter(|e| !e.optional)
        .map(|e| e.name)
        .collect();
    assert_eq!(required, ["1995-2000_SML.zip"]);
}

#[test]
fn validation_files_are_fetched_but_not_pack_sources() {
    let root = workspace_root();
    let pack: BTreeSet<String> = fetch::entries(&root)
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    let validation = fetch::validation_entries(&root).unwrap();
    assert!(
        validation
            .iter()
            .any(|e| e.name == "AdvancedGuideFiles.zip")
    );
    assert!(validation.iter().all(|e| !pack.contains(&e.name)));
    assert!(
        validation
            .iter()
            .all(|e| e.sha256.as_deref().is_some_and(|s| !s.is_empty()))
    );
}

#[test]
fn no_pack_code_names_a_validation_file() {
    let names: Vec<String> = fetch::validation_entries(&workspace_root())
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    let mut todo = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(dir) = todo.pop() {
        for item in fs::read_dir(&dir).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                todo.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let text = fs::read_to_string(&path).unwrap();
                let named: Vec<&String> = names.iter().filter(|n| text.contains(*n)).collect();
                assert!(named.is_empty(), "{}: {named:?}", path.display());
            }
        }
    }
}
