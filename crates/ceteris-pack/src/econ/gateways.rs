//! Gateways of the two-level trade kernel (specs/econ/SPEC.md, Pack additions, Gateways; trade
//! SPEC, Two-level kernel).
//!
//! Each country gets k gateways, k proportional to the square root of its 2000 trade (BACI
//! exports plus imports), at least one, at most its candidate count, scaled so that the world
//! total is near `TOTAL`. The candidates are the country's listed ports and airports and its land
//! crossing points: the endpoints of graph edges (road, rail) whose two ends lie in different 2000
//! countries, merged within `MERGE_KM` per country pair. The k gateways are the k-medoids of the
//! candidates on great-circle distance (an agent default, ADR 0014).
//!
//! Each gateway's mass m_a (ADR 0092) is its share of its country's 2000 trade throughput, so the
//! masses of a country's gateways sum to 1. The pack's sources give no throughput per gateway
//! (Natural Earth ports and OurAirports airports carry no tonnage, value or passenger counts, and
//! land crossings no traffic), so every gateway of a country with k gateways takes 1/k.
//!
//! Every land crossing enters as a candidate, not as a gateway of its own: gateways for all of
//! the P1 graph's merged crossings would make the all-pairs crossing matrix about twenty times
//! larger than the kernel's budget of about 600 gateways (trade SPEC, cost per good).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, UInt8Array, UInt32Array};

use crate::numeric::pairwise_sum;

pub const TOTAL: usize = 600;
pub const MERGE_KM: f64 = 50.0;
pub const EARTH_KM: f64 = 6371.0088;
/// Gateway kinds in `gateways.arrow`.
pub const PORT: u8 = 0;
pub const AIRPORT: u8 = 1;
pub const LAND: u8 = 2;
pub const KINDS: [&str; 3] = ["port", "airport", "land crossing"];
/// The province of a node outside every province (a sea node).
pub const NO_PROVINCE: u32 = u32::MAX;
pub const ITERATIONS: usize = 100;

/// Node kinds and edge modes of the P1 graph that gateways read.
const NODE_PORT: u8 = 2;
const NODE_AIRPORT: u8 = 3;
const LAND_MODES: [u8; 2] = [0, 1];

/// Columns of `nodes.arrow`.
#[derive(Clone, Debug, Default)]
pub struct Nodes {
    pub kind: Vec<u8>,
    pub province: Vec<u32>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
}

/// Columns of `edges.arrow`.
#[derive(Clone, Debug, Default)]
pub struct Edges {
    pub a: Vec<u32>,
    pub b: Vec<u32>,
    pub mode: Vec<u8>,
}

/// One row of `gateways.arrow`; `country` indexes the sorted country codes of the provinces,
/// `mass` is the gateway's share of its country's trade throughput.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gateway {
    pub node: u32,
    pub country: u32,
    pub kind: u8,
    pub mass: f64,
}

/// Haversine distance in km, in numpy's operation order.
pub fn great_circle_km(lon1: f64, lat1: f64, lon2: f64, lat2: f64) -> f64 {
    let (lon1, lat1, lon2, lat2) = (
        lon1.to_radians(),
        lat1.to_radians(),
        lon2.to_radians(),
        lat2.to_radians(),
    );
    let s = libm::sin((lat2 - lat1) / 2.0);
    let t = libm::sin((lon2 - lon1) / 2.0);
    let h = s * s + libm::cos(lat1) * libm::cos(lat2) * (t * t);
    2.0 * EARTH_KM * libm::asin(h.min(1.0).sqrt())
}

/// Country code of each graph node through its province (`country` by province index); "" for
/// sea nodes.
pub fn node_country(province: &[u32], country: &[String]) -> Result<Vec<String>, String> {
    province
        .iter()
        .map(|&p| match p {
            NO_PROVINCE => Ok(String::new()),
            _ => country
                .get(p as usize)
                .cloned()
                .ok_or_else(|| format!("node province {p} of {} provinces", country.len())),
        })
        .collect()
}

/// Road and rail edges between two countries as (node, node), one per country pair and
/// `MERGE_KM` of midpoint.
///
/// Edges are taken in pack order; an edge is merged into the first kept crossing of its country
/// pair whose midpoint lies within `MERGE_KM` of its own.
pub fn land_crossings(
    nodes: &Nodes,
    edges: &Edges,
    country: &[String],
) -> Result<Vec<(usize, usize)>, String> {
    let mut kept: BTreeMap<(&str, &str), Vec<(f64, f64)>> = BTreeMap::new();
    let mut out = Vec::new();
    for ((&a, &b), mode) in edges.a.iter().zip(&edges.b).zip(&edges.mode) {
        let (x, y) = (a as usize, b as usize);
        let (Some(cx), Some(cy)) = (country.get(x), country.get(y)) else {
            return Err(format!("edge {x}-{y} of {} nodes", country.len()));
        };
        if !LAND_MODES.contains(mode) || cx == cy || cx.is_empty() || cy.is_empty() {
            continue;
        }
        let pair = if cx < cy { (cx, cy) } else { (cy, cx) };
        let (lon, lat) = (&nodes.lon, &nodes.lat);
        let mut mid = ((lon[x] + lon[y]) / 2.0, (lat[x] + lat[y]) / 2.0);
        if (lon[x] - lon[y]).abs() > 180.0 {
            mid.0 = ((mid.0 + 360.0) % 360.0) - 180.0;
        }
        let near = kept.entry((pair.0.as_str(), pair.1.as_str())).or_default();
        if near
            .iter()
            .any(|&(lo, la)| great_circle_km(mid.0, mid.1, lo, la) < MERGE_KM)
        {
            continue;
        }
        near.push(mid);
        out.push((x, y));
    }
    Ok(out)
}

/// Gateways per country: round(s sqrt(trade)) within [1, candidates], with s the scale whose sum
/// is nearest `total`.
pub fn counts(
    trade: &BTreeMap<String, f64>,
    candidates: &BTreeMap<String, usize>,
    total: usize,
) -> BTreeMap<String, usize> {
    let root: Vec<f64> = candidates
        .keys()
        .map(|c| trade.get(c).copied().unwrap_or(0.0).sqrt())
        .collect();
    let cap: Vec<f64> = candidates.values().map(|&n| n as f64).collect();
    let k = |s: f64| -> Vec<usize> {
        root.iter()
            .zip(&cap)
            .map(|(r, &c)| (s * r).round_ties_even().max(1.0).min(c) as usize)
            .collect()
    };
    let sum = |s: f64| k(s).iter().sum::<usize>();
    let (mut lo, mut hi) = (0.0, 1.0);
    let reach = total.min(candidates.values().sum());
    while sum(hi) < reach {
        hi *= 2.0;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        if sum(mid) < total {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let key = |s: f64| (sum(s).abs_diff(total), s);
    let best = if key(hi) < key(lo) { hi } else { lo };
    candidates.keys().cloned().zip(k(best)).collect()
}

/// Index of the first smallest value, as `np.argmin`.
fn argmin(values: impl IntoIterator<Item = f64>) -> usize {
    let mut best = (0, f64::INFINITY);
    for (i, v) in values.into_iter().enumerate() {
        if i == 0 || v < best.1 {
            best = (i, v);
        }
    }
    best.0
}

/// Indices of k medoids on great-circle distance: greedy build, then alternate assignment and
/// medoid updates, sorted.
///
/// Deterministic: ties go to the lower index, and in the assignment to the medoid chosen first.
pub fn kmedoids(lon: &[f64], lat: &[f64], k: usize, iterations: usize) -> Vec<usize> {
    let n = lon.len();
    if k >= n {
        return (0..n).collect();
    }
    let d: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| great_circle_km(lon[i], lat[i], lon[j], lat[j]))
                .collect()
        })
        .collect();
    let mut medoids = vec![argmin(d.iter().map(|row| pairwise_sum(row)))];
    let mut nearest = d[medoids[0]].clone();
    let mut gain_row = vec![0.0; n];
    for _ in 1..k {
        let mut best = (0, f64::NEG_INFINITY);
        for (i, row) in d.iter().enumerate() {
            let gain = if medoids.contains(&i) {
                -1.0
            } else {
                for ((g, &near), &x) in gain_row.iter_mut().zip(&nearest).zip(row) {
                    *g = (near - x).max(0.0);
                }
                pairwise_sum(&gain_row)
            };
            if i == 0 || gain > best.1 {
                best = (i, gain);
            }
        }
        medoids.push(best.0);
        for (near, &x) in nearest.iter_mut().zip(&d[best.0]) {
            *near = near.min(x);
        }
    }
    for _ in 0..iterations {
        let label: Vec<usize> = (0..n)
            .map(|j| argmin(medoids.iter().map(|&m| d[m][j])))
            .collect();
        let mut update = medoids.clone();
        for (c, slot) in update.iter_mut().enumerate() {
            let members: Vec<usize> = (0..n).filter(|&j| label[j] == c).collect();
            if members.is_empty() {
                continue;
            }
            let sums = members.iter().map(|&r| {
                let row: Vec<f64> = members.iter().map(|&j| d[r][j]).collect();
                pairwise_sum(&row)
            });
            *slot = members[argmin(sums)];
        }
        if update == medoids {
            break;
        }
        medoids = update;
    }
    medoids.sort_unstable();
    medoids
}

/// `gateways.arrow` rows, sorted by country, then node. `province_country` is the country code of
/// each province; the country index is into its sorted distinct codes.
pub fn select(
    nodes: &Nodes,
    edges: &Edges,
    province_country: &[String],
    trade: &BTreeMap<String, f64>,
    total: usize,
) -> Result<Vec<Gateway>, String> {
    let country = node_country(&nodes.province, province_country)?;
    let mut cand: BTreeMap<&str, BTreeMap<usize, u8>> = BTreeMap::new();
    for (i, &kind) in nodes.kind.iter().enumerate() {
        if kind == NODE_PORT || kind == NODE_AIRPORT {
            let gateway = if kind == NODE_PORT { PORT } else { AIRPORT };
            cand.entry(&country[i]).or_default().insert(i, gateway);
        }
    }
    for (x, y) in land_crossings(nodes, edges, &country)? {
        for v in [x, y] {
            cand.entry(&country[v])
                .or_default()
                .entry(v)
                .or_insert(LAND);
        }
    }
    let codes: Vec<&str> = province_country
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let missing: Vec<&str> = codes
        .iter()
        .copied()
        .filter(|c| cand.get(c).is_none_or(BTreeMap::is_empty))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "countries without a port, airport or land crossing: {missing:?}"
        ));
    }
    let sizes: BTreeMap<String, usize> = codes
        .iter()
        .map(|c| ((*c).to_owned(), cand[c].len()))
        .collect();
    let k = counts(trade, &sizes, total);
    let mut rows = Vec::new();
    for (ci, c) in codes.iter().enumerate() {
        let ids: Vec<(usize, u8)> = cand[c].iter().map(|(&i, &kind)| (i, kind)).collect();
        let lon: Vec<f64> = ids.iter().map(|&(i, _)| nodes.lon[i]).collect();
        let lat: Vec<f64> = ids.iter().map(|&(i, _)| nodes.lat[i]).collect();
        let medoids = kmedoids(&lon, &lat, k[*c], ITERATIONS);
        let mass = 1.0 / medoids.len() as f64;
        for m in medoids {
            let (node, kind) = ids[m];
            rows.push(Gateway {
                node: u32::try_from(node).map_err(|_| format!("node {node} beyond uint32"))?,
                country: u32::try_from(ci).map_err(|_| "too many countries for uint32")?,
                kind,
                mass,
            });
        }
    }
    Ok(rows)
}

/// `gateways.arrow`: node, country, kind, mass.
pub fn columns(rows: &[Gateway]) -> Vec<(String, ArrayRef)> {
    vec![
        (
            "node".into(),
            Arc::new(UInt32Array::from_iter_values(rows.iter().map(|r| r.node))),
        ),
        (
            "country".into(),
            Arc::new(UInt32Array::from_iter_values(
                rows.iter().map(|r| r.country),
            )),
        ),
        (
            "kind".into(),
            Arc::new(UInt8Array::from_iter_values(rows.iter().map(|r| r.kind))),
        ),
        (
            "mass".into(),
            Arc::new(Float64Array::from_iter_values(rows.iter().map(|r| r.mass))),
        ),
    ]
}

/// The gateway summary: count by kind, the range per country and the eight countries with most.
pub fn report(rows: &[Gateway], codes: &[String]) -> String {
    let mut kinds = [0usize; KINDS.len()];
    for r in rows {
        kinds[usize::from(r.kind)] += 1;
    }
    let width = rows
        .iter()
        .map(|r| r.country as usize + 1)
        .max()
        .unwrap_or(0)
        .max(codes.len());
    let mut per = vec![0usize; width];
    for r in rows {
        per[r.country as usize] += 1;
    }
    let mut top: Vec<(usize, &String)> = per.iter().copied().zip(codes).collect();
    top.sort_unstable_by(|a, b| b.cmp(a));
    let most: Vec<String> = top
        .iter()
        .take(8)
        .map(|(n, c)| format!("{c} {n}"))
        .collect();
    let by_kind: Vec<String> = kinds
        .iter()
        .zip(KINDS)
        .map(|(n, name)| format!("{n} {name}"))
        .collect();
    format!(
        "gateways: {} ({}); per country min {}, max {}; most: {}",
        rows.len(),
        by_kind.join(", "),
        per.iter().min().copied().unwrap_or(0),
        per.iter().max().copied().unwrap_or(0),
        most.join(", ")
    )
}
