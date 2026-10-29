//! Reader for the pack's `.fill` buffers (ADR 0010; written by `ceteris_pack::geo::fills`):
//! triangulated polygons in Web Mercator (0 to 1, y down), one feature index per vertex.

use std::path::Path;

const MAGIC: &[u8; 4] = b"CFIL";
const VERSION: u32 = 1;

pub struct Fill {
    pub xy: Vec<[f32; 2]>,
    pub feature: Vec<u32>,
    pub index: Vec<u32>,
    pub features: u32,
}

impl Fill {
    pub fn read(path: &Path) -> Result<Self, String> {
        let data = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&data).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(data: &[u8]) -> Result<Self, String> {
        if data.len() < 20 || &data[..4] != MAGIC {
            return Err("not a fill file".into());
        }
        let words: Vec<u32> = data[4..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect();
        let [version, v, i, features] = [words[0], words[1], words[2], words[3]];
        if version != VERSION {
            return Err(format!("fill format version {version}, expected {VERSION}"));
        }
        let (v, i) = (v as usize, i as usize);
        if data.len() != 20 + 12 * v + 4 * i {
            return Err("length does not match the header".into());
        }
        let body = &words[4..];
        Ok(Self {
            xy: body[..2 * v]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| p.map(f32::from_bits))
                .collect(),
            feature: body[2 * v..3 * v].to_vec(),
            index: body[3 * v..].to_vec(),
            features,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(words: &[u32]) -> Vec<u8> {
        MAGIC
            .iter()
            .copied()
            .chain(words.iter().flat_map(|w| w.to_le_bytes()))
            .collect()
    }

    #[test]
    fn parses_one_triangle() {
        let xy = [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0].map(f32::to_bits);
        let mut words = vec![VERSION, 3, 3, 1];
        words.extend(xy);
        words.extend([0, 0, 0, 0, 1, 2]);
        let fill = Fill::parse(&bytes(&words)).unwrap();
        assert_eq!(fill.xy, [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]);
        assert_eq!(fill.feature, [0, 0, 0]);
        assert_eq!(fill.index, [0, 1, 2]);
        assert_eq!(fill.features, 1);
    }

    #[test]
    fn rejects_bad_headers() {
        assert!(Fill::parse(b"CFIX0000000000000000").is_err());
        assert!(Fill::parse(&bytes(&[2, 0, 0, 0])).is_err());
        assert!(Fill::parse(&bytes(&[VERSION, 3, 3, 1])).is_err());
    }
}
