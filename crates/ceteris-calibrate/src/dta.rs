//! Stata datasets in formats 113 to 115 (Stata 8 to 13), the files of Yotov et al. (2016), and
//! format 118 (Stata 14 to 18), the US import files of Schott (2008) (ADR 0277).

use std::collections::BTreeMap;

/// One column: numbers as f64 with missing values as NaN, or strings.
#[derive(Clone, Debug, PartialEq)]
pub enum Column {
    Numbers(Vec<f64>),
    Strings(Vec<String>),
}

impl Column {
    pub fn numbers(&self) -> Option<&[f64]> {
        match self {
            Column::Numbers(v) => Some(v),
            Column::Strings(_) => None,
        }
    }

    pub fn strings(&self) -> Option<&[String]> {
        match self {
            Column::Strings(v) => Some(v),
            Column::Numbers(_) => None,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Column::Numbers(v) => v.len(),
            Column::Strings(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The rows where `keep` holds.
    pub fn select(&self, keep: &[bool]) -> Column {
        match self {
            Column::Numbers(v) => Column::Numbers(select(v, keep)),
            Column::Strings(v) => Column::Strings(select(v, keep)),
        }
    }
}

/// The elements of `v` where `keep` holds.
pub(crate) fn select<T: Clone>(v: &[T], keep: &[bool]) -> Vec<T> {
    v.iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(x, _)| x.clone())
        .collect()
}

/// Columns by variable name.
pub type Dataset = BTreeMap<String, Column>;

/// Width in bytes of numeric type code `t` (byte, int, long, float, double).
fn numeric_width(t: u8) -> Option<usize> {
    match t {
        251 => Some(1),
        252 => Some(2),
        253 | 254 => Some(4),
        255 => Some(8),
        _ => None,
    }
}

struct Bytes<'a> {
    data: &'a [u8],
    little: bool,
}

impl Bytes<'_> {
    fn take<const N: usize>(&self, at: usize) -> Result<[u8; N], String> {
        let mut b: [u8; N] = self
            .data
            .get(at..at + N)
            .ok_or("Stata file ends early")?
            .try_into()
            .expect("slice of N bytes");
        if !self.little {
            b.reverse();
        }
        Ok(b)
    }

    /// The value at `at` of numeric type `t`, NaN for a missing code (at or above the type's
    /// smallest missing value).
    fn number(&self, t: u8, at: usize) -> Result<f64, String> {
        Ok(match t {
            251 => {
                let v = i8::from_le_bytes(self.take::<1>(at)?);
                if v >= 101 { f64::NAN } else { f64::from(v) }
            }
            252 => {
                let v = i16::from_le_bytes(self.take::<2>(at)?);
                if v >= 32741 { f64::NAN } else { f64::from(v) }
            }
            253 => {
                let v = i32::from_le_bytes(self.take::<4>(at)?);
                if v >= 2_147_483_621 {
                    f64::NAN
                } else {
                    f64::from(v)
                }
            }
            254 => {
                let v = f32::from_le_bytes(self.take::<4>(at)?);
                if v >= 1.701_411_8e38 {
                    f64::NAN
                } else {
                    f64::from(v)
                }
            }
            _ => {
                let v = f64::from_le_bytes(self.take::<8>(at)?);
                if v >= 8.988_465_674_311_58e307 {
                    f64::NAN
                } else {
                    v
                }
            }
        })
    }
}

const TAGGED: &[u8] = b"<stata_dta><header><release>";

/// Format 118's type code as the old numeric code (byte, int, long, float, double) or, for a
/// fixed string, its width.
fn tagged_type(t: u16) -> Result<(u8, usize), String> {
    Ok(match t {
        1..=2045 => (0, usize::from(t)),
        65526 => (255, 8),
        65527 => (254, 4),
        65528 => (253, 4),
        65529 => (252, 2),
        65530 => (251, 1),
        _ => return Err(format!("Stata type {t} (strL or unknown) is not supported")),
    })
}

/// Format 118: the header, then the section offsets of `<map>` locate the types, names and data.
fn read_dta_118(data: &[u8]) -> Result<Dataset, String> {
    let ends = || "Stata file ends early".to_string();
    let mut pos = TAGGED.len();
    let expect = |pos: &mut usize, tag: &[u8]| -> Result<(), String> {
        if data.get(*pos..*pos + tag.len()) != Some(tag) {
            return Err(format!(
                "Stata 118: expected {} at byte {pos}",
                String::from_utf8_lossy(tag)
            ));
        }
        *pos += tag.len();
        Ok(())
    };
    expect(&mut pos, b"118</release><byteorder>")?;
    let little = match data.get(pos..pos + 3) {
        Some(b"LSF") => true,
        Some(b"MSF") => false,
        _ => return Err("Stata 118: byte order is not LSF or MSF".into()),
    };
    pos += 3;
    let b = Bytes { data, little };
    expect(&mut pos, b"</byteorder><K>")?;
    let nvar = usize::from(u16::from_le_bytes(b.take::<2>(pos)?));
    pos += 2;
    expect(&mut pos, b"</K><N>")?;
    let nobs = u64::from_le_bytes(b.take::<8>(pos)?);
    let nobs = usize::try_from(nobs).map_err(|_| format!("{nobs} observations"))?;
    pos += 8;
    expect(&mut pos, b"</N><label>")?;
    pos += 2 + usize::from(u16::from_le_bytes(b.take::<2>(pos)?));
    expect(&mut pos, b"</label><timestamp>")?;
    pos += 1 + usize::from(*data.get(pos).ok_or_else(ends)?);
    expect(&mut pos, b"</timestamp></header><map>")?;
    let map: Vec<usize> = (0..14)
        .map(|i| {
            let v = u64::from_le_bytes(b.take::<8>(pos + 8 * i)?);
            usize::try_from(v).map_err(|_| format!("map offset {v}"))
        })
        .collect::<Result<_, _>>()?;
    let mut at = map[2];
    expect(&mut at, b"<variable_types>")?;
    let types = (0..nvar)
        .map(|i| tagged_type(u16::from_le_bytes(b.take::<2>(at + 2 * i)?)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut at = map[3];
    expect(&mut at, b"<varnames>")?;
    let names = (0..nvar)
        .map(|i| {
            let raw = data
                .get(at + 129 * i..at + 129 * (i + 1))
                .ok_or_else(ends)?;
            let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
            String::from_utf8(raw[..end].to_vec()).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let mut start = map[9];
    expect(&mut start, b"<data>")?;
    let row: usize = types.iter().map(|t| t.1).sum();
    if data.len() < start + row * nobs {
        return Err(ends());
    }
    let mut out = Dataset::new();
    let mut offset = 0;
    for (name, (t, width)) in names.into_iter().zip(types) {
        let at = |r: usize| start + r * row + offset;
        let column = if t == 0 {
            Column::Strings(
                (0..nobs)
                    .map(|r| {
                        let raw = &data[at(r)..at(r) + width];
                        let end = raw.iter().position(|&c| c == 0).unwrap_or(width);
                        String::from_utf8_lossy(&raw[..end]).into_owned()
                    })
                    .collect(),
            )
        } else {
            Column::Numbers(
                (0..nobs)
                    .map(|r| b.number(t, at(r)))
                    .collect::<Result<_, _>>()?,
            )
        };
        out.insert(name, column);
        offset += width;
    }
    Ok(out)
}

/// The columns of a Stata dataset in format 113 to 115: numbers as f64 with missing values as
/// NaN, strings with trailing NULs removed, read as Latin-1.
pub fn read_dta(data: &[u8]) -> Result<Dataset, String> {
    if data.starts_with(TAGGED) {
        return read_dta_118(data);
    }
    let (version, order) = match data {
        [v, o, ..] => (*v, *o),
        _ => return Err("Stata file ends early".into()),
    };
    if !(113..=115).contains(&version) {
        return Err(format!("Stata format {version} is not supported"));
    }
    let b = Bytes {
        data,
        little: order == 2,
    };
    let nvar = usize::from(u16::from_le_bytes(b.take::<2>(4)?));
    let nobs = i32::from_le_bytes(b.take::<4>(6)?);
    let nobs = usize::try_from(nobs).map_err(|_| format!("{nobs} observations"))?;
    let mut pos = 4 + 2 + 4 + 81 + 18;
    let types = data.get(pos..pos + nvar).ok_or("Stata file ends early")?;
    pos += nvar;
    let names = (0..nvar)
        .map(|i| {
            let raw = data
                .get(pos + 33 * i..pos + 33 * (i + 1))
                .ok_or("Stata file ends early")?;
            let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
            String::from_utf8(raw[..end].to_vec()).map_err(|e| e.to_string())
        })
        .collect::<Result<Vec<_>, String>>()?;
    let format_width = if version >= 114 { 49 } else { 12 };
    pos += 33 * nvar + 2 * (nvar + 1) + format_width * nvar + 33 * nvar + 81 * nvar;
    loop {
        let kind = i8::from_le_bytes(b.take::<1>(pos)?);
        let length = i32::from_le_bytes(b.take::<4>(pos + 1)?);
        let length = usize::try_from(length).map_err(|_| format!("expansion field {length}"))?;
        pos += 5 + length;
        if kind == 0 {
            break;
        }
    }
    let widths: Vec<usize> = types
        .iter()
        .map(|&t| numeric_width(t).unwrap_or(usize::from(t)))
        .collect();
    let row: usize = widths.iter().sum();
    if data.len() < pos + row * nobs {
        return Err("Stata file ends early".into());
    }
    let mut out = Dataset::new();
    let mut offset = 0;
    for ((name, &t), width) in names.into_iter().zip(types).zip(widths) {
        let at = |r: usize| pos + r * row + offset;
        let column = if numeric_width(t).is_some() {
            Column::Numbers(
                (0..nobs)
                    .map(|r| b.number(t, at(r)))
                    .collect::<Result<_, _>>()?,
            )
        } else {
            Column::Strings(
                (0..nobs)
                    .map(|r| {
                        let raw = &data[at(r)..at(r) + width];
                        let end = raw.iter().rposition(|&c| c != 0).map_or(0, |i| i + 1);
                        raw[..end].iter().map(|&c| char::from(c)).collect()
                    })
                    .collect(),
            )
        };
        out.insert(name, column);
        offset += width;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A format 118 file, little-endian: str4 `cty`, byte `k`, long `w` and double `v`.
    fn file_118(rows: &[(&str, i8, i32, f64)]) -> Vec<u8> {
        let mut f = TAGGED.to_vec();
        f.extend(b"118</release><byteorder>LSF</byteorder><K>");
        f.extend(4u16.to_le_bytes());
        f.extend(b"</K><N>");
        f.extend((rows.len() as u64).to_le_bytes());
        f.extend(b"</N><label>");
        f.extend(2u16.to_le_bytes());
        f.extend(b"ab</label><timestamp>");
        f.push(3);
        f.extend(b"now</timestamp></header><map>");
        let map_at = f.len();
        f.extend([0u8; 8 * 14]);
        f.extend(b"</map>");
        let mut map = [0u64; 14];
        map[2] = f.len() as u64;
        f.extend(b"<variable_types>");
        for t in [4u16, 65530, 65528, 65526] {
            f.extend(t.to_le_bytes());
        }
        f.extend(b"</variable_types>");
        map[3] = f.len() as u64;
        f.extend(b"<varnames>");
        for n in ["cty", "k", "w", "v"] {
            let mut name = [0u8; 129];
            name[..n.len()].copy_from_slice(n.as_bytes());
            f.extend(name);
        }
        f.extend(b"</varnames>");
        map[9] = f.len() as u64;
        f.extend(b"<data>");
        for (c, k, w, v) in rows {
            let mut s = [0u8; 4];
            s[..c.len()].copy_from_slice(c.as_bytes());
            f.extend(s);
            f.extend(k.to_le_bytes());
            f.extend(w.to_le_bytes());
            f.extend(v.to_le_bytes());
        }
        f.extend(b"</data></stata_dta>");
        for (i, m) in map.iter().enumerate() {
            f[map_at + 8 * i..map_at + 8 * (i + 1)].copy_from_slice(&m.to_le_bytes());
        }
        f
    }

    #[test]
    fn reads_format_118() {
        let missing = f64::from_bits(0x7fe0_0000_0000_0000);
        let d = read_dta(&file_118(&[
            ("5700", 3, 120, 2.5),
            ("12", 101, -7, missing),
        ]))
        .unwrap();
        assert_eq!(d["cty"].strings().unwrap(), ["5700", "12"]);
        let k = d["k"].numbers().unwrap();
        assert_eq!(k[0], 3.0);
        assert!(k[1].is_nan());
        assert_eq!(d["w"].numbers().unwrap(), [120.0, -7.0]);
        let v = d["v"].numbers().unwrap();
        assert_eq!(v[0], 2.5);
        assert!(v[1].is_nan());
    }

    #[test]
    fn a_wrong_tag_is_an_error() {
        let mut f = file_118(&[("1", 1, 1, 1.0)]);
        let at = TAGGED.len() + 3;
        f[at] = b'x';
        assert!(read_dta(&f).unwrap_err().contains("expected"));
    }
}
