//! Content-addressed snapshot store (Section 14.3; ADRs 0002, 0211).
//!
//! A snapshot is a manifest of chunk hashes over the canonical bytes of ADR 0002, cut at
//! fixed offsets and stored as zstd files named by the BLAKE3 hash of their raw bytes. The
//! state hash stays over the canonical bytes, so moving to chunks re-blesses nothing. The
//! store is a cache keyed by engine: another engine's directory is refused, never misread.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use ceteris_core::snapshot::{Hash, hex, state_hash};

pub const CHUNK_BYTES: usize = 64 * 1024;
const ZSTD_LEVEL: i32 = 1;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// The directory was written by another engine version ("cache from another version").
    OtherEngine {
        found: String,
        wanted: String,
    },
    MissingChunk(String),
    Corrupt(String),
    BadManifest(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "store: {e}"),
            Self::OtherEngine { found, wanted } => {
                write!(
                    f,
                    "store: cache from another version ({found}, this build is {wanted})"
                )
            }
            Self::MissingChunk(h) => write!(f, "store: missing chunk {h}"),
            Self::Corrupt(m) => write!(f, "store: corrupt: {m}"),
            Self::BadManifest(m) => write!(f, "store: bad manifest: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub state_hash: Hash,
    pub len: u64,
    pub chunks: Vec<Hash>,
}

impl Manifest {
    pub fn to_text(&self) -> String {
        let mut s = format!("{}\n{}\n", hex(&self.state_hash), self.len);
        for c in &self.chunks {
            s.push_str(&hex(c));
            s.push('\n');
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Self, StoreError> {
        let bad = |m: &str| StoreError::BadManifest(m.to_string());
        let mut lines = text.lines();
        let state_hash = unhex(lines.next().ok_or_else(|| bad("empty"))?)?;
        let len = lines
            .next()
            .and_then(|l| l.parse().ok())
            .ok_or_else(|| bad("length"))?;
        let chunks = lines.map(unhex).collect::<Result<_, _>>()?;
        Ok(Self {
            state_hash,
            len,
            chunks,
        })
    }
}

fn unhex(s: &str) -> Result<Hash, StoreError> {
    let bad = || StoreError::BadManifest(format!("hash `{s}`"));
    if s.len() != 64 {
        return Err(bad());
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(2 * i..2 * i + 2).ok_or_else(bad)?, 16).map_err(|_| bad())?;
    }
    Ok(out)
}

pub struct ChunkStore {
    dir: PathBuf,
}

impl ChunkStore {
    /// Opens or creates the store for `engine`.
    pub fn open(dir: impl Into<PathBuf>, engine: &str) -> Result<Self, StoreError> {
        let dir = dir.into();
        fs::create_dir_all(dir.join("chunks"))?;
        let stamp = dir.join("ENGINE");
        match fs::read_to_string(&stamp) {
            Ok(found) if found != engine => {
                return Err(StoreError::OtherEngine {
                    found,
                    wanted: engine.to_string(),
                });
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => fs::write(&stamp, engine)?,
            Err(e) => return Err(e.into()),
        }
        Ok(Self { dir })
    }

    fn chunk_path(&self, h: &Hash) -> PathBuf {
        let name = hex(h);
        self.dir.join("chunks").join(&name[..2]).join(name)
    }

    /// Writes each chunk not already present and returns the manifest.
    pub fn put(&self, canonical: &[u8]) -> Result<Manifest, StoreError> {
        let mut chunks = Vec::new();
        for raw in canonical.chunks(CHUNK_BYTES) {
            let h = *blake3::hash(raw).as_bytes();
            let path = self.chunk_path(&h);
            if !path.exists() {
                fs::create_dir_all(path.parent().expect("chunk path has a parent"))?;
                let packed = zstd::encode_all(raw, ZSTD_LEVEL)?;
                let tmp = path.with_extension("tmp");
                fs::write(&tmp, packed)?;
                fs::rename(&tmp, &path)?;
            }
            chunks.push(h);
        }
        Ok(Manifest {
            state_hash: state_hash(canonical),
            len: canonical.len() as u64,
            chunks,
        })
    }

    /// Rebuilds the canonical bytes and checks them against the manifest's state hash.
    pub fn get(&self, m: &Manifest) -> Result<Vec<u8>, StoreError> {
        let mut out = Vec::with_capacity(usize::try_from(m.len).unwrap_or(0));
        for h in &m.chunks {
            let packed = fs::read(self.chunk_path(h)).map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => StoreError::MissingChunk(hex(h)),
                _ => e.into(),
            })?;
            let raw = zstd::decode_all(packed.as_slice())?;
            if blake3::hash(&raw).as_bytes() != h {
                return Err(StoreError::Corrupt(format!("chunk {}", hex(h))));
            }
            out.extend_from_slice(&raw);
        }
        if out.len() as u64 != m.len || state_hash(&out) != m.state_hash {
            return Err(StoreError::Corrupt("state hash mismatch".into()));
        }
        Ok(out)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn chunk_count(&self) -> usize {
        fs::read_dir(self.dir.join("chunks"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|d| fs::read_dir(d.path()).map_or(0, |r| r.flatten().count()))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ceteris_core::rng::Seed;
    use ceteris_core::snapshot::{canonical, decode_canonical};
    use ceteris_core::toy::Toy;

    const ENGINE: &str = "test+1";

    fn store(dir: &tempfile::TempDir) -> ChunkStore {
        ChunkStore::open(dir.path().join("cache"), ENGINE).unwrap()
    }

    fn big_toy() -> Toy {
        Toy::new(6000)
    }

    #[test]
    fn chunk_store_restore_equals_canonical() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let bytes = canonical(0, &big_toy());
        assert!(bytes.len() > 3 * CHUNK_BYTES);
        let m = s.put(&bytes).unwrap();
        assert_eq!(m.state_hash, state_hash(&bytes));
        assert_eq!(s.get(&m).unwrap(), bytes);
        let m2 = Manifest::from_text(&m.to_text()).unwrap();
        assert_eq!(m, m2);
        let (tick, _toy): (u32, Toy) = decode_canonical(&s.get(&m2).unwrap()).unwrap();
        assert_eq!(tick, 0);
    }

    #[test]
    fn unchanged_tables_share_chunks_across_branches() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let parent = big_toy();
        let mut child = big_toy();
        child.price[0] *= 1.25;
        let mp = s.put(&canonical(0, &parent)).unwrap();
        let before = s.chunk_count();
        let mc = s.put(&canonical(0, &child)).unwrap();
        assert_ne!(mp.state_hash, mc.state_hash);
        let added = s.chunk_count() - before;
        assert!(
            added >= 1 && added * 4 <= mc.chunks.len(),
            "added {added} of {}",
            mc.chunks.len()
        );
    }

    #[test]
    fn cache_from_another_engine_is_decode_error() {
        let dir = tempfile::tempdir().unwrap();
        store(&dir);
        let err = ChunkStore::open(dir.path().join("cache"), "test+2")
            .err()
            .unwrap();
        assert!(matches!(err, StoreError::OtherEngine { .. }));
        assert!(err.to_string().contains("cache from another version"));
    }

    #[test]
    fn deleting_the_cache_and_replaying_reproduces_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let run = |s: &ChunkStore| {
            let mut sim = Toy::sim(40, Seed(7));
            let mut out = Vec::new();
            for _ in 0..24 {
                sim.step().unwrap();
                let m = s.put(&canonical(sim.tick(), sim.world())).unwrap();
                out.push((m.state_hash, s.get(&m).unwrap().len()));
            }
            out
        };
        let first = run(&store(&dir));
        fs::remove_dir_all(dir.path().join("cache")).unwrap();
        assert_eq!(run(&store(&dir)), first);
    }

    #[test]
    fn missing_or_corrupt_chunk_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let s = store(&dir);
        let m = s.put(&canonical(0, &big_toy())).unwrap();
        let path = s.chunk_path(&m.chunks[0]);
        fs::write(&path, zstd::encode_all(&b"tampered"[..], 1).unwrap()).unwrap();
        assert!(matches!(s.get(&m), Err(StoreError::Corrupt(_))));
        fs::remove_file(&path).unwrap();
        assert!(matches!(s.get(&m), Err(StoreError::MissingChunk(_))));
    }
}
