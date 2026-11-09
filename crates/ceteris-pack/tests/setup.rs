//! The first-run path against a local HTTP server: mirror fallback, resuming, manual files and
//! the setup run with the upstream host unreachable (ADR 0215).

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use ceteris_pack::fetch::{self, Entry, Status};
use ceteris_pack::setup;
use sha2::{Digest, Sha256};

const BODY: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
const DEAD: &str = "http://127.0.0.1:9";

fn hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct Server {
    base: String,
    /// The Range header of each request to `/flaky`, in order.
    ranges: Arc<Mutex<Vec<Option<String>>>>,
}

/// `/file` serves BODY; `/wrong` serves other bytes; `/flaky` cuts the first full response short
/// and honours `Range` afterwards.
fn serve() -> Server {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let ranges = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&ranges);
    let flaky_hits = AtomicUsize::new(0);
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut request = Vec::new();
            let mut byte = [0u8; 1];
            while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).unwrap_or(0) == 1 {
                request.push(byte[0]);
            }
            let text = String::from_utf8_lossy(&request).into_owned();
            let range = text.lines().find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("range: bytes=")
                    .and_then(|r| r.strip_suffix('-').map(str::to_owned))
            });
            let n = BODY.len();
            let response: Vec<u8> = if text.starts_with("GET /file ") {
                [
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n")
                        .as_bytes(),
                    BODY,
                ]
                .concat()
            } else if text.starts_with("GET /wrong ") {
                b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".to_vec()
            } else if text.starts_with("GET /flaky ") {
                log.lock().unwrap().push(range.clone());
                flaky_hits.fetch_add(1, Ordering::SeqCst);
                match range.and_then(|r| r.parse::<usize>().ok()) {
                    Some(from) => [
                        format!(
                            "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            n - from
                        )
                        .as_bytes(),
                        &BODY[from..],
                    ]
                    .concat(),
                    None => [
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {n}\r\nConnection: close\r\n\r\n")
                            .as_bytes(),
                        &BODY[..10],
                    ]
                    .concat(),
                }
            } else {
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
            };
            let _ = stream.write_all(&response);
        }
    });
    Server { base, ranges }
}

fn entry(url: String, mirror: Option<String>) -> Entry {
    Entry {
        source: "a".into(),
        name: "f.bin".into(),
        sha256: Some(hex(BODY)),
        bytes: Some(BODY.len() as u64),
        url,
        mirror,
        optional: false,
        manual: false,
    }
}

#[test]
fn mirror_serves_when_upstream_is_missing_or_wrong() {
    let server = serve();
    for upstream in [format!("{DEAD}/file"), format!("{}/wrong", server.base)] {
        let cache = tempfile::tempdir().unwrap();
        let e = entry(upstream, Some(format!("{}/file", server.base)));
        let path = fetch::fetch(&e, cache.path()).unwrap();
        assert_eq!(fs::read(path).unwrap(), BODY);
    }
}

#[test]
fn both_hosts_failing_names_both() {
    let server = serve();
    let cache = tempfile::tempdir().unwrap();
    let e = entry(
        format!("{DEAD}/file"),
        Some(format!("{}/wrong", server.base)),
    );
    let message = fetch::fetch(&e, cache.path()).unwrap_err().to_string();
    assert!(
        message.contains("; mirror: f.bin: checksum mismatch"),
        "{message}"
    );
    assert!(!cache.path().join("f.bin.part").exists());
}

#[test]
fn interrupted_download_resumes_with_a_range_request() {
    let server = serve();
    let cache = tempfile::tempdir().unwrap();
    let e = entry(format!("{}/flaky", server.base), None);
    assert!(fetch::fetch(&e, cache.path()).is_err());
    assert_eq!(
        fetch::status(&e, cache.path()).unwrap(),
        Status::Partial(10)
    );
    let path = fetch::fetch(&e, cache.path()).unwrap();
    assert_eq!(fs::read(path).unwrap(), BODY);
    assert_eq!(*server.ranges.lock().unwrap(), [None, Some("10".into())]);
}

#[test]
fn wrong_manual_file_is_refused_with_the_expected_name_and_size() {
    let cache = tempfile::tempdir().unwrap();
    let drop = tempfile::tempdir().unwrap();
    let wrong = drop.path().join("f.bin");
    fs::write(&wrong, b"not it").unwrap();
    let e = entry("https://example.org/f".into(), None);
    let message = fetch::accept(&e, &wrong, cache.path()).unwrap_err();
    assert!(message.contains("expected f.bin (36 bytes)"), "{message}");
    assert!(!cache.path().join("f.bin").exists());
    fs::write(&wrong, BODY).unwrap();
    fetch::accept(&e, &wrong, cache.path()).unwrap();
    assert_eq!(fetch::status(&e, cache.path()).unwrap(), Status::Verified);
}

fn root_with(fetch_toml: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join(ceteris_pack::sources::CONFIG_DIR);
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("sources.toml"),
        "[[source]]\nid = \"a\"\nlicence = \"CC0-1.0\"\n",
    )
    .unwrap();
    fs::write(config.join("validation.toml"), "").unwrap();
    fs::write(config.join("fetch.toml"), fetch_toml).unwrap();
    root
}

#[test]
fn setup_completes_from_the_archive_and_the_drop_folder() {
    let server = serve();
    let (sha, n) = (hex(BODY), BODY.len());
    let toml = format!(
        "[[file]]\nsource = \"a\"\nname = \"auto.bin\"\nsha256 = \"{sha}\"\nbytes = {n}\nurl = \"{DEAD}/file\"\nmirror = \"{}/file\"\n\n\
         [[file]]\nsource = \"a\"\nname = \"hand.bin\"\nsha256 = \"{sha}\"\nbytes = {n}\nurl = \"https://example.org/hand\"\nmanual = true\n",
        server.base
    );
    let root = root_with(&toml);
    let drop = tempfile::tempdir().unwrap();
    let mut out = Vec::new();

    // Nothing dropped yet: the automatic file arrives, the manual one is asked for.
    assert!(!setup::run(root.path(), Some(drop.path()), 0, &mut out).unwrap());
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("hand.bin: the host rejects scripted downloads"),
        "{text}"
    );
    assert!(text.contains("(36 bytes) in a browser"), "{text}");
    assert!(text.contains("1 of 2 files ready"), "{text}");

    fs::write(drop.path().join("hand.bin"), BODY).unwrap();
    let mut out = Vec::new();
    assert!(setup::run(root.path(), Some(drop.path()), 0, &mut out).unwrap());
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("2 of 2 files ready")
    );
}

#[test]
fn setup_stops_when_the_disk_is_too_small() {
    let root = root_with("");
    let error = setup::run(root.path(), None, u64::MAX / 2, &mut Vec::new()).unwrap_err();
    assert!(error.starts_with("not enough disk space in "), "{error}");
}
