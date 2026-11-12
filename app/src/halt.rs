//! The repro bundle of a halt and the report-a-problem flow (ADR 0214). The bundle has the
//! format of `ceteris run`'s, so `ceteris replay` reproduces it. Nothing is uploaded: the person
//! attaches the bundle to a GitHub issue.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sim::Halt;

const BUNDLE_FILE: &str = "bundle.txt";
const ISSUES_URL: &str = "https://github.com/animal-spirits/ceteris/issues/new";
/// Keeps the URL well under the length browsers and GitHub accept.
const MAX_DETAIL_CHARS: usize = 600;

/// Where bundles go: the platform's log folder for the app.
pub fn bundle_root() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let state = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from);
    match (cfg!(target_os = "macos"), home, state) {
        (true, Some(h), _) => h.join("Library/Logs/Ceteris/bundles"),
        (_, _, Some(s)) => s.join("ceteris/bundles"),
        (_, Some(h), _) => h.join(".local/state/ceteris/bundles"),
        _ => std::env::temp_dir().join("ceteris-bundles"),
    }
}

/// Writes `root/halt-<unix seconds>-<tick>/bundle.txt` and returns the folder.
pub fn write_bundle(root: &Path, pack: &Path, halt: &Halt) -> Result<PathBuf, String> {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let dir = root.join(format!("halt-{secs}-{}", halt.tick));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = format!(
        "ceteris-bundle 1\nengine={}\narg=--pack\narg={}\narg=--ticks\narg={}\n\
         halt-tick={}\nhalt-invariant={}\nhalt-detail={}\n",
        env!("CARGO_PKG_VERSION"),
        pack.display(),
        halt.tick,
        halt.tick,
        halt.kind,
        halt.detail.replace('\n', " "),
    );
    let file = dir.join(BUNDLE_FILE);
    std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(dir)
}

/// A prefilled new-issue URL. It carries the halt record, engine version and platform, no paths.
pub fn issue_url(halt: &Halt) -> String {
    let title = format!("Halt at tick {}: {}", halt.tick, halt.kind);
    let body = format!(
        "Halt record\n\n- tick: {}\n- kind: {}\n- detail: {}\n- engine: {}\n- platform: {}-{}\n\n\
         Attach `bundle.txt` from the bundle folder.\n",
        halt.tick,
        halt.kind,
        halt.detail
            .replace('\n', " ")
            .chars()
            .take(MAX_DETAIL_CHARS)
            .collect::<String>(),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    format!(
        "{ISSUES_URL}?title={}&body={}",
        percent_encode(&title),
        percent_encode(&body)
    )
}

fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(b).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Opens a folder or URL with the platform's opener.
pub fn open(target: &str) -> Result<(), String> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(opener)
        .arg(target)
        .spawn()
        .map(drop)
        .map_err(|e| format!("{opener}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn halt() -> Halt {
        Halt {
            tick: 7,
            kind: "injected".into(),
            detail: "a b\nc".into(),
        }
    }

    #[test]
    fn bundle_has_the_cli_format() {
        let root = tempfile::tempdir().unwrap();
        let dir = write_bundle(root.path(), Path::new("/p"), &halt()).unwrap();
        let text = std::fs::read_to_string(dir.join(BUNDLE_FILE)).unwrap();
        assert!(text.starts_with("ceteris-bundle 1\nengine="));
        assert!(text.contains("arg=--pack\narg=/p\narg=--ticks\narg=7\n"));
        assert!(text.ends_with("halt-tick=7\nhalt-invariant=injected\nhalt-detail=a b c\n"));
    }

    #[test]
    fn issue_url_is_encoded_and_has_no_path() {
        let url = issue_url(&halt());
        assert!(url.starts_with("https://github.com/animal-spirits/ceteris/issues/new?title=Halt%20at%20tick%207%3A%20injected&body="));
        assert!(!url.contains(' ') && !url.contains('\n'));
    }

    #[test]
    fn issue_url_encodes_unicode_and_reserved_and_caps_length() {
        let mut h = halt();
        h.detail = format!("é&=#?+/ {}", "x".repeat(5000));
        let url = issue_url(&h);
        assert!(url.contains("%C3%A9%26%3D%23%3F%2B%2F%20"));
        assert!(url.is_ascii() && url.len() < 2500);
        let query = url.split_once('?').unwrap().1;
        assert_eq!(query.matches('&').count(), 1);
    }

    #[test]
    fn issue_url_has_version_and_platform() {
        let url = issue_url(&halt());
        assert!(url.contains(&percent_encode(&format!(
            "- engine: {}\n- platform: {}-{}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH
        ))));
    }
}
