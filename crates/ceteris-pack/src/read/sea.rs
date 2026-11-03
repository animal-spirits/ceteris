//! WIOD Socio-Economic Accounts, 2016 release (Timmer et al. 2015; CC BY 4.0): labour
//! compensation (LAB, the self-employed included) and value added (VA) by industry in 2000,
//! summed onto the 10 goods (RFC 0302 D1).

use std::collections::BTreeMap;
use std::path::Path;

use super::icio::N_GOODS;
use super::xlsx::{self, Cell};
use crate::econ::goods::good_of_division;

/// The SEA file in the fetch cache.
pub const SEA: &str = "WIOD_SEA_Nov16.xlsx";

/// Goods of a WIOD industry code: its first ISIC division (A01, C10-C12, M69_M70), else its
/// section (F, I, R_S). Mining (B) is one industry there and counts in full for both goods 1 and
/// 2, the ICIO split of which it cannot give.
pub fn goods_of(code: &str) -> Vec<usize> {
    let division: String = code
        .chars()
        .skip(1)
        .take_while(char::is_ascii_digit)
        .collect();
    if !division.is_empty() {
        return good_of_division(&division).into_iter().collect();
    }
    match code.chars().next() {
        Some('B') => vec![1, 2],
        Some('F') => vec![8],
        Some(c) if ('G'..='U').contains(&c) => vec![9],
        _ => Vec::new(),
    }
}

/// LAB and VA of 2000 by good (millions of national currency), per ISO3 economy.
pub type Sea = BTreeMap<String, ([f64; N_GOODS], [f64; N_GOODS])>;

/// The DATA sheet's 2000 LAB and VA, summed by good; industries without a good, the WIOD
/// totals and other variables left out.
pub fn read_sea(path: &Path) -> Result<Sea, String> {
    let origin = path.display().to_string();
    let mut out = Sea::new();
    for r in xlsx::sheet(path, "DATA")? {
        let text = |name: &str| r.get(name).map(Cell::to_text).unwrap_or_default();
        let variable = text("variable");
        if variable != "LAB" && variable != "VA" {
            continue;
        }
        let Some(value) = r.get("2000").or_else(|| r.get("2000.0")) else {
            continue;
        };
        let value = value
            .to_number()
            .ok_or_else(|| format!("{origin}: 2000 '{}' is not a number", value.to_text()))?;
        let entry = out
            .entry(text("country"))
            .or_insert(([0.0; N_GOODS], [0.0; N_GOODS]));
        for g in goods_of(&text("code")) {
            if variable == "LAB" {
                entry.0[g] += value;
            } else {
                entry.1[g] += value;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wiod_codes_map_to_goods() {
        assert_eq!(goods_of("A01"), [0]);
        assert_eq!(goods_of("B"), [1, 2]);
        assert_eq!(goods_of("C10-C12"), [3]);
        assert_eq!(goods_of("C19"), [1]);
        assert_eq!(goods_of("C31_C32"), [4]);
        assert_eq!(goods_of("D35"), [7]);
        assert_eq!(goods_of("E37-E39"), [7]);
        assert_eq!(goods_of("F"), [8]);
        assert_eq!(goods_of("G45"), [9]);
        assert_eq!(goods_of("R_S"), [9]);
        assert_eq!(goods_of("U"), [9]);
        assert!(goods_of("TOT").is_empty());
    }
}
