//! `pack://` serves the world pack's files to the webview from disk, with HTTP Range
//! support (PMTiles reads byte ranges), so the pack is never copied into the app bundle
//! (ADR 0010).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tauri::http::{Request, Response, StatusCode, header};

/// The first `bytes=` range of a Range header, clamped to the file: (start, end inclusive).
pub fn parse_range(spec: &str, len: u64) -> Option<(u64, u64)> {
    let (a, b) = spec
        .strip_prefix("bytes=")?
        .split(',')
        .next()?
        .split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let (start, end) = match (a.is_empty(), b.is_empty()) {
        (false, _) => {
            let start: u64 = a.parse().ok()?;
            let end = if b.is_empty() {
                len.checked_sub(1)?
            } else {
                b.parse::<u64>().ok()?.min(len.checked_sub(1)?)
            };
            (start, end)
        }
        (true, false) => {
            let suffix: u64 = b.parse().ok()?;
            (len.saturating_sub(suffix), len.checked_sub(1)?)
        }
        (true, true) => return None,
    };
    (start <= end).then_some((start, end))
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The file a request path names, if it lies inside the pack directory.
pub fn resolve(root: &Path, uri_path: &str) -> Option<PathBuf> {
    let relative = percent_decode(uri_path.trim_start_matches('/'))?;
    let path = root.join(relative).canonicalize().ok()?;
    (path.starts_with(root.canonicalize().ok()?) && path.is_file()).then_some(path)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("json") => "application/json",
        Some("pmtiles") => "application/vnd.pmtiles",
        _ => "application/octet-stream",
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    with_cors(Response::builder().status(code))
        .body(Vec::new())
        .unwrap()
}

// The page's origin (the dev server, or tauri://) differs from pack://, so responses are CORS-enabled.
fn with_cors(b: tauri::http::response::Builder) -> tauri::http::response::Builder {
    b.header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "Range")
        .header(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            "Content-Range, Content-Length, Accept-Ranges",
        )
}

pub fn serve(root: &Path, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() == "OPTIONS" {
        return status(StatusCode::NO_CONTENT);
    }
    let Some(path) = resolve(root, request.uri().path()) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Ok(mut file) = File::open(&path) else {
        return status(StatusCode::NOT_FOUND);
    };
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok());
    let (code, start, end) = match range {
        None => (StatusCode::OK, 0, len.saturating_sub(1)),
        Some(spec) => match parse_range(spec, len) {
            Some((s, e)) => (StatusCode::PARTIAL_CONTENT, s, e),
            None => return status(StatusCode::RANGE_NOT_SATISFIABLE),
        },
    };
    let mut body = vec![
        0;
        if len == 0 {
            0
        } else {
            (end - start + 1) as usize
        }
    ];
    if file
        .seek(SeekFrom::Start(start))
        .and_then(|_| file.read_exact(&mut body))
        .is_err()
    {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let mut b = with_cors(Response::builder().status(code))
        .header(header::CONTENT_TYPE, content_type(&path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, body.len());
    if code == StatusCode::PARTIAL_CONTENT {
        b = b.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    b.body(body).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_follow_rfc_9110() {
        assert_eq!(parse_range("bytes=0-126", 1000), Some((0, 126)));
        assert_eq!(parse_range("bytes=990-", 1000), Some((990, 999)));
        assert_eq!(parse_range("bytes=-10", 1000), Some((990, 999)));
        assert_eq!(parse_range("bytes=500-5000", 1000), Some((500, 999)));
        assert_eq!(parse_range("bytes=1000-", 1000), None);
        assert_eq!(parse_range("bytes=-", 1000), None);
        assert_eq!(parse_range("items=0-1", 1000), None);
    }

    #[test]
    fn paths_cannot_leave_the_pack() {
        let root = std::env::temp_dir().join("ceteris-pack-protocol-test");
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/a.json"), b"{}").unwrap();
        assert!(resolve(&root, "/web/a.json").is_some());
        assert!(resolve(&root, "/web%2Fa.json").is_some());
        assert!(resolve(&root, "/web/../../etc/passwd").is_none());
        assert!(resolve(&root, "/web/missing.json").is_none());
    }

    #[test]
    fn serves_partial_content() {
        let root = std::env::temp_dir().join("ceteris-pack-protocol-serve");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("f.bin"), (0u8..100).collect::<Vec<_>>()).unwrap();
        let req = Request::builder()
            .uri("pack://localhost/f.bin")
            .header(header::RANGE, "bytes=10-19")
            .body(Vec::new())
            .unwrap();
        let res = serve(&root, &req);
        assert_eq!(res.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(res.body(), &(10u8..20).collect::<Vec<_>>());
        assert_eq!(res.headers()[header::CONTENT_RANGE], "bytes 10-19/100");
    }
}
