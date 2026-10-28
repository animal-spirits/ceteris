//! The 10 P2 goods: ICIO industry mapping and weight per value from BACI (specs/econ/SPEC.md,
//! Goods).

use std::collections::BTreeMap;
use std::path::Path;

use crate::read::baci::{baci_by_product, read_hs_cpa};
use crate::read::icio::N_GOODS;
use crate::read::wdi::Names;

/// The goods traded between countries are the first `TRADED`; the rest are local.
pub const TRADED: usize = 7;
pub const NAMES: [&str; N_GOODS] = [
    "Agriculture",
    "Crude oil and gas",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals, refined fuels and metals",
    "Machinery, electronics and vehicles",
    "Electricity and utilities",
    "Construction",
    "Services",
];

/// What an ICIO industry maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Industry {
    Good(usize),
    /// Shared between goods 1 and 2 (the combined B05_06 of earlier editions); the ICIO
    /// aggregation splits it by EIA 2000 production values at Pink Sheet prices.
    Split,
}

const SERVICES: [&str; 20] = [
    "G", "H49", "H50", "H51", "H52", "H53", "I", "J58T60", "J61", "J62_63", "K", "L", "M", "N",
    "O", "P", "Q", "R", "S", "T",
];

/// ICIO industry to good. The 2025 edition has 50 industries (A01, A02 and A03 separate, B05
/// and B06 split, C24A/C24B, C301/C302T309); the older names (A01_02, B05_06, B07_08, C24, C30)
/// stay mapped.
pub fn icio_industry(code: &str) -> Option<Industry> {
    let good = match code {
        "A01" | "A02" | "A03" | "A01_02" => 0,
        "B06" | "B09" => 1,
        "B05" | "B07" | "B08" | "B07_08" => 2,
        "B05_06" => return Some(Industry::Split),
        "C10T12" => 3,
        "C13T15" | "C16" | "C17_18" | "C31T33" => 4,
        "C19" | "C20" | "C21" | "C22" | "C23" | "C24" | "C24A" | "C24B" | "C25" => 5,
        "C26" | "C27" | "C28" | "C29" | "C30" | "C301" | "C302T309" => 6,
        "D" | "E" => 7,
        "F" => 8,
        c if SERVICES.contains(&c) => 9,
        _ => return None,
    };
    Some(Industry::Good(good))
}

/// ISIC Rev 4 division to ICIO industry; divisions 24 and 30 hold two industries that share a
/// good.
fn division_industry(division: &str) -> Option<&'static str> {
    Some(match division {
        "01" => "A01",
        "02" => "A02",
        "03" => "A03",
        "05" => "B05",
        "06" => "B06",
        "07" => "B07",
        "08" => "B08",
        "09" => "B09",
        "10" | "11" | "12" => "C10T12",
        "13" | "14" | "15" => "C13T15",
        "16" => "C16",
        "17" | "18" => "C17_18",
        "19" => "C19",
        "20" => "C20",
        "21" => "C21",
        "22" => "C22",
        "23" => "C23",
        "24" => "C24A",
        "25" => "C25",
        "26" => "C26",
        "27" => "C27",
        "28" => "C28",
        "29" => "C29",
        "30" => "C301",
        "31" | "32" | "33" => "C31T33",
        "35" => "D",
        "36" | "37" | "38" | "39" => "E",
        "41" | "42" | "43" => "F",
        _ => return None,
    })
}

/// Good of a two-digit ISIC Rev 4 division; services (45 and up) are good 9, None for unknown.
pub fn good_of_division(division: &str) -> Option<usize> {
    if let Some(industry) = division_industry(division) {
        return match icio_industry(industry) {
            Some(Industry::Good(g)) => Some(g),
            _ => None,
        };
    }
    let number = division
        .chars()
        .all(|c| c.is_ascii_digit())
        .then(|| division.parse::<u32>().ok())
        .flatten()?;
    (45..=99).contains(&number).then_some(9)
}

/// Good of a CPA 2.1 code of the OECD key (section letter, division, then any finer split such
/// as C24A or C302_309); None for the unallocated `_X`.
pub fn good_of_cpa(code: &str) -> Option<usize> {
    let division: String = code.chars().skip(1).take(2).collect();
    if division.is_empty() || !division.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    good_of_division(&division)
}

/// The good of each HS 6-digit line, which the key gives one activity so that it lies wholly in
/// one good; unallocated lines are left out.
///
/// Sums over the lines run in key order, which is HS code order in the OECD key, the order of
/// this map.
pub fn hs_goods(hs_cpa: &Names) -> BTreeMap<&str, usize> {
    hs_cpa
        .iter()
        .filter_map(|(hs, code)| Some((hs.as_str(), good_of_cpa(code)?)))
        .collect()
}

/// Tonnes per USD for each good (0 for the local goods 7 to 9), from the BACI 2000 flows.
pub fn weights_from_baci(baci_2000: &Path, hs_key: &Path) -> Result<[f64; N_GOODS], String> {
    let hs_cpa = read_hs_cpa(hs_key)?;
    let lines = hs_goods(&hs_cpa);
    let (tonnes, value) = baci_by_product(baci_2000)?;
    let (mut t, mut v) = ([0.0; N_GOODS], [0.0; N_GOODS]);
    for (hs, good) in lines {
        t[good] += tonnes.get(hs).copied().unwrap_or(0.0);
        v[good] += value.get(hs).copied().unwrap_or(0.0);
    }
    let mut w = [0.0; N_GOODS];
    for g in 0..TRADED {
        w[g] = t[g] / v[g];
    }
    Ok(w)
}
