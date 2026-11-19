//! The first-run setup screen's model (ADR 0215, RFC 0333): a disk check, one row per source,
//! resumable downloads, manual files accepted by hash, and the schema-change rebuild notice.
//! Pure state; the fetch and build back end (#228) feeds `Source::state` and `Setup::free`.

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

/// What the cache holds for a source; mirrors `ceteris_pack::fetch::Status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    /// An interrupted download keeps these bytes; the next fetch resumes from them.
    Partial(u64),
    Verified,
    /// The cached file does not match its pin; it is fetched again.
    Wrong,
}

/// A manual download: the provider's page and the exact file expected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manual {
    pub page: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub name: String,
    pub licence: String,
    pub bytes: u64,
    /// A source outside the prebuilt pack (RFC 0333): the fuller pack's upgrade.
    pub optional: bool,
    pub manual: Option<Manual>,
    pub state: State,
}

impl Source {
    /// Bytes still to download.
    pub fn remaining(&self) -> u64 {
        match self.state {
            State::Verified => 0,
            State::Partial(done) => self.bytes.saturating_sub(done),
            State::Missing | State::Wrong => self.bytes,
        }
    }

    /// One short line for the row.
    pub fn status(&self) -> String {
        match (&self.state, &self.manual) {
            (State::Verified, _) => "ready".into(),
            (State::Partial(done), _) => format!("resumes at {}", size(*done)),
            (State::Wrong, Some(_)) => "wrong file, replace it".into(),
            (State::Wrong, None) => "checksum mismatch, fetched again".into(),
            (State::Missing, Some(_)) => "download by hand".into(),
            (State::Missing, None) => "waiting".into(),
        }
    }
}

/// The pack schema the app needs against the one on disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schema {
    pub needed: u32,
    pub found: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    pub sources: Vec<Source>,
    /// Free bytes where the cache and pack live.
    pub free: u64,
    /// Space the pack build needs on top of the downloads.
    pub build_bytes: u64,
    pub schema: Schema,
    /// Seconds a rebuild from cached sources is expected to take.
    pub rebuild_secs: u64,
}

impl Setup {
    /// Bytes the setup still has to write: downloads left plus the build.
    pub fn needed(&self) -> u64 {
        let downloads: u64 = self
            .sources
            .iter()
            .filter(|s| !s.optional)
            .map(Source::remaining)
            .sum();
        downloads + self.build_bytes
    }

    pub fn disk_ok(&self) -> bool {
        self.free >= self.needed()
    }

    pub fn disk_line(&self) -> String {
        let (need, free) = (size(self.needed()), size(self.free));
        if self.disk_ok() {
            format!("Needs {need}; {free} free.")
        } else {
            format!("Needs {need} but only {free} is free. Free some space to continue.")
        }
    }

    /// A schema mismatch rebuilds from the cached sources, without downloading again.
    pub fn rebuild_notice(&self) -> Option<String> {
        (self.schema.found != Some(self.schema.needed)).then(|| {
            let mins = self.rebuild_secs.div_ceil(60);
            format!(
                "The pack on disk predates this version. It will be rebuilt from the cached \
                 sources, with no new download, in about {mins} min."
            )
        })
    }

    /// The build may start when the disk fits and every required source is verified.
    pub fn can_build(&self) -> bool {
        self.disk_ok()
            && self
                .sources
                .iter()
                .filter(|s| !s.optional)
                .all(|s| s.state == State::Verified)
    }

    /// Accepts a file the person downloaded by hand: it must match a manual source's pin. A wrong
    /// file is refused with the names and sizes still expected; the file is not touched.
    pub fn accept(&mut self, digest: &str, bytes: u64) -> Result<String, String> {
        let hit = self.sources.iter_mut().find(|s| {
            s.state != State::Verified
                && s.bytes == bytes
                && s.manual.as_ref().is_some_and(|m| m.sha256 == digest)
        });
        if let Some(source) = hit {
            source.state = State::Verified;
            return Ok(source.name.clone());
        }
        let mut expected = String::new();
        for s in self
            .sources
            .iter()
            .filter(|s| s.manual.is_some() && s.state != State::Verified)
        {
            let _ = write!(expected, "\n{} ({})", s.name, size(s.bytes));
        }
        Err(format!(
            "Not one of the expected files (got {}). Still expected:{expected}",
            size(bytes)
        ))
    }
}

/// The SHA-256 (lower-case hex) and length of a file.
pub fn digest(path: &Path) -> io::Result<(String, u64)> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let bytes = io::copy(&mut file, &mut hasher)?;
    let hex = hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    Ok((hex, bytes))
}

/// Decimal units, as providers state them.
pub fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "kB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// A clean first run shaped like the real one (RFC 0333): the prebuilt pack's sources, one manual
/// download, one upgrade source. The back end replaces it (#228).
pub fn fixture() -> Setup {
    let source =
        |name: &str, licence: &str, bytes, optional, manual: Option<Manual>, state| Source {
            name: name.into(),
            licence: licence.into(),
            bytes,
            optional,
            manual,
            state,
        };
    Setup {
        sources: vec![
            source(
                "ne_10m_admin_0_countries.zip",
                "public domain",
                4_930_492,
                false,
                None,
                State::Verified,
            ),
            source(
                "ne_10m_railroads.zip",
                "public domain",
                15_116_579,
                false,
                None,
                State::Partial(6_000_000),
            ),
            source(
                "ne_10m_ports.zip",
                "public domain",
                2_000_000,
                false,
                None,
                State::Missing,
            ),
            source(
                "kummu_gdp.nc",
                "CC BY 4.0",
                480_000_000,
                true,
                Some(Manual {
                    page: "https://doi.org/10.5061/dryad.dk1j0".into(),
                    sha256: "0".repeat(64),
                }),
                State::Missing,
            ),
        ],
        free: 50_000_000_000,
        build_bytes: 3_000_000_000,
        schema: Schema {
            needed: 3,
            found: Some(2),
        },
        rebuild_secs: 540,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disk_check_counts_remaining_required_bytes_and_the_build() {
        let mut s = fixture();
        assert_eq!(
            s.needed(),
            (15_116_579 - 6_000_000) + 2_000_000 + 3_000_000_000
        );
        assert!(s.disk_ok());
        s.free = s.needed() - 1;
        assert!(!s.disk_ok());
        assert!(s.disk_line().contains("only"));
    }

    #[test]
    fn build_waits_for_every_required_source() {
        let mut s = fixture();
        assert!(!s.can_build());
        for src in &mut s.sources {
            src.state = State::Verified;
        }
        assert!(s.can_build());
        s.free = 0;
        assert!(!s.can_build());
    }

    #[test]
    fn schema_mismatch_announces_a_rebuild_with_its_duration() {
        let mut s = fixture();
        assert!(s.rebuild_notice().unwrap().contains("about 9 min"));
        s.schema.found = Some(s.schema.needed);
        assert_eq!(s.rebuild_notice(), None);
        s.schema.found = None;
        assert!(s.rebuild_notice().is_some());
    }

    #[test]
    fn manual_file_is_accepted_by_hash_and_size() {
        let mut s = fixture();
        s.sources[3].manual.as_mut().unwrap().sha256 = "ab".repeat(32);
        assert_eq!(
            s.accept(&"ab".repeat(32), 480_000_000),
            Ok("kummu_gdp.nc".into())
        );
        assert_eq!(s.sources[3].state, State::Verified);
    }

    #[test]
    fn wrong_manual_file_is_refused_with_the_expected_name_and_size() {
        let mut s = fixture();
        let err = s.accept(&"cd".repeat(32), 12).unwrap_err();
        assert!(err.contains("kummu_gdp.nc (480.0 MB)"), "{err}");
        assert_eq!(s.sources[3].state, State::Missing);
    }

    #[test]
    fn interrupted_download_resumes_from_the_kept_bytes() {
        let s = &fixture().sources[1];
        assert_eq!(s.remaining(), 15_116_579 - 6_000_000);
        assert_eq!(s.status(), "resumes at 6.0 MB");
    }

    #[test]
    fn digest_matches_a_known_vector() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            digest(&path).unwrap(),
            (
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                3
            )
        );
    }
}
