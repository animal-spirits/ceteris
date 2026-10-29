//! Stata datasets in formats 113 to 115 (Stata 8 to 13), the files of Yotov et al. (2016).

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
        fn pick<T: Clone>(v: &[T], keep: &[bool]) -> Vec<T> {
            v.iter()
                .zip(keep)
                .filter(|(_, k)| **k)
                .map(|(x, _)| x.clone())
                .collect()
        }
        match self {
            Column::Numbers(v) => Column::Numbers(pick(v, keep)),
            Column::Strings(v) => Column::Strings(pick(v, keep)),
        }
    }
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

/// The columns of a Stata dataset in format 113 to 115: numbers as f64 with missing values as
/// NaN, strings with trailing NULs removed, read as Latin-1.
pub fn read_dta(data: &[u8]) -> Result<Dataset, String> {
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
