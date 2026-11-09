//! OECD Analytical AMNE, 2026 edition (ICIO 2025 grid): value added of 2000 by host, industry
//! and ownership, and gross output by host, industry and parent, summed onto the goods with a
//! foreign-owned share (ADR 0309).

use std::collections::BTreeMap;
use std::fs::File;
use std::io::BufReader;
use std::ops::RangeInclusive;
use std::path::Path;

use csv::StringRecord;

use super::icio::N_GOODS;
use super::{Csv, YEAR, cell, err, float};
use crate::econ::goods::good_of_division;

/// Value added by ownership and gross output by parent.
pub const VA: &str = "aamne2026-xvem.csv";
pub const OUTPUT: &str = "aamne2026-bilateral-output.csv";
/// AAMNE's rest of the world, as host and as parent.
pub const ROW: &str = "ROW";
/// Goods with a foreign-owned share: oil and gas, mining and manufacturing (ADR 0104).
pub const GOODS: RangeInclusive<usize> = 1..=6;

/// Goods of an AAMNE industry among [`GOODS`]: mining (B05T09) is one industry and counts in full
/// for goods 1 and 2, as in SEA; a manufacturing industry its first division's good.
pub fn goods_of(code: &str) -> Vec<usize> {
    let division: String = code
        .chars()
        .skip(1)
        .take_while(char::is_ascii_digit)
        .collect();
    match code.chars().next() {
        Some('B') => vec![1, 2],
        Some('C') => good_of_division(&division)
            .filter(|g| GOODS.contains(g))
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// Domestic- and foreign-owned value added of 2000 by good (million USD), per host economy
/// (ISO3, `ROW` included).
pub type Owned = BTreeMap<String, ([f64; N_GOODS], [f64; N_GOODS])>;

fn value(text: &str, origin: &str) -> Result<f64, String> {
    if text.is_empty() {
        Ok(0.0)
    } else {
        float(text, origin)
    }
}

/// The 2000 GVA of `VA` by host, ownership (`D`, `F`) and good.
pub fn read_owned(path: &Path) -> Result<Owned, String> {
    let file = File::open(path).map_err(|e| err(path, e))?;
    let mut csv = Csv::new(BufReader::new(file), path)?;
    let [year, cou, own, isic, gva] = ["year", "cou", "own", "isic", "GVA"].map(|c| csv.col(c));
    let (year, cou, own, isic, gva) = (year?, cou?, own?, isic?, gva?);
    let origin = path.display().to_string();
    let mut out = Owned::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        if cell(&row, year) != YEAR.to_string() {
            continue;
        }
        let foreign = match cell(&row, own) {
            "D" => false,
            "F" => true,
            other => return Err(format!("{origin}: ownership '{other}'")),
        };
        let v = value(cell(&row, gva), &origin)?;
        let entry = out
            .entry(cell(&row, cou).to_owned())
            .or_insert(([0.0; N_GOODS], [0.0; N_GOODS]));
        for g in goods_of(cell(&row, isic)) {
            if foreign {
                entry.1[g] += v;
            } else {
                entry.0[g] += v;
            }
        }
    }
    Ok(out)
}

/// Gross output of 2000 in [`GOODS`] (mining once) owned abroad, by host and parent economy;
/// a host's own column left out.
pub fn read_parents(path: &Path) -> Result<BTreeMap<String, BTreeMap<String, f64>>, String> {
    let file = File::open(path).map_err(|e| err(path, e))?;
    let mut csv = Csv::new(BufReader::new(file), path)?;
    let [year, cou, isic] = ["year", "cou", "isic"].map(|c| csv.col(c));
    let (year, cou, isic) = (year?, cou?, isic?);
    let parents: Vec<(usize, String)> = csv
        .header
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != year && i != cou && i != isic)
        .map(|(i, name)| (i, name.clone()))
        .collect();
    let origin = path.display().to_string();
    let mut out: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        if cell(&row, year) != YEAR.to_string() || goods_of(cell(&row, isic)).is_empty() {
            continue;
        }
        let host = cell(&row, cou);
        let by_parent = out.entry(host.to_owned()).or_default();
        for (i, parent) in parents.iter().filter(|(_, p)| p.as_str() != host) {
            *by_parent.entry(parent.clone()).or_insert(0.0) += value(cell(&row, *i), &origin)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aamne_codes_map_to_goods() {
        assert_eq!(goods_of("B05T09"), [1, 2]);
        assert_eq!(goods_of("C19"), [1]);
        assert_eq!(goods_of("C10T12"), [3]);
        assert_eq!(goods_of("C17T18"), [4]);
        assert_eq!(goods_of("C24"), [5]);
        assert_eq!(goods_of("C29"), [6]);
        assert_eq!(goods_of("C31T33"), [4]);
        assert!(goods_of("A01T03").is_empty());
        assert!(goods_of("D35_E36T39").is_empty());
        assert!(goods_of("K64T66").is_empty());
    }
}
