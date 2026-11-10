//! 2000 applied tariffs by importer, exporter and tariff group (specs/econ/SPEC.md, Pack
//! additions; trade SPEC, Tariff wedge).
//!
//! The importer's WDI weighted mean applied rate of each group (primary goods, manufactures)
//! applies to every exporter outside its trade agreements; inside a free trade agreement or
//! customs union in force on 1 January 2000 the rate is zero. An importer with no WDI rate takes
//! its donor's (same region, nearest log income), flagged imputed.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, UInt8Array, UInt32Array};

use super::icio::REGIONS;
use super::people::donor;
use crate::numeric::pairwise_sum;
use crate::read::people::{Observed, parts};
use crate::read::tariffs::Pairs;
use crate::read::wdi::Names;

/// Tariff groups: primary goods, manufactured goods.
pub const GROUPS: usize = 2;
/// Source codes of economies that a pack country combines or that the sources name otherwise,
/// besides `PARTS`.
pub const ALIASES: [(&str, &str); 4] = [
    ("SCG", "YUG"),
    ("SRB", "YUG"),
    ("MNE", "YUG"),
    ("ANT", "ANT"),
];

/// Pairs of pack country indices.
pub type IndexPairs = BTreeSet<(usize, usize)>;

#[derive(Clone, Debug, PartialEq)]
pub struct Tariffs {
    /// `[importer][exporter][group]`, fraction.
    pub rate: Vec<Vec<[f64; GROUPS]>>,
    /// The importer's rate per group outside its agreements.
    pub own: Vec<[f64; GROUPS]>,
    pub agreement: Vec<Vec<bool>>,
    /// The importer's rates are its donor's.
    pub imputed: Vec<bool>,
    /// Per importer, the source code whose rates it takes, empty where they are its own.
    pub donor: Vec<String>,
}

/// Source code pairs as pack country index pairs, both orders; pairs within one pack country are
/// dropped.
pub fn pack_pairs(pairs: &Pairs, codes: &[String], iso: &Names) -> IndexPairs {
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, c) in codes.iter().enumerate() {
        for s in parts(c, iso).into_iter().chain([c.clone()]) {
            index.entry(s).or_insert(i);
        }
    }
    for (s, c) in ALIASES {
        if let Some(i) = codes.iter().position(|x| x == c) {
            index.entry(s.to_owned()).or_insert(i);
        }
    }
    let mut out = IndexPairs::new();
    for (a, b) in pairs {
        if let (Some(&i), Some(&j)) = (index.get(a), index.get(b))
            && i != j
        {
            out.insert((i, j));
            out.insert((j, i));
        }
    }
    out
}

fn mean(values: &[f64]) -> f64 {
    pairwise_sum(values) / values.len() as f64
}

/// Rates for every (importer, exporter, group) of the pack countries `codes`; zero on the
/// diagonal and in agreements.
///
/// A dependency (`sovereign` names another pack country) trades free of tariffs with its
/// sovereign, and without a WDI rate of its own it takes the sovereign's; any other country
/// without one takes its donor's.
pub fn table(
    rates: &[BTreeMap<String, f64>],
    agreements: &IndexPairs,
    codes: &[String],
    iso: &Names,
    obs: &Observed,
    sovereign: &BTreeMap<String, String>,
) -> Result<Tariffs, String> {
    if rates.len() != GROUPS {
        return Err(format!("{} rate series, expected {GROUPS}", rates.len()));
    }
    let n = codes.len();
    let found: Vec<Option<[f64; GROUPS]>> = codes
        .iter()
        .map(|c| {
            let ps = parts(c, iso);
            let mut own = [0.0; GROUPS];
            for (g, series) in rates.iter().enumerate() {
                let v: Vec<f64> = ps.iter().filter_map(|p| series.get(p)).copied().collect();
                if v.is_empty() {
                    return None;
                }
                own[g] = mean(&v);
            }
            Some(own)
        })
        .collect();
    let have: BTreeMap<&String, [f64; GROUPS]> = codes
        .iter()
        .zip(&found)
        .filter_map(|(c, f)| Some((c, (*f)?)))
        .collect();
    let by_iso: BTreeMap<String, [f64; GROUPS]> = have
        .iter()
        .map(|(c, v)| (iso.get(*c).unwrap_or(c).clone(), *v))
        .collect();
    let mut own = vec![[0.0; GROUPS]; n];
    let mut donors = vec![String::new(); n];
    for (i, c) in codes.iter().enumerate() {
        if let Some(f) = found[i] {
            own[i] = f;
            continue;
        }
        let sov = sovereign.get(c).unwrap_or(c);
        if let (true, Some(v)) = (sov != c, have.get(sov)) {
            donors[i] = sov.clone();
            own[i] = *v;
            continue;
        }
        let ps = parts(c, iso);
        let region = REGIONS
            .iter()
            .find(|(k, _)| k == c)
            .map(|(_, r)| *r)
            .or_else(|| {
                ps.iter()
                    .find_map(|p| obs.region.get(p))
                    .map(String::as_str)
            });
        let income: Vec<f64> = ps
            .iter()
            .filter_map(|p| obs.gdp_per_capita.get(p))
            .copied()
            .collect();
        let log_income = (!income.is_empty()).then(|| libm::log(mean(&income)));
        let d = donor(region, log_income, &by_iso, obs, &ps)?;
        own[i] = by_iso[&d];
        donors[i] = d;
    }
    let mut agreement = vec![vec![false; n]; n];
    for &(i, j) in agreements {
        agreement[i][j] = true;
    }
    for (c, s) in sovereign {
        if c != s
            && let (Some(i), Some(j)) = (
                codes.iter().position(|x| x == c),
                codes.iter().position(|x| x == s),
            )
        {
            agreement[i][j] = true;
            agreement[j][i] = true;
        }
    }
    let rate = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| {
                    if i == j || agreement[i][j] {
                        [0.0; GROUPS]
                    } else {
                        own[i]
                    }
                })
                .collect()
        })
        .collect();
    let imputed = donors.iter().map(|d| !d.is_empty()).collect();
    Ok(Tariffs {
        rate,
        own,
        agreement,
        imputed,
        donor: donors,
    })
}

/// `econ_tariffs.arrow`: one row per (importer, exporter, group), importer-major, groups innermost.
pub fn columns(t: &Tariffs) -> Result<Vec<(String, ArrayRef)>, String> {
    let n = u32::try_from(t.rate.len()).map_err(|_| "too many countries for uint32")?;
    let rows = || {
        (0..n as usize)
            .flat_map(|i| (0..n as usize).flat_map(move |j| (0..GROUPS).map(move |g| (i, j, g))))
    };
    Ok(vec![
        (
            "importer".into(),
            Arc::new(UInt32Array::from_iter_values(
                rows().map(|(i, _, _)| i as u32),
            )),
        ),
        (
            "exporter".into(),
            Arc::new(UInt32Array::from_iter_values(
                rows().map(|(_, j, _)| j as u32),
            )),
        ),
        (
            "group".into(),
            Arc::new(UInt8Array::from_iter_values(
                rows().map(|(_, _, g)| g as u8),
            )),
        ),
        (
            "rate".into(),
            Arc::new(Float64Array::from_iter_values(
                rows().map(|(i, j, g)| t.rate[i][j][g]),
            )),
        ),
        (
            "agreement".into(),
            Arc::new(
                rows()
                    .map(|(i, j, _)| t.agreement[i][j])
                    .collect::<BooleanArray>(),
            ),
        ),
        (
            "imputed".into(),
            Arc::new(
                rows()
                    .map(|(i, _, _)| t.imputed[i])
                    .collect::<BooleanArray>(),
            ),
        ),
    ])
}

/// The tariff summary: importers with their own rates, agreement pairs, mean own rates and the
/// share of `trade` (by pack country) covered by own rates.
pub fn report(t: &Tariffs, codes: &[String], trade: &BTreeMap<String, f64>) -> String {
    let n = codes.len();
    let imputed = t.imputed.iter().filter(|x| **x).count();
    let pairs = t.agreement.iter().flatten().filter(|x| **x).count() / 2;
    let own: Vec<&[f64; GROUPS]> = t
        .own
        .iter()
        .zip(&t.imputed)
        .filter_map(|(o, i)| (!i).then_some(o))
        .collect();
    let means: Vec<String> = (0..GROUPS)
        .map(|g| {
            let column: Vec<f64> = own.iter().map(|o| o[g]).collect();
            format!("{:.4}", mean(&column))
        })
        .collect();
    let mut lines = vec![
        format!(
            "tariffs: {} of {n} importers with WDI rates, {imputed} from donors; {pairs} country pairs in agreements",
            n - imputed
        ),
        format!(
            "mean rate (primary, manufactured), own importers: {}",
            means.join(", ")
        ),
    ];
    if !trade.is_empty() {
        let covered: f64 = trade
            .iter()
            .filter_map(|(c, v)| {
                let i = codes.iter().position(|x| x == c)?;
                (!t.imputed[i]).then_some(*v)
            })
            .sum();
        let total: f64 = trade.values().sum();
        lines.push(format!(
            "share of 2000 trade with own rates: {:.4}",
            covered / total
        ));
    }
    lines.join("\n")
}
