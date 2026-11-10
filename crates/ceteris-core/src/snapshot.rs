//! Canonical state encoding, state hashes and content-addressed snapshots
//! (Sections 4.1, 14.3; ADR 0002).

use rayon::prelude::*;
use std::collections::BTreeMap;
#[expect(
    clippy::disallowed_types,
    reason = "keyed lookup only, never iterated (ADR 0097)"
)]
use std::collections::HashMap;
use std::fmt;

const MAGIC: &[u8; 4] = b"CETS";
/// 2: ledger accounts carry their kind and wide balances are `i128` (ADR 0106). 3: worlds
/// carry their parameters and name their network.
const FORMAT: u32 = 3;
const ZSTD_LEVEL: i32 = 1;
const ZSTD_FRAME: usize = 4 << 20;

pub type Hash = [u8; 32];

pub trait Encode {
    fn encode(&self, e: &mut Encoder);
}

pub trait Decode: Sized {
    fn decode(d: &mut Decoder) -> Result<Self, DecodeError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "snapshot decode: {}", self.0)
    }
}

impl std::error::Error for DecodeError {}

/// Little-endian, floats by bit pattern, slices length-prefixed.
#[derive(Default)]
pub struct Encoder {
    buf: Vec<u8>,
}

impl Encoder {
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i128(&mut self, v: i128) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn f64(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    pub fn count(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("slice too long to encode"));
    }
    pub fn u32s(&mut self, vs: &[u32]) {
        self.count(vs.len());
        vs.iter().for_each(|&v| self.u32(v));
    }
    pub fn i64s(&mut self, vs: &[i64]) {
        self.count(vs.len());
        vs.iter().for_each(|&v| self.i64(v));
    }
    pub fn f64s(&mut self, vs: &[f64]) {
        self.count(vs.len());
        vs.iter().for_each(|&v| self.f64(v));
    }
    pub fn bools(&mut self, vs: &[bool]) {
        self.count(vs.len());
        vs.iter().for_each(|&v| self.u8(u8::from(v)));
    }
}

pub struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn at_end(&self) -> bool {
        self.pos == self.buf.len()
    }

    pub fn finish(self) -> Result<(), DecodeError> {
        if self.pos == self.buf.len() {
            Ok(())
        } else {
            Err(DecodeError(format!(
                "{} trailing bytes",
                self.buf.len() - self.pos
            )))
        }
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let end = self.pos + N;
        let bytes = self
            .buf
            .get(self.pos..end)
            .ok_or_else(|| DecodeError(format!("truncated at byte {}", self.pos)))?;
        self.pos = end;
        Ok(bytes.try_into().unwrap())
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }
    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    pub fn i64(&mut self) -> Result<i64, DecodeError> {
        Ok(i64::from_le_bytes(self.take()?))
    }
    pub fn i128(&mut self) -> Result<i128, DecodeError> {
        Ok(i128::from_le_bytes(self.take()?))
    }
    pub fn f64(&mut self) -> Result<f64, DecodeError> {
        Ok(f64::from_bits(u64::from_le_bytes(self.take()?)))
    }
    pub fn count(&mut self) -> Result<usize, DecodeError> {
        Ok(self.u32()? as usize)
    }
    pub fn u32s(&mut self) -> Result<Vec<u32>, DecodeError> {
        (0..self.count()?).map(|_| self.u32()).collect()
    }
    pub fn i64s(&mut self) -> Result<Vec<i64>, DecodeError> {
        (0..self.count()?).map(|_| self.i64()).collect()
    }
    pub fn f64s(&mut self) -> Result<Vec<f64>, DecodeError> {
        (0..self.count()?).map(|_| self.f64()).collect()
    }
    pub fn bools(&mut self) -> Result<Vec<bool>, DecodeError> {
        (0..self.count()?)
            .map(|_| match self.u8()? {
                0 => Ok(false),
                1 => Ok(true),
                b => Err(DecodeError(format!("invalid bool byte {b}"))),
            })
            .collect()
    }
}

/// Canonical bytes of a world at a tick: magic, format version, tick, world.
pub fn canonical<W: Encode>(tick: u32, world: &W) -> Vec<u8> {
    let mut e = Encoder::default();
    e.buf.extend_from_slice(MAGIC);
    e.u32(FORMAT);
    e.u32(tick);
    world.encode(&mut e);
    e.into_bytes()
}

pub fn state_hash(canonical: &[u8]) -> Hash {
    // the same digest as `blake3::hash`, with the tree hashed on the rayon pool
    *blake3::Hasher::new()
        .update_rayon(canonical)
        .finalize()
        .as_bytes()
}

pub fn hex(hash: &Hash) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Decodes canonical bytes into (tick, world) with `decode` reading the world part.
pub fn decode_canonical_with<W>(
    bytes: &[u8],
    decode: impl FnOnce(&mut Decoder) -> Result<W, DecodeError>,
) -> Result<(u32, W), DecodeError> {
    let mut d = Decoder::new(bytes);
    if &d.take::<4>()? != MAGIC {
        return Err(DecodeError("bad magic".into()));
    }
    let format = d.u32()?;
    if format != FORMAT {
        return Err(DecodeError(format!(
            "snapshot format version {format}, this build reads {FORMAT}"
        )));
    }
    let tick = d.u32()?;
    let world = decode(&mut d)?;
    d.finish()?;
    Ok((tick, world))
}

/// Decodes canonical bytes into (tick, world).
pub fn decode_canonical<W: Decode>(bytes: &[u8]) -> Result<(u32, W), DecodeError> {
    decode_canonical_with(bytes, W::decode)
}

/// In-memory snapshots, content-addressed so identical states share one blob.
#[derive(Default)]
pub struct SnapshotStore {
    by_tick: BTreeMap<u32, Hash>,
    #[expect(
        clippy::disallowed_types,
        reason = "keyed lookup only, never iterated (ADR 0097)"
    )]
    blobs: HashMap<Hash, Vec<u8>>,
}

impl SnapshotStore {
    /// Stores the canonical bytes for `tick` and returns their hash.
    pub fn insert(&mut self, tick: u32, canonical: &[u8]) -> Hash {
        // the blob is concatenated zstd frames of `ZSTD_FRAME` bytes, compressed in parallel and
        // read back whole by `decode_all`; compressing ahead of the dedup check costs only on a repeat
        let (hash, blob) = rayon::join(
            || state_hash(canonical),
            || {
                let frames: Vec<Vec<u8>> = canonical
                    .par_chunks(ZSTD_FRAME)
                    .map(|c| zstd::bulk::compress(c, ZSTD_LEVEL).expect("zstd compression failed"))
                    .collect();
                frames.concat()
            },
        );
        self.blobs.entry(hash).or_insert(blob);
        self.by_tick.insert(tick, hash);
        hash
    }

    /// The latest snapshot at or before `tick`, as (tick, canonical bytes).
    pub fn at_or_before(&self, tick: u32) -> Option<(u32, Vec<u8>)> {
        let (&t, hash) = self.by_tick.range(..=tick).next_back()?;
        let bytes = zstd::stream::decode_all(&self.blobs[hash][..]).expect("corrupt snapshot blob");
        Some((t, bytes))
    }

    /// Drops the snapshots after `tick`, which belong to a timeline left by an intervention.
    pub fn truncate_after(&mut self, tick: u32) {
        self.by_tick.split_off(&(tick + 1));
        let kept: std::collections::BTreeSet<Hash> = self.by_tick.values().copied().collect();
        self.blobs.retain(|h, _| kept.contains(h));
    }

    pub fn hashes(&self) -> impl Iterator<Item = (u32, Hash)> + '_ {
        self.by_tick.iter().map(|(&t, &h)| (t, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, PartialEq)]
    struct Sample {
        a: Vec<u32>,
        b: Vec<i64>,
        c: Vec<f64>,
        d: Vec<bool>,
        e: u16,
    }

    impl Encode for Sample {
        fn encode(&self, e: &mut Encoder) {
            e.u32s(&self.a);
            e.i64s(&self.b);
            e.f64s(&self.c);
            e.bools(&self.d);
            e.u16(self.e);
        }
    }

    impl Decode for Sample {
        fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
            Ok(Self {
                a: d.u32s()?,
                b: d.i64s()?,
                c: d.f64s()?,
                d: d.bools()?,
                e: d.u16()?,
            })
        }
    }

    proptest! {
        #[test]
        fn canonical_round_trip(
            a in proptest::collection::vec(any::<u32>(), 0..50),
            b in proptest::collection::vec(any::<i64>(), 0..50),
            c in proptest::collection::vec(-1e300f64..1e300, 0..50),
            d in proptest::collection::vec(any::<bool>(), 0..50),
            e in any::<u16>(),
            tick in any::<u32>(),
        ) {
            let s = Sample { a, b, c, d, e };
            let bytes = canonical(tick, &s);
            let (t, back): (u32, Sample) = decode_canonical(&bytes).unwrap();
            prop_assert_eq!(t, tick);
            prop_assert_eq!(&back, &s);
            prop_assert_eq!(canonical(t, &back), bytes);
        }
    }

    #[test]
    fn truncated_and_trailing_bytes_are_rejected() {
        let s = Sample {
            a: vec![1],
            b: vec![],
            c: vec![],
            d: vec![],
            e: 0,
        };
        let mut bytes = canonical(0, &s);
        assert!(decode_canonical::<Sample>(&bytes[..bytes.len() - 1]).is_err());
        bytes.push(0);
        assert!(decode_canonical::<Sample>(&bytes).is_err());
    }

    #[test]
    fn i128_round_trips() {
        let mut e = Encoder::default();
        for v in [0, -1, i128::MIN, i128::MAX, i128::from(i64::MIN) - 1] {
            e.i128(v);
        }
        let bytes = e.into_bytes();
        let mut d = Decoder::new(&bytes);
        for v in [0, -1, i128::MIN, i128::MAX, i128::from(i64::MIN) - 1] {
            assert_eq!(d.i128().unwrap(), v);
        }
        d.finish().unwrap();
    }

    #[test]
    fn old_codec_version_is_a_decode_error() {
        let s = Sample {
            a: vec![1],
            b: vec![2],
            c: vec![],
            d: vec![],
            e: 0,
        };
        let mut bytes = canonical(7, &s);
        for old in [0, FORMAT - 1, FORMAT + 1] {
            bytes[4..8].copy_from_slice(&old.to_le_bytes());
            let err = decode_canonical::<Sample>(&bytes).unwrap_err();
            assert!(
                err.0.contains(&format!("version {old}")),
                "{err} does not name version {old}"
            );
        }
    }

    #[test]
    fn identical_states_share_a_blob() {
        let mut store = SnapshotStore::default();
        let h1 = store.insert(12, b"same");
        let h2 = store.insert(24, b"same");
        assert_eq!(h1, h2);
        assert_eq!(store.blobs.len(), 1);
        assert_eq!(store.at_or_before(30).unwrap(), (24, b"same".to_vec()));
        assert_eq!(store.at_or_before(23).unwrap().0, 12);
    }
}
