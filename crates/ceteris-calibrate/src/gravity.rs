//! Direct estimation of trade costs by PPML structural gravity (ADR 0022).
//!
//! The estimator ([`ppml::hdfe`]) first reproduces the published PPML estimates of Yotov,
//! Piermartini, Monteiro and Larch (2016) on their sample ([`yotov`]). On 2000 BACI it then
//! estimates, with least-cost (S05) costs between countries from the pack's transport graph
//! (route choice is deferred past P2, ADR 0017):
//!
//! - theta per class (P01, ADR 0023): minus the coefficient on ln(1 + tariff), international
//!   flows;
//! - the border friction nu (P05): a border dummy on a sample with domestic flows (2000 ICIO
//!   output less exports), as an ad valorem equivalent at the class thetas of the same fit;
//! - mode cost ratios (P04a-d): coefficients on tonne-km by mode along the S05 paths, relative to
//!   road.
//!
//! Outputs carry provenance "data". `ceteris gravity --write` puts them in `gravity_2000.toml`
//! only when no stop rule fires ([`stop_rules`]); the econ pack build does not read that file,
//! since whether the model switches to the estimates is the owner's decision.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::path::Path;

use ceteris_pack::econ::goods::hs_goods;
use ceteris_pack::read::baci::{country_codes, for_each_flow, read_hs_cpa};
use ceteris_pack::read::icio::{aggregate, exports};
use ceteris_pack::read::natural_earth::{IsoCodes, read_codes};
use ceteris_pack::read::people::parts;
use ceteris_pack::read::read_member;
use ceteris_reference::io::pairwise_sum;
use ceteris_reference::output::json_float;
use ceteris_reference::ppml::{self, Fit};
use rayon::prelude::*;

use crate::dta::{Column, Dataset, read_dta};
use crate::json::Json;
use crate::write::Table;

pub const BACI: &str = "BACI_HS92_V202601.zip";
pub const YOTOV: &str = "AdvancedGuideFiles.zip";
const ICIO: &str = "1995-2000_SML.zip";
const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
const ADMIN0: &str = "ne_10m_admin_0_countries.zip";
const YEAR: i64 = 2000;

fn fit_err(e: ppml::PpmlError) -> String {
    e.to_string()
}

// ---------------------------------------------------------------- Yotov et al. (2016)

pub const YOTOV_YEARS: [f64; 6] = [1986.0, 1990.0, 1994.0, 1998.0, 2002.0, 2006.0];
pub const GRAVITY_TERMS: [&str; 3] = ["CNTG", "LANG", "CLNY"];

/// One published column: (term, estimate, pair-clustered s.e.), observations, R2 where printed.
pub struct Published {
    pub column: &'static str,
    pub terms: &'static [(&'static str, f64, f64)],
    pub n: usize,
    pub r2: Option<f64>,
}

/// Published PPML estimates and pair-clustered standard errors, transcribed from Yotov,
/// Piermartini, Monteiro and Larch (2016), An Advanced Guide to Trade Policy Analysis: The
/// Structural Gravity Model, WTO and UNCTAD: Table 1 column (4), p. 42; Table 2 columns (2) to
/// (5), p. 47; Table 3 columns (2) to (4), p. 51.
pub const YOTOV_PUBLISHED: [Published; 8] = [
    Published {
        column: "table1_ppml",
        terms: &[
            ("ln_DIST", -0.841, 0.032),
            ("CNTG", 0.437, 0.083),
            ("LANG", 0.247, 0.077),
            ("CLNY", -0.222, 0.116),
        ],
        n: 28152,
        r2: Some(0.614),
    },
    Published {
        column: "table2_ppml",
        terms: &[
            ("ln_DIST_1986", -0.859, 0.037),
            ("ln_DIST_1990", -0.834, 0.038),
            ("ln_DIST_1994", -0.835, 0.035),
            ("ln_DIST_1998", -0.847, 0.035),
            ("ln_DIST_2002", -0.848, 0.032),
            ("ln_DIST_2006", -0.836, 0.031),
            ("CNTG", 0.437, 0.083),
            ("LANG", 0.248, 0.077),
            ("CLNY", -0.222, 0.116),
        ],
        n: 28152,
        r2: None,
    },
    Published {
        column: "table2_intra",
        terms: &[
            ("ln_DIST_1986", -0.980, 0.072),
            ("ln_DIST_1990", -0.940, 0.073),
            ("ln_DIST_1994", -0.915, 0.072),
            ("ln_DIST_1998", -0.887, 0.071),
            ("ln_DIST_2002", -0.884, 0.071),
            ("ln_DIST_2006", -0.872, 0.071),
            ("CNTG", 0.371, 0.140),
            ("LANG", 0.337, 0.168),
            ("CLNY", 0.019, 0.156),
            ("ln_DIST_INTRA", -0.488, 0.101),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table2_brdr",
        terms: &[
            ("ln_DIST_1986", -0.857, 0.063),
            ("ln_DIST_1990", -0.819, 0.063),
            ("ln_DIST_1994", -0.796, 0.063),
            ("ln_DIST_1998", -0.770, 0.063),
            ("ln_DIST_2002", -0.767, 0.063),
            ("ln_DIST_2006", -0.754, 0.062),
            ("CNTG", 0.574, 0.155),
            ("LANG", 0.352, 0.137),
            ("CLNY", 0.027, 0.125),
            ("ln_DIST_INTRA", -0.602, 0.109),
            ("SMCTRY", 1.689, 0.574),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table2_fes",
        terms: &[
            ("ln_DIST_1986", -0.910, 0.032),
            ("ln_DIST_1990", -0.879, 0.032),
            ("ln_DIST_1994", -0.860, 0.032),
            ("ln_DIST_1998", -0.833, 0.032),
            ("ln_DIST_2002", -0.829, 0.032),
            ("ln_DIST_2006", -0.811, 0.032),
            ("CNTG", 0.442, 0.082),
            ("LANG", 0.241, 0.076),
            ("CLNY", -0.220, 0.117),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table3_ppml",
        terms: &[
            ("ln_DIST", -0.822, 0.031),
            ("CNTG", 0.416, 0.083),
            ("LANG", 0.250, 0.077),
            ("CLNY", -0.205, 0.114),
            ("RTA", 0.191, 0.066),
        ],
        n: 28152,
        r2: None,
    },
    Published {
        column: "table3_intra",
        terms: &[
            ("ln_DIST", -0.800, 0.030),
            ("CNTG", 0.393, 0.079),
            ("LANG", 0.244, 0.077),
            ("CLNY", -0.182, 0.113),
            ("RTA", 0.409, 0.069),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table3_endg",
        terms: &[("RTA", 0.557, 0.102)],
        n: 28482,
        r2: None,
    },
];

/// A float ordered as numpy sorts finite values.
#[derive(Clone, Copy, PartialEq)]
struct Num(f64);

impl Eq for Num {}

impl PartialOrd for Num {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Num {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Group codes of `keys`: each key's rank among the distinct keys (numpy's `unique` inverse).
fn codes<K: Ord + Clone>(keys: &[K]) -> Vec<i64> {
    let distinct: Vec<K> = keys
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<K>>()
        .into_iter()
        .collect();
    keys.iter()
        .map(|k| distinct.binary_search(k).expect("a key of the set") as i64)
        .collect()
}

fn numbers<'a>(d: &'a Dataset, name: &str) -> Result<&'a [f64], String> {
    d.get(name)
        .and_then(Column::numbers)
        .ok_or_else(|| format!("no numeric column {name}"))
}

fn strings<'a>(d: &'a Dataset, name: &str) -> Result<&'a [String], String> {
    d.get(name)
        .and_then(Column::strings)
        .ok_or_else(|| format!("no string column {name}"))
}

fn yotov_data(path: &Path, member: &str) -> Result<Dataset, String> {
    let suffix = format!("Chapter1/Datasets/{member}");
    let d = read_dta(&read_member(path, |n| n.ends_with(&suffix))?)?;
    let keep: Vec<bool> = numbers(&d, "year")?
        .iter()
        .map(|y| YOTOV_YEARS.contains(y))
        .collect();
    Ok(d.into_iter().map(|(k, v)| (k, v.select(&keep))).collect())
}

fn pick(v: &[f64], keep: &[bool]) -> Vec<f64> {
    v.iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(x, _)| *x)
        .collect()
}

fn pick_codes(v: &[i64], keep: &[bool]) -> Vec<i64> {
    v.iter()
        .zip(keep)
        .filter(|(_, k)| **k)
        .map(|(x, _)| *x)
        .collect()
}

/// Named regressor columns, in order.
type Columns = Vec<(String, Vec<f64>)>;

fn hdfe(
    flows: &[f64],
    fixed_effects: &[&[i64]],
    regressors: &Columns,
    cluster: Option<&[i64]>,
) -> Result<Fit, String> {
    let reg: Vec<(&str, &[f64])> = regressors
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_slice()))
        .collect();
    ppml::hdfe(flows, fixed_effects, &reg, cluster).map_err(fit_err)
}

/// The application's exporter-time and importer-time codes, pair codes, foreign mask and ln
/// distance.
struct Yotov {
    d: Dataset,
    exp_t: Vec<i64>,
    imp_t: Vec<i64>,
    foreign: Vec<bool>,
    ln_dist: Vec<f64>,
}

impl Yotov {
    fn read(path: &Path, member: &str) -> Result<Yotov, String> {
        let d = yotov_data(path, member)?;
        let year = numbers(&d, "year")?;
        let (exporter, importer) = (strings(&d, "exporter")?, strings(&d, "importer")?);
        let by_year = |c: &[String]| -> Vec<i64> {
            let keys: Vec<(String, Num)> = c
                .iter()
                .zip(year)
                .map(|(c, y)| (c.clone(), Num(*y)))
                .collect();
            codes(&keys)
        };
        let (exp_t, imp_t) = (by_year(exporter), by_year(importer));
        let foreign = exporter.iter().zip(importer).map(|(e, i)| e != i).collect();
        let ln_dist = numbers(&d, "DIST")?.iter().map(|x| x.ln()).collect();
        Ok(Yotov {
            d,
            exp_t,
            imp_t,
            foreign,
            ln_dist,
        })
    }

    fn col(&self, name: &str) -> Result<Vec<f64>, String> {
        numbers(&self.d, name).map(<[f64]>::to_vec)
    }

    fn gravity(&self) -> Result<Columns, String> {
        GRAVITY_TERMS
            .iter()
            .map(|k| Ok(((*k).to_owned(), self.col(k)?)))
            .collect()
    }

    /// One group per country's internal trade, one for all international.
    fn intra_fe(&self) -> Result<Vec<i64>, String> {
        let keys: Vec<&str> = strings(&self.d, "exporter")?
            .iter()
            .zip(&self.foreign)
            .map(|(e, f)| if *f { "" } else { e.as_str() })
            .collect();
        Ok(codes(&keys))
    }

    fn foreign_fit(&self, terms: &Columns, pair: &[i64]) -> Result<Fit, String> {
        let f = &self.foreign;
        let sub: Columns = terms.iter().map(|(k, v)| (k.clone(), pick(v, f))).collect();
        hdfe(
            &pick(&self.col("trade")?, f),
            &[&pick_codes(&self.exp_t, f), &pick_codes(&self.imp_t, f)],
            &sub,
            Some(&pick_codes(pair, f)),
        )
    }
}

/// The PPML columns of [`YOTOV_PUBLISHED`], in its order, estimated as in the authors' Stata
/// files (`TraditionalGravity.do`, `DistancePuzzle.do`, `RTAsEffects.do`): exporter-time and
/// importer-time fixed effects, standard errors clustered by country pair.
pub fn yotov(path: &Path) -> Result<Vec<(String, Fit)>, String> {
    let mut fits = Vec::new();
    let y = Yotov::read(path, "Chapter1Application1.dta")?;
    let pair = codes(
        &numbers(&y.d, "pair_id")?
            .iter()
            .map(|&p| Num(p))
            .collect::<Vec<_>>(),
    );
    let mut terms: Columns = vec![("ln_DIST".into(), y.ln_dist.clone())];
    terms.extend(y.gravity()?);
    fits.push(("table1_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));

    let y = Yotov::read(path, "Chapter1Application2.dta")?;
    let pair = codes(
        &numbers(&y.d, "pair_id")?
            .iter()
            .map(|&p| Num(p))
            .collect::<Vec<_>>(),
    );
    let smctry: Vec<f64> = y.foreign.iter().map(|f| f64::from(u8::from(!f))).collect();
    let year = y.col("year")?;
    let by_year: Columns = YOTOV_YEARS
        .iter()
        .map(|&t| {
            let v = year
                .iter()
                .zip(&y.ln_dist)
                .map(|(&yr, &l)| if yr == t { l } else { 0.0 })
                .collect();
            (format!("ln_DIST_{t}"), v)
        })
        .collect();
    let gravity = y.gravity()?;
    let mut terms = by_year.clone();
    terms.extend(gravity.clone());
    fits.push(("table2_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));
    let mut intra_years: Columns = by_year
        .iter()
        .map(|(k, v)| {
            let v = v.iter().zip(&smctry).map(|(x, s)| x * (1.0 - s)).collect();
            (k.clone(), v)
        })
        .collect();
    let trade = y.col("trade")?;
    let fe: [&[i64]; 2] = [&y.exp_t, &y.imp_t];
    let mut intra = intra_years.clone();
    intra.extend(gravity.clone());
    intra.push((
        "ln_DIST_INTRA".into(),
        y.ln_dist.iter().zip(&smctry).map(|(l, s)| l * s).collect(),
    ));
    fits.push((
        "table2_intra".to_owned(),
        hdfe(&trade, &fe, &intra, Some(&pair))?,
    ));
    let mut brdr = intra;
    brdr.push(("SMCTRY".into(), smctry));
    fits.push((
        "table2_brdr".to_owned(),
        hdfe(&trade, &fe, &brdr, Some(&pair))?,
    ));
    let intra_fe = y.intra_fe()?;
    intra_years.extend(gravity);
    fits.push((
        "table2_fes".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &intra_fe],
            &intra_years,
            Some(&pair),
        )?,
    ));

    let y = Yotov::read(path, "Chapter1Application3.dta")?;
    // As RTAsEffects.do: internal pairs apart.
    let pair_keys: Vec<Num> = numbers(&y.d, "pair_id")?
        .iter()
        .zip(&y.foreign)
        .map(|(&p, &f)| Num(p + if f { 0.0 } else { 100_000.0 }))
        .collect();
    let pair = codes(&pair_keys);
    let mut terms: Columns = vec![("ln_DIST".into(), y.ln_dist.clone())];
    terms.extend(y.gravity()?);
    terms.push(("RTA".into(), y.col("RTA")?));
    fits.push(("table3_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));
    let trade = y.col("trade")?;
    let intra_fe = y.intra_fe()?;
    fits.push((
        "table3_intra".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &intra_fe],
            &terms,
            Some(&pair),
        )?,
    ));
    fits.push((
        "table3_endg".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &pair],
            &vec![("RTA".into(), y.col("RTA")?)],
            Some(&pair),
        )?,
    ));
    Ok(fits)
}

// ---------------------------------------------------------------- S05 costs on the pack graph

pub const ROAD: u8 = 0;
pub const RAIL: u8 = 1;
pub const SEA: u8 = 2;
pub const AIR: u8 = 3;
pub const RAIL_ACCESS: u8 = 4;
pub const PORT_ACCESS: u8 = 5;
pub const MINOR_PORT_ACCESS: u8 = 6;
pub const AIR_ACCESS: u8 = 7;
pub const CANAL: u8 = 8;
/// Path quantities per tonne: cost-relevant km by road, rail, sea and air (length times
/// circuity times the terrain factor, the km that P04a-d price), handling and canal fees (USD),
/// and hours.
pub const N_Q: usize = 6;
pub const Q_FEE: usize = 4;
pub const Q_HOURS: usize = 5;
pub const MODE_NAMES: [&str; 4] = ["road", "rail", "sea", "air"];

/// Path quantities of one edge or path.
pub type Quantities = [f64; N_Q];

/// Mirror of `ceteris_geo::graph::TransportParams::default()` (geo SPEC, P04).
#[derive(Clone, Debug, PartialEq)]
pub struct Transport {
    /// P04a-d.
    pub per_tkm: [f64; 4],
    /// P04e-h.
    pub circuity: [f64; 4],
    /// P04i-l.
    pub speed_kmh: [f64; 4],
    /// P04m: rail terminal, listed port, minor port, airport.
    pub fee: [f64; 4],
    /// P04n.
    pub fee_hours: [f64; 4],
    /// P04o.
    pub canal_fee: f64,
    /// P04p.
    pub terrain_slope: f64,
    /// P04q.
    pub terrain_cap: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Transport {
            per_tkm: [0.10, 0.04, 0.008, 0.80],
            circuity: [1.3, 1.3, 1.0, 1.05],
            speed_kmh: [40.0, 25.0, 25.0, 500.0],
            fee: [5.0, 8.0, 20.0, 60.0],
            fee_hours: [24.0, 48.0, 72.0, 12.0],
            canal_fee: 5.0,
            terrain_slope: 0.5,
            terrain_cap: 3.0,
        }
    }
}

/// P43 time cost of transit lambda_g per day, the starts of `TradeParams::default()` (trade
/// SPEC), traded goods.
pub const TIME_COST: [f64; N_TRADED] = [0.002, 0.002, 0.002, 0.005, 0.010, 0.010, 0.010];
/// P01 classes of `TradeParams::default()` (ADR 0023) and their current values.
pub const CLASSES: [&[usize]; 3] = [&[3], &[6], &[0, 1, 2, 4, 5]];
pub const THETA_DEFAULT: [f64; 3] = [2.62, 5.19, 10.57];
/// P05 start.
pub const BORDER_DEFAULT: f64 = 0.3;
/// `ceteris_trade::goods::tariff_group`: primary, manufactured.
pub const TARIFF_GROUP: [usize; N_TRADED] = [0, 0, 0, 0, 1, 1, 1];
pub const N_TRADED: usize = 7;

/// Per-edge path quantities as `ceteris_geo::graph::price` prices them.
pub fn edge_quantities(
    mode: &[u8],
    length_km: &[f64],
    tri_m: &[f64],
    p: &Transport,
) -> Vec<Quantities> {
    mode.iter()
        .zip(length_km)
        .zip(tri_m)
        .map(|((&m, &length), &tri)| {
            let mut q = [0.0; N_Q];
            let phi = (1.0 + p.terrain_slope * tri / 100.0).min(p.terrain_cap);
            match m {
                ROAD | RAIL | SEA | AIR => {
                    let i = usize::from(m);
                    let km = length * p.circuity[i];
                    q[i] = km * if m == ROAD || m == RAIL { phi } else { 1.0 };
                    q[Q_HOURS] = km / p.speed_kmh[i];
                }
                RAIL_ACCESS | PORT_ACCESS | MINOR_PORT_ACCESS | AIR_ACCESS => {
                    let fee = usize::from(m - RAIL_ACCESS);
                    let km = length * p.circuity[0];
                    q[0] = km;
                    q[Q_FEE] = p.fee[fee];
                    q[Q_HOURS] = km / p.speed_kmh[0] + p.fee_hours[fee];
                }
                CANAL => {
                    let km = length * p.circuity[2];
                    q[2] = km;
                    q[Q_FEE] = p.canal_fee;
                    q[Q_HOURS] = km / p.speed_kmh[2];
                }
                _ => {}
            }
            q
        })
        .collect()
}

/// Cost per tonne of path quantities: P04a-d times the km by mode, plus fees.
pub fn edge_cost(q: &Quantities, p: &Transport) -> f64 {
    let mut c = 0.0;
    for (q, per) in q.iter().zip(&p.per_tkm) {
        c += q * per;
    }
    c + q[Q_FEE]
}

/// The pack's transport graph priced at a [`Transport`], for least-cost trees with path
/// quantities.
pub struct Graph {
    n: usize,
    /// CSR of the cheapest edge per ordered node pair, columns ascending.
    indptr: Vec<usize>,
    target: Vec<usize>,
    weight: Vec<f64>,
    /// The quantities of each CSR entry's edge.
    q: Vec<Quantities>,
}

/// A heap entry ordered as the pair (-distance, node) under `<`: the smallest distance first,
/// the larger node first among equal distances.
#[derive(PartialEq)]
struct Entry(f64, usize);

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        (-self.0)
            .partial_cmp(&-other.0)
            .expect("distances are not NaN")
            .then(self.1.cmp(&other.1))
    }
}

impl Graph {
    pub fn new(nodes: usize, a: &[usize], b: &[usize], q: &[Quantities], p: &Transport) -> Graph {
        let cost: Vec<f64> = q.iter().map(|q| edge_cost(q, p)).collect();
        let mut arcs: Vec<(usize, usize, usize)> = a
            .iter()
            .zip(b)
            .enumerate()
            .map(|(e, (&a, &b))| (a, b, e))
            .chain(a.iter().zip(b).enumerate().map(|(e, (&a, &b))| (b, a, e)))
            .collect();
        // Stable: of parallel edges at equal cost, the first in that order.
        arcs.sort_by(|x, y| {
            (x.0, x.1)
                .cmp(&(y.0, y.1))
                .then(cost[x.2].total_cmp(&cost[y.2]))
        });
        arcs.dedup_by(|later, first| later.0 == first.0 && later.1 == first.1);
        let mut indptr = vec![0; nodes + 1];
        for &(u, _, _) in &arcs {
            indptr[u + 1] += 1;
        }
        for i in 0..nodes {
            indptr[i + 1] += indptr[i];
        }
        Graph {
            n: nodes,
            indptr,
            target: arcs.iter().map(|x| x.1).collect(),
            // A zero-cost edge keeps a negligible cost, as an explicit zero of a sparse graph
            // would be dropped.
            weight: arcs.iter().map(|x| cost[x.2].max(1e-300)).collect(),
            q: arcs.iter().map(|x| q[x.2]).collect(),
        }
    }

    pub fn nodes(&self) -> usize {
        self.n
    }

    /// Least cost from `source` to every node and the path quantities along each least-cost
    /// path; unreachable nodes have infinite cost and zero quantities. Ties between paths of
    /// equal cost go to the node scanned first, the scan taking the smallest distance and then
    /// the largest node index; the quantities are summed by pointer jumping, each pass doubling
    /// the stretch of path summed.
    pub fn tree(&self, source: usize) -> (Vec<f64>, Vec<Quantities>) {
        let n = self.n;
        let mut dist = vec![f64::INFINITY; n];
        let mut parent: Vec<usize> = (0..n).collect();
        let mut via = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        dist[source] = 0.0;
        heap.push(Entry(0.0, source));
        while let Some(Entry(d, v)) = heap.pop() {
            if dist[v] < d {
                continue;
            }
            for j in self.indptr[v]..self.indptr[v + 1] {
                let w = self.target[j];
                let next = d + self.weight[j];
                if dist[w] > next {
                    dist[w] = next;
                    heap.push(Entry(next, w));
                    parent[w] = v;
                    via[w] = j;
                }
            }
        }
        let mut acc: Vec<Quantities> = via
            .iter()
            .map(|&j| {
                if j == usize::MAX {
                    [0.0; N_Q]
                } else {
                    self.q[j]
                }
            })
            .collect();
        loop {
            let up: Vec<usize> = parent.iter().map(|&p| parent[p]).collect();
            let add: Vec<Quantities> = parent.iter().map(|&p| acc[p]).collect();
            for (a, b) in acc.iter_mut().zip(&add) {
                for k in 0..N_Q {
                    a[k] += b[k];
                }
            }
            if up == parent {
                return (dist, acc);
            }
            parent = up;
        }
    }
}

/// Mean S05 path quantities between countries per traded good, `q[((a * C + b) * N_TRADED + g)]`
/// for exporter a and importer b, weighted by exporter province VA of the good times importer
/// province VA; `reach[a * C + b]` is false where no weighted province pair is connected.
#[derive(Clone, Debug, PartialEq)]
pub struct CountryCosts {
    pub n: usize,
    pub q: Vec<Quantities>,
    pub reach: Vec<bool>,
}

impl CountryCosts {
    pub fn at(&self, a: usize, b: usize, g: usize) -> &Quantities {
        &self.q[(a * self.n + b) * N_TRADED + g]
    }
}

/// The index of the first smallest of `v` (numpy's `argmin` without NaN).
fn argmin(v: impl IntoIterator<Item = f64>) -> (usize, f64) {
    let mut best = (0, f64::INFINITY);
    for (i, x) in v.into_iter().enumerate() {
        if i == 0 || x < best.1 {
            best = (i, x);
        }
    }
    best
}

/// The weighted mean of the pair quantities of one country pair: `w[o][d][g] = wo[o][g] *
/// (win[d] * ok[o][d])`; None when no weight is positive.
fn weighted_mean(
    wo: &[[f64; N_TRADED]],
    win: &[f64],
    ok: &[Vec<bool>],
    q: &[Vec<Quantities>],
) -> Option<[Quantities; N_TRADED]> {
    let mut num = [[0.0; N_Q]; N_TRADED];
    let mut den = [0.0; N_TRADED];
    let mut any = false;
    for (o, wo) in wo.iter().enumerate() {
        for (d, &win) in win.iter().enumerate() {
            let inner = win * f64::from(u8::from(ok[o][d]));
            for g in 0..N_TRADED {
                let w = wo[g] * inner;
                any |= w > 0.0;
                den[g] += w;
                for k in 0..N_Q {
                    num[g][k] += w * q[o][d][k];
                }
            }
        }
    }
    any.then(|| {
        let mut out = [[0.0; N_Q]; N_TRADED];
        for g in 0..N_TRADED {
            for k in 0..N_Q {
                out[g][k] = num[g][k] / den[g];
            }
        }
        out
    })
}

/// A province's least costs and path quantities to the provinces and gateways of its country.
struct Access {
    dom_cost: Vec<f64>,
    dom_q: Vec<Quantities>,
    acc_cost: Vec<f64>,
    acc_q: Vec<Quantities>,
}

/// S05 costs between and within the pack's countries as the trade kernel builds them
/// (`EconInputs::network_from_graph`): domestic pairs by the least-cost path between provinces,
/// and pairs of different countries by the cheapest chain of an access leg, a crossing leg
/// between gateways and an access leg, over every gateway pair. Inside a province trade is
/// costless (A2).
pub fn country_costs(pack: &Path, p: &Transport) -> Result<CountryCosts, String> {
    let read = |name: &str| Table::open(&pack.join(name));
    let (nodes, edges, gw, prov) = (
        read("nodes.arrow")?,
        read("edges.arrow")?,
        read("gateways.arrow")?,
        read("econ_provinces.arrow")?,
    );
    let countries = read("countries.arrow")?.strings("code")?;
    let index: BTreeMap<&str, usize> = countries
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let country: Vec<usize> = read("provinces.arrow")?
        .strings("country")?
        .iter()
        .map(|c| {
            index
                .get(c.as_str())
                .copied()
                .ok_or_else(|| format!("province country {c} is not a pack country"))
        })
        .collect::<Result<_, _>>()?;
    let (np, nc) = (country.len(), countries.len());
    let va_cols = (0..10)
        .map(|g| prov.f64(&format!("va_{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mode: Vec<u8> = edges.usize("mode")?.iter().map(|&m| m as u8).collect();
    let q_edge = edge_quantities(&mode, &edges.f64("length_km")?, &edges.f64("tri_m")?, p);
    let graph = Graph::new(
        nodes.rows(),
        &edges.usize("a")?,
        &edges.usize("b")?,
        &q_edge,
        p,
    );
    let gnode = gw.usize("node")?;
    let gcountry = gw.usize("country")?;
    let provinces_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..np).filter(|&s| country[s] == c).collect())
        .collect();
    let gateways_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..gnode.len()).filter(|&i| gcountry[i] == c).collect())
        .collect();

    let access: Vec<Access> = (0..np)
        .into_par_iter()
        .map(|s| {
            let (dist, q) = graph.tree(s);
            let c = country[s];
            let at = |nodes: &mut dyn Iterator<Item = usize>| -> (Vec<f64>, Vec<Quantities>) {
                nodes.map(|n| (dist[n], q[n])).unzip()
            };
            let (dom_cost, dom_q) = at(&mut provinces_of[c].iter().copied());
            let (acc_cost, acc_q) = at(&mut gateways_of[c].iter().map(|&g| gnode[g]));
            Access {
                dom_cost,
                dom_q,
                acc_cost,
                acc_q,
            }
        })
        .collect();
    let crossing: Vec<(Vec<f64>, Vec<Quantities>)> = gnode
        .par_iter()
        .map(|&s| {
            let (dist, q) = graph.tree(s);
            gnode.iter().map(|&n| (dist[n], q[n])).unzip()
        })
        .collect();

    // Exporter: the good's VA; importer: all VA.
    let weight_out: Vec<[f64; N_TRADED]> = (0..np)
        .map(|s| std::array::from_fn(|g| va_cols[g][s]))
        .collect();
    let weight_in: Vec<f64> = (0..np)
        .map(|s| pairwise_sum(&(0..10).map(|g| va_cols[g][s]).collect::<Vec<_>>()))
        .collect();
    let mut out = vec![[0.0; N_Q]; nc * nc * N_TRADED];
    let mut reach = vec![false; nc * nc];
    let mut put = |a: usize, b: usize, mean: [Quantities; N_TRADED]| {
        for (g, q) in mean.into_iter().enumerate() {
            out[(a * nc + b) * N_TRADED + g] = q;
        }
        reach[a * nc + b] = true;
    };
    for a in 0..nc {
        let pa = &provinces_of[a];
        let win_a: Vec<f64> = pa.iter().map(|&s| weight_in[s]).collect();
        if pa.is_empty() || !win_a.iter().any(|&w| w > 0.0) {
            continue;
        }
        // A good the country does not make: all VA.
        let makes: Vec<bool> = (0..N_TRADED)
            .map(|g| pa.iter().any(|&s| weight_out[s][g] > 0.0))
            .collect();
        let wo: Vec<[f64; N_TRADED]> = pa
            .iter()
            .map(|&s| {
                std::array::from_fn(|g| {
                    if makes[g] {
                        weight_out[s][g]
                    } else {
                        weight_in[s]
                    }
                })
            })
            .collect();
        let ok: Vec<Vec<bool>> = pa
            .iter()
            .map(|&s| access[s].dom_cost.iter().map(|c| c.is_finite()).collect())
            .collect();
        let q: Vec<Vec<Quantities>> = pa.iter().map(|&s| access[s].dom_q.clone()).collect();
        if let Some(mean) = weighted_mean(&wo, &win_a, &ok, &q) {
            put(a, a, mean);
        }
        let ga = &gateways_of[a];
        if ga.is_empty() {
            continue;
        }
        // The cheapest first two legs to every gateway j: over a's gateways i, access to i
        // plus the crossing from i to j.
        let (first, u): (Vec<Vec<usize>>, Vec<Vec<f64>>) = pa
            .iter()
            .map(|&s| {
                (0..gnode.len())
                    .map(|j| {
                        argmin(
                            ga.iter()
                                .zip(&access[s].acc_cost)
                                .map(|(&i, c)| c + crossing[i].0[j]),
                        )
                    })
                    .unzip()
            })
            .unzip();
        for b in 0..nc {
            let (pb, gb) = (&provinces_of[b], &gateways_of[b]);
            if b == a || pb.is_empty() || gb.is_empty() || !pb.iter().any(|&s| weight_in[s] > 0.0) {
                continue;
            }
            let mut ok = vec![vec![false; pb.len()]; pa.len()];
            let mut q = vec![vec![[0.0; N_Q]; pb.len()]; pa.len()];
            for o in 0..pa.len() {
                let qa = &access[pa[o]].acc_q;
                for (d, &t) in pb.iter().enumerate() {
                    let cb = &access[t].acc_cost;
                    let (last, best) = argmin(gb.iter().zip(cb).map(|(&j, c)| u[o][j] + c));
                    if !best.is_finite() {
                        continue;
                    }
                    ok[o][d] = true;
                    let ai = first[o][gb[last]];
                    let x = &crossing[ga[ai]].1[gb[last]];
                    let qb = &access[t].acc_q[last];
                    q[o][d] = std::array::from_fn(|k| qa[ai][k] + x[k] + qb[k]);
                }
            }
            if !ok.iter().flatten().any(|&x| x) {
                continue;
            }
            let win_b: Vec<f64> = pb.iter().map(|&s| weight_in[s]).collect();
            if let Some(mean) = weighted_mean(&wo, &win_b, &ok, &q) {
                put(a, b, mean);
            }
        }
    }
    Ok(CountryCosts {
        n: nc,
        q: out,
        reach,
    })
}

/// S05 iceberg factor 1 + c w_g + lambda_g T (trade SPEC, Leg factors) of path quantities.
pub fn tau(q: &Quantities, weight: f64, time_cost: f64, p: &Transport) -> f64 {
    1.0 + edge_cost(q, p) * weight + time_cost * q[Q_HOURS] / 24.0
}

// ---------------------------------------------------------------- flows

/// The fetch cache's file `name` under `cache`.
fn cached(cache: &Path, name: &str) -> std::path::PathBuf {
    cache.join(name)
}

/// The pack country index of every source economy code: each country's parts (or ISO code)
/// and its own code, the first country to claim a code keeping it; BACI's Taiwan and Serbia and
/// Montenegro codes follow their countries.
fn pack_index(codes: &[String], iso: &IsoCodes) -> BTreeMap<String, usize> {
    let mut index = BTreeMap::new();
    for (i, c) in codes.iter().enumerate() {
        for s in parts(c, iso).into_iter().chain([c.clone()]) {
            index.entry(s).or_insert(i);
        }
    }
    for (s, c) in [("S19", "TWN"), ("SCG", "YUG")] {
        if let Some(&i) = index.get(c) {
            index.entry(s.to_owned()).or_insert(i);
        }
    }
    index
}

/// 2000 BACI trade by (exporter, importer, traded good) of the pack countries, USD, as
/// `flows[(o * C + d) * N_TRADED + g]`; HS lines assigned to goods by the OECD HS to CPA key,
/// which puts each line wholly in one good. Key lines that are not numbers (the EU aggregates)
/// match no BACI line.
pub fn baci_flows(cache: &Path, codes: &[String], iso: &IsoCodes) -> Result<Vec<f64>, String> {
    let hs_cpa = read_hs_cpa(&cached(cache, HS_KEY))?;
    let good_of: BTreeMap<i64, usize> = hs_goods(&hs_cpa)
        .into_iter()
        .filter(|(k, _)| !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()))
        .map(|(k, g)| (k.parse().expect("digits"), g))
        .collect();
    let index = pack_index(codes, iso);
    let baci = cached(cache, BACI);
    let lookup: BTreeMap<i64, usize> = country_codes(&baci)?
        .into_iter()
        .filter_map(|(k, v)| Some((k, *index.get(&v)?)))
        .collect();
    let nc = codes.len();
    let mut flows = vec![0.0; nc * nc * N_TRADED];
    // Sums run per batch and are then added, as a bincount per batch would.
    let mut batch = vec![0.0; nc * nc * N_TRADED];
    let (mut touched, mut seen) = (Vec::new(), vec![false; nc * nc]);
    for_each_flow(&baci, |row| {
        let Some(f) = row else {
            for &cell in &touched {
                for g in 0..N_TRADED {
                    flows[cell * N_TRADED + g] += batch[cell * N_TRADED + g];
                    batch[cell * N_TRADED + g] = 0.0;
                }
                seen[cell] = false;
            }
            touched.clear();
            return;
        };
        let (Some(&i), Some(&j)) = (lookup.get(&f.i), lookup.get(&f.j)) else {
            return;
        };
        if i == j {
            return;
        }
        let v = f.v * 1000.0;
        let good = good_of.get(&f.k).copied().filter(|&g| g < N_TRADED);
        let cell = i * nc + j;
        if !seen[cell] {
            seen[cell] = true;
            touched.push(cell);
        }
        for g in 0..N_TRADED {
            let share = if good == Some(g) { 1.0 } else { 0.0 };
            batch[cell * N_TRADED + g] += v * share;
        }
    })?;
    Ok(flows)
}

/// Domestic flows by (pack country, traded good), USD: 2000 ICIO output less exports, for the
/// countries that are ICIO economies (`va_basis` icio); NaN elsewhere and where exports exceed
/// output.
pub fn icio_domestic(
    cache: &Path,
    codes: &[String],
    iso: &IsoCodes,
    va_basis: &[String],
) -> Result<Vec<[f64; N_TRADED]>, String> {
    let icio = cached(cache, ICIO);
    let output = aggregate(&icio, YEAR)?;
    let sold = exports(&icio, YEAR)?;
    codes
        .iter()
        .zip(va_basis)
        .map(|(c, basis)| {
            let code = iso.get(c).unwrap_or(c);
            let mut out = [f64::NAN; N_TRADED];
            if basis == "icio"
                && code != "ROW"
                && let Some(o) = output.get(code)
            {
                let s = sold
                    .get(code)
                    .ok_or_else(|| format!("no ICIO exports of {code}"))?;
                for g in 0..N_TRADED {
                    let d = (o.output[g] - s[g]) * 1e6;
                    out[g] = if d >= 0.0 { d } else { f64::NAN };
                }
            }
            Ok(out)
        })
        .collect()
}

// ---------------------------------------------------------------- estimators

/// One observation per (exporter, importer, traded good); `q` the S05 path quantities and `tau`
/// their iceberg factor at the good's weight and time cost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub exporter: Vec<usize>,
    pub importer: Vec<usize>,
    pub good: Vec<usize>,
    /// USD.
    pub flow: Vec<f64>,
    /// Ad valorem.
    pub tariff: Vec<f64>,
    pub tau: Vec<f64>,
    pub q: Vec<Quantities>,
    /// Tonnes per USD of the good.
    pub weight: Vec<f64>,
    /// P43 lambda of the good.
    pub time_cost: Vec<f64>,
}

impl Sample {
    pub fn rows(&self, mask: &[bool]) -> Sample {
        fn sel<T: Clone>(v: &[T], m: &[bool]) -> Vec<T> {
            v.iter()
                .zip(m)
                .filter(|(_, k)| **k)
                .map(|(x, _)| x.clone())
                .collect()
        }
        Sample {
            exporter: sel(&self.exporter, mask),
            importer: sel(&self.importer, mask),
            good: sel(&self.good, mask),
            flow: sel(&self.flow, mask),
            tariff: sel(&self.tariff, mask),
            tau: sel(&self.tau, mask),
            q: sel(&self.q, mask),
            weight: sel(&self.weight, mask),
            time_cost: sel(&self.time_cost, mask),
        }
    }

    pub fn len(&self) -> usize {
        self.flow.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flow.is_empty()
    }

    /// Exporter-good and importer-good effects (outward and inward multilateral resistance).
    pub fn fixed_effects(&self) -> [Vec<i64>; 2] {
        let code = |c: &[usize]| -> Vec<i64> {
            c.iter()
                .zip(&self.good)
                .map(|(&c, &g)| (c * N_TRADED + g) as i64)
                .collect()
        };
        [code(&self.exporter), code(&self.importer)]
    }

    /// Unordered country pair: the cluster of Yotov et al. (2016).
    pub fn pairs(&self) -> Vec<i64> {
        let lo_hi: Vec<(usize, usize)> = self
            .exporter
            .iter()
            .zip(&self.importer)
            .map(|(&e, &i)| (e.min(i), e.max(i)))
            .collect();
        let top = lo_hi.iter().map(|x| x.1).max().unwrap_or(0) + 1;
        lo_hi.iter().map(|(l, h)| (l * top + h) as i64).collect()
    }

    fn fit(&self, regressors: &Columns) -> Result<Fit, String> {
        let [o, d] = self.fixed_effects();
        hdfe(&self.flow, &[&o, &d], regressors, Some(&self.pairs()))
    }

    fn ln_tariff(&self) -> Vec<f64> {
        self.tariff.iter().map(|t| t.ln_1p()).collect()
    }
}

fn masked(v: &[f64], m: &[bool]) -> Vec<f64> {
    v.iter()
        .zip(m)
        .map(|(x, &m)| x * f64::from(u8::from(m)))
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theta {
    pub goods: Vec<usize>,
    /// Minus the coefficient on ln(1 + tariff).
    pub theta: f64,
    pub se: f64,
    /// Coefficient on ln tau (S05).
    pub tau: f64,
    pub se_tau: f64,
    pub n: usize,
    pub pseudo_r2: f64,
}

/// P01 per class: PPML of international flows of the class's goods on ln(1 + tariff) and ln tau
/// (S05) with exporter-good and importer-good fixed effects, clustered by country pair.
pub fn theta_by_class(s: &Sample, classes: &[&[usize]]) -> Result<Vec<Theta>, String> {
    classes
        .iter()
        .map(|goods| {
            let mask: Vec<bool> = (0..s.len())
                .map(|i| goods.contains(&s.good[i]) && s.exporter[i] != s.importer[i])
                .collect();
            let sub = s.rows(&mask);
            let fit = sub.fit(&vec![
                ("ln_tariff".into(), sub.ln_tariff()),
                ("ln_tau".into(), sub.tau.iter().map(|t| t.ln()).collect()),
            ])?;
            Ok(Theta {
                goods: goods.to_vec(),
                theta: -fit.coef("ln_tariff"),
                se: fit.se("ln_tariff"),
                tau: fit.coef("ln_tau"),
                se_tau: fit.se("ln_tau"),
                n: fit.n,
                pseudo_r2: fit.pseudo_r2,
            })
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Border {
    /// P05, ad valorem: exp(sum_c share_c ln(1 + nu_c)) - 1.
    pub nu: f64,
    pub se: f64,
    pub nu_by_class: Vec<f64>,
    pub se_by_class: Vec<f64>,
    /// From the same fit.
    pub theta_by_class: Vec<f64>,
    /// Coefficients on the border dummy.
    pub border_by_class: Vec<f64>,
    pub n: usize,
    pub pseudo_r2: f64,
}

/// g' cov g.
fn quadratic(g: &[f64], fit: &Fit) -> f64 {
    let k = g.len();
    let gc: Vec<f64> = (0..k)
        .map(|j| {
            let mut s = 0.0;
            for (i, gi) in g.iter().enumerate() {
                s += gi * fit.cov[(i, j)];
            }
            s
        })
        .collect();
    let mut s = 0.0;
    for (a, b) in gc.iter().zip(g) {
        s += a * b;
    }
    s
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    let mut s = 0.0;
    for (x, y) in a.iter().zip(b) {
        s += x * y;
    }
    s
}

fn position(fit: &Fit, name: &str) -> usize {
    fit.names
        .iter()
        .position(|n| n == name)
        .expect("a regressor of the fit")
}

/// Membership of each observation in each class.
fn members(s: &Sample, classes: &[&[usize]]) -> Vec<Vec<bool>> {
    classes
        .iter()
        .map(|c| s.good.iter().map(|g| c.contains(g)).collect())
        .collect()
}

/// P05: PPML on a sample with domestic flows, per class a border dummy, ln(1 + tariff) and ln
/// tau, with exporter-good and importer-good fixed effects, clustered by country pair. The
/// border coefficient is -theta_c ln(1 + nu) (trade SPEC, two-level kernel), so ln(1 + nu_c) =
/// b_c / beta_c with beta_c the tariff coefficient; nu pools the classes with weights `shares`
/// (default equal). Delta-method errors.
pub fn border_friction(
    s: &Sample,
    classes: &[&[usize]],
    shares: Option<&[f64]>,
) -> Result<Border, String> {
    let member = members(s, classes);
    let border: Vec<f64> = s
        .exporter
        .iter()
        .zip(&s.importer)
        .map(|(e, i)| f64::from(u8::from(e != i)))
        .collect();
    let (ln_tariff, ln_tau) = (
        s.ln_tariff(),
        s.tau.iter().map(|t| t.ln()).collect::<Vec<_>>(),
    );
    let mut reg: Columns = Vec::new();
    for (c, m) in member.iter().enumerate() {
        reg.push((format!("ln_tariff_{c}"), masked(&ln_tariff, m)));
        reg.push((format!("ln_tau_{c}"), masked(&ln_tau, m)));
        reg.push((format!("border_{c}"), masked(&border, m)));
    }
    let fit = s.fit(&reg)?;
    let k = classes.len();
    let w: Vec<f64> = match shares {
        None => vec![1.0 / k as f64; k],
        Some(sh) => {
            let total = sh.iter().fold(0.0, |a, b| a + b);
            sh.iter().map(|x| x / total).collect()
        }
    };
    let it: Vec<usize> = (0..k)
        .map(|c| position(&fit, &format!("ln_tariff_{c}")))
        .collect();
    let ib: Vec<usize> = (0..k)
        .map(|c| position(&fit, &format!("border_{c}")))
        .collect();
    let beta: Vec<f64> = it.iter().map(|&i| fit.coef[i]).collect();
    let b: Vec<f64> = ib.iter().map(|&i| fit.coef[i]).collect();
    let ln_nu: Vec<f64> = b.iter().zip(&beta).map(|(b, beta)| b / beta).collect();
    let (mut nu_c, mut se_c) = (Vec::new(), Vec::new());
    let mut grad = vec![0.0; fit.names.len()];
    for c in 0..k {
        let mut g = vec![0.0; fit.names.len()];
        g[ib[c]] = 1.0 / beta[c];
        g[it[c]] = -b[c] / (beta[c] * beta[c]);
        nu_c.push(ln_nu[c].exp_m1());
        se_c.push(ln_nu[c].exp() * quadratic(&g, &fit).sqrt());
        for (a, x) in grad.iter_mut().zip(&g) {
            *a += w[c] * x;
        }
    }
    let pooled = dot(&w, &ln_nu);
    Ok(Border {
        nu: pooled.exp_m1(),
        se: pooled.exp() * quadratic(&grad, &fit).sqrt(),
        nu_by_class: nu_c,
        se_by_class: se_c,
        theta_by_class: beta.iter().map(|x| -x).collect(),
        border_by_class: b,
        n: fit.n,
        pseudo_r2: fit.pseudo_r2,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modes {
    /// P04b-d over P04a: rail, sea and air cost per tonne-km relative to road.
    pub ratio: [f64; 3],
    pub se: [f64; 3],
    /// False for a mode no path uses.
    pub identified: [bool; 3],
    pub theta_by_class: Vec<f64>,
    /// Coefficient on ln tau(ratio).
    pub tau_by_class: Vec<f64>,
    pub n: usize,
    pub pseudo_r2: f64,
    pub iterations: usize,
}

pub const MODE_TOL: f64 = 1e-9;
pub const MODE_MAX_ITER: usize = 50;

/// P04 ratios: nonlinear PPML of international flows on, per class, ln(1 + tariff) and ln
/// tau(r), with tau(r) = 1 + w (f_road (km_road + sum_m r_m km_m) + fees) + lambda hours / 24 the
/// S05 factor along the least-cost paths and road at its P04a value as numeraire. Gauss-Newton:
/// each step is a PPML on the same regressors plus kappa_c w f_road km_m / tau, whose
/// coefficients update r; at convergence the clustered covariance of those coefficients is that
/// of r.
pub fn mode_costs(s: &Sample, classes: &[&[usize]], p: &Transport) -> Result<Modes, String> {
    let mask: Vec<bool> = (0..s.len())
        .map(|i| s.exporter[i] != s.importer[i])
        .collect();
    let s = s.rows(&mask);
    let member = members(&s, classes);
    let identified: [bool; 3] =
        std::array::from_fn(|m| s.q.iter().fold(f64::NEG_INFINITY, |a, q| a.max(q[m + 1])) > 0.0);
    let road = p.per_tkm[0];
    let mut r: [f64; 3] = std::array::from_fn(|m| p.per_tkm[m + 1] / road);
    let tau_of = |r: &[f64; 3]| -> Vec<f64> {
        (0..s.len())
            .map(|i| {
                let q = &s.q[i];
                let km = q[1] * r[0] + q[2] * r[1] + q[3] * r[2];
                1.0 + s.weight[i] * (road * (q[0] + km) + q[Q_FEE])
                    + s.time_cost[i] * q[Q_HOURS] / 24.0
            })
            .collect()
    };
    let ln_tariff = s.ln_tariff();
    let base = |t: &[f64]| -> Columns {
        let ln_t: Vec<f64> = t.iter().map(|t| t.ln()).collect();
        let mut reg = Vec::new();
        for (c, m) in member.iter().enumerate() {
            reg.push((format!("ln_tariff_{c}"), masked(&ln_tariff, m)));
            reg.push((format!("ln_tau_{c}"), masked(&ln_t, m)));
        }
        reg
    };
    let d_name = |m: usize| format!("d_{}", MODE_NAMES[m + 1]);
    let mut fit = s.fit(&base(&tau_of(&r)))?;
    let mut iterations = 0;
    for iteration in 1..=MODE_MAX_ITER {
        let t = tau_of(&r);
        let mut kappa = vec![0.0; s.len()];
        for (c, m) in member.iter().enumerate() {
            let coef = fit.coef(&format!("ln_tau_{c}"));
            for (k, &m) in kappa.iter_mut().zip(m) {
                *k += coef * f64::from(u8::from(m));
            }
        }
        let mut reg = base(&t);
        for m in (0..3).filter(|&m| identified[m]) {
            let v = (0..s.len())
                .map(|i| kappa[i] * s.weight[i] * road * s.q[i][m + 1] / t[i])
                .collect();
            reg.push((d_name(m), v));
        }
        fit = s.fit(&reg)?;
        let step: [f64; 3] = std::array::from_fn(|m| {
            if identified[m] {
                fit.coef(&d_name(m))
            } else {
                0.0
            }
        });
        for m in 0..3 {
            r[m] += step[m];
        }
        if (0..3).all(|m| step[m].abs() <= MODE_TOL * r[m].abs().max(1.0)) {
            iterations = iteration;
            break;
        }
    }
    if iterations == 0 {
        return Err(format!(
            "mode costs did not converge in {MODE_MAX_ITER} steps: {r:?}"
        ));
    }
    let k = classes.len();
    Ok(Modes {
        ratio: std::array::from_fn(|m| if identified[m] { r[m] } else { f64::NAN }),
        se: std::array::from_fn(|m| {
            if identified[m] {
                fit.se(&d_name(m))
            } else {
                f64::NAN
            }
        }),
        identified,
        theta_by_class: (0..k)
            .map(|c| -fit.coef(&format!("ln_tariff_{c}")))
            .collect(),
        tau_by_class: (0..k).map(|c| fit.coef(&format!("ln_tau_{c}"))).collect(),
        n: fit.n,
        pseudo_r2: fit.pseudo_r2,
        iterations,
    })
}

// ---------------------------------------------------------------- 2000 BACI on the pack

/// Counts for the report.
#[derive(Clone, Debug, PartialEq)]
pub struct Notes {
    pub countries: usize,
    pub countries_with_baci_trade: usize,
    pub unreached_pairs: usize,
    pub baci_international_usd: f64,
    pub baci_international_usd_in_sample: f64,
    pub domestic_countries: usize,
    pub domestic_cells_dropped: usize,
    pub class_trade_usd: Vec<f64>,
}

impl Notes {
    pub fn json(&self) -> Json {
        let int = |n: usize| Json::Int(n as i64);
        Json::object([
            ("countries", int(self.countries)),
            (
                "countries_with_baci_trade",
                int(self.countries_with_baci_trade),
            ),
            ("unreached_pairs", int(self.unreached_pairs)),
            (
                "baci_international_usd",
                Json::Float(self.baci_international_usd),
            ),
            (
                "baci_international_usd_in_sample",
                Json::Float(self.baci_international_usd_in_sample),
            ),
            ("domestic_countries", int(self.domestic_countries)),
            ("domestic_cells_dropped", int(self.domestic_cells_dropped)),
            (
                "class_trade_usd",
                Json::List(
                    self.class_trade_usd
                        .iter()
                        .map(|&x| Json::Float(x))
                        .collect(),
                ),
            ),
        ])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Estimates {
    pub theta: Vec<Theta>,
    pub border: Border,
    pub modes: Modes,
    /// 2000 BACI international trade by class, the weights of P05.
    pub class_shares: Vec<f64>,
    pub n_countries: usize,
    pub n_domestic_countries: usize,
    pub notes: Notes,
}

/// The international sample (every pair of different pack countries connected by S05, zeros
/// kept) and the sample with domestic flows (ICIO economies only, both ends), and counts for the
/// report; also the country costs they use.
pub fn samples(pack: &Path, cache: &Path) -> Result<(Sample, Sample, Notes, CountryCosts), String> {
    let read = |name: &str| Table::open(&pack.join(name));
    let codes = read("countries.arrow")?.strings("code")?;
    let (iso, _) = read_codes(&cached(cache, ADMIN0))?;
    let nc = codes.len();
    let flows = baci_flows(cache, &codes, &iso)?;
    let domestic = icio_domestic(
        cache,
        &codes,
        &iso,
        &read("econ_countries.arrow")?.strings("va_basis")?,
    )?;
    let t = read("econ_tariffs.arrow")?;
    let mut rate = vec![0.0; nc * nc * 2];
    for (((i, e), g), r) in t
        .usize("importer")?
        .into_iter()
        .zip(t.usize("exporter")?)
        .zip(t.usize("group")?)
        .zip(t.f64("rate")?)
    {
        rate[(i * nc + e) * 2 + g] = r;
    }
    let p = Transport::default();
    let costs = country_costs(pack, &p)?;
    let weight = read("econ_goods.arrow")?.f64("weight")?;

    let build = |mask: &dyn Fn(usize, usize, usize) -> bool| -> Sample {
        let mut s = Sample::default();
        for o in 0..nc {
            for d in 0..nc {
                for g in 0..N_TRADED {
                    if !mask(o, d, g) {
                        continue;
                    }
                    let q = *costs.at(o, d, g);
                    let (w, lam) = (weight[g], TIME_COST[g]);
                    let flow = if o == d {
                        let x = domestic[o][g];
                        if x.is_nan() { 0.0 } else { x }
                    } else {
                        flows[(o * nc + d) * N_TRADED + g]
                    };
                    let tariff = if o == d {
                        0.0
                    } else {
                        rate[(d * nc + o) * 2 + TARIFF_GROUP[g]]
                    };
                    s.exporter.push(o);
                    s.importer.push(d);
                    s.good.push(g);
                    s.flow.push(flow);
                    s.tariff.push(tariff);
                    s.tau.push(tau(&q, w, lam, &p));
                    s.q.push(q);
                    s.weight.push(w);
                    s.time_cost.push(lam);
                }
            }
        }
        s
    };
    let pair = |o: usize, d: usize| costs.reach[o * nc + d] && o != d;
    let international = build(&|o, d, _| pair(o, d));
    let icio_ok = |c: usize, g: usize| domestic[c][g].is_finite();
    let both: Vec<bool> = (0..nc)
        .map(|c| (0..N_TRADED).all(|g| icio_ok(c, g)))
        .collect();
    let with_domestic = build(&|o, d, g| {
        (pair(o, d) && both[o] && both[d]) || (o == d && icio_ok(o, g) && costs.reach[o * nc + d])
    });
    let mut traded = 0;
    for c in 0..nc {
        let mut out = Vec::with_capacity(nc * N_TRADED);
        let mut inn = Vec::with_capacity(nc * N_TRADED);
        for x in 0..nc {
            for g in 0..N_TRADED {
                out.push(flows[(c * nc + x) * N_TRADED + g]);
                inn.push(flows[(x * nc + c) * N_TRADED + g]);
            }
        }
        if pairwise_sum(&out) + pairwise_sum(&inn) > 0.0 {
            traded += 1;
        }
    }
    let class_trade_usd = CLASSES
        .iter()
        .map(|c| {
            let v: Vec<f64> = (0..nc * nc)
                .flat_map(|cell| c.iter().map(move |&g| (cell, g)))
                .map(|(cell, g)| flows[cell * N_TRADED + g])
                .collect();
            pairwise_sum(&v)
        })
        .collect();
    let notes = Notes {
        countries: nc,
        countries_with_baci_trade: traded,
        unreached_pairs: (0..nc * nc)
            .filter(|&i| !costs.reach[i] && i / nc != i % nc)
            .count(),
        baci_international_usd: pairwise_sum(&flows),
        baci_international_usd_in_sample: pairwise_sum(&international.flow),
        domestic_countries: both.iter().filter(|&&b| b).count(),
        domestic_cells_dropped: (0..nc)
            .filter(|&c| both[c])
            .map(|c| (0..N_TRADED).filter(|&g| !icio_ok(c, g)).count())
            .sum::<usize>(),
        class_trade_usd,
    };
    Ok((international, with_domestic, notes, costs))
}

pub fn estimate(pack: &Path, cache: &Path) -> Result<Estimates, String> {
    let (international, with_domestic, notes, _) = samples(pack, cache)?;
    let trade = &notes.class_trade_usd;
    let total = trade.iter().fold(0.0, |a, b| a + b);
    let shares: Vec<f64> = trade.iter().map(|x| x / total).collect();
    let p = Transport::default();
    Ok(Estimates {
        theta: theta_by_class(&international, &CLASSES)?,
        border: border_friction(&with_domestic, &CLASSES, Some(&shares))?,
        modes: mode_costs(&international, &CLASSES, &p)?,
        class_shares: shares,
        n_countries: notes.countries,
        n_domestic_countries: notes.domestic_countries,
        notes,
    })
}

// ---------------------------------------------------------------- outputs

/// The P04 rows are ratios to road (P04a), the numeraire of [`mode_costs`].
pub const P04_IDS: [&str; 3] = ["P04b/P04a", "P04c/P04a", "P04d/P04a"];

/// One estimated value with its standard error, observations, pseudo-R2 and current default.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: &'static str,
    pub index: usize,
    pub goods: Vec<usize>,
    pub value: f64,
    pub se: f64,
    pub n: usize,
    pub pseudo_r2: f64,
    pub default: f64,
}

pub fn rows(est: &Estimates, p: &Transport) -> Vec<Row> {
    let mut out: Vec<Row> = est
        .theta
        .iter()
        .enumerate()
        .map(|(c, t)| Row {
            id: "P01",
            index: c,
            goods: t.goods.clone(),
            value: t.theta,
            se: t.se,
            n: t.n,
            pseudo_r2: t.pseudo_r2,
            default: THETA_DEFAULT[c],
        })
        .collect();
    let b = &est.border;
    out.push(Row {
        id: "P05",
        index: 0,
        goods: vec![],
        value: b.nu,
        se: b.se,
        n: b.n,
        pseudo_r2: b.pseudo_r2,
        default: BORDER_DEFAULT,
    });
    let m = &est.modes;
    for (k, id) in P04_IDS.iter().enumerate() {
        out.push(Row {
            id,
            index: 0,
            goods: vec![],
            value: m.ratio[k],
            se: m.se[k],
            n: m.n,
            pseudo_r2: m.pseudo_r2,
            default: p.per_tkm[k + 1] / p.per_tkm[0],
        });
    }
    out
}

/// Python's `repr` of a list of ints.
fn int_list(v: &[usize]) -> String {
    let items: Vec<String> = v.iter().map(usize::to_string).collect();
    format!("[{}]", items.join(", "))
}

fn num(x: f64) -> String {
    if x.is_finite() {
        json_float(x)
    } else {
        "nan".into()
    }
}

/// Python's `format(x, f".{precision}g")`.
pub fn py_g(x: f64, precision: usize) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf" } else { "-inf" }.into();
    }
    let p = precision.max(1);
    let sci = format!("{:.*e}", p - 1, x);
    let (mantissa, exp) = sci.split_once('e').expect("exponent notation");
    let exp: i32 = exp.parse().expect("an integer exponent");
    let trim = |s: &str| -> String {
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s.to_owned()
        }
    };
    if -4 <= exp && exp < p as i32 {
        trim(&format!("{:.*}", (p as i32 - 1 - exp) as usize, x))
    } else {
        let sign = if exp < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", trim(mantissa), exp.abs())
    }
}

/// `gravity_2000.toml`.
pub fn to_toml(est: &Estimates) -> String {
    let mut lines: Vec<String> = [
        "# P01 theta by class, P05 border friction and P04 mode cost ratios, estimated by PPML structural gravity on",
        "# 2000 BACI with least-cost (S05) costs between the countries of the pack (ADR 0022; written by",
        "# `ceteris gravity --write`).",
        "# TradeParams::default() keeps its values until the owner decides on the switch.",
        "provenance = \"data\"",
        "",
    ]
    .map(str::to_owned)
    .to_vec();
    for r in rows(est, &Transport::default()) {
        lines.extend([
            "[[estimate]]".to_owned(),
            format!("id = \"{}\"", r.id),
            format!("index = {}", r.index),
            format!("goods = {}", int_list(&r.goods)),
            format!("value = {}", num(r.value)),
            format!("se = {}", num(r.se)),
            format!("n = {}", r.n),
            format!("pseudo_r2 = {}", num(r.pseudo_r2)),
            format!("default = {}", num(r.default)),
            String::new(),
        ]);
    }
    lines.join("\n")
}

/// The stop rules: estimates that go to the owner instead of the pack.
pub fn stop_rules(est: &Estimates) -> Vec<String> {
    let mut fired = Vec::new();
    for (c, t) in est.theta.iter().enumerate() {
        if !(1.0..=25.0).contains(&t.theta) || t.se > t.theta.abs() {
            fired.push(format!(
                "P01 class {c}: theta {:.3} (s.e. {:.3})",
                t.theta, t.se
            ));
        }
    }
    let b = &est.border;
    if !(0.05..=3.0).contains(&b.nu) {
        fired.push(format!("P05: nu {:.3} (s.e. {:.3})", b.nu, b.se));
    }
    let [rail, sea, air] = est.modes.ratio;
    // water < rail < road < air, road = 1; NaN (unidentified) fails.
    if !(sea < rail && rail < 1.0 && 1.0 < air) {
        fired.push(format!(
            "P04: ratios to road rail {}, sea {}, air {} break water < rail < road < air",
            py_g(rail, 4),
            py_g(sea, 4),
            py_g(air, 4)
        ));
    }
    fired
}

fn fixed3(v: &[f64]) -> String {
    v.iter()
        .map(|x| format!("{x:.3}"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn report(est: &Estimates) -> String {
    let mut lines = vec![
        "| ID | Class / mode | Estimate | s.e. | N | pseudo-R2 | Default |".to_owned(),
        "|---|---|---|---|---|---|---|".to_owned(),
    ];
    for r in rows(est, &Transport::default()) {
        let label = if r.id == "P01" {
            format!("class {}, goods {}", r.index, int_list(&r.goods))
        } else {
            String::new()
        };
        lines.push(format!(
            "| {} | {label} | {} | {} | {} | {:.3} | {} |",
            r.id,
            py_g(r.value, 4),
            py_g(r.se, 3),
            r.n,
            r.pseudo_r2,
            py_g(r.default, 4)
        ));
    }
    let b = &est.border;
    lines.push(String::new());
    lines.push(format!(
        "P05 by class: nu {} (s.e. {}); theta in that fit {}; border coefficients {}; class weights {}",
        fixed3(&b.nu_by_class),
        fixed3(&b.se_by_class),
        fixed3(&b.theta_by_class),
        fixed3(&b.border_by_class),
        fixed3(&est.class_shares)
    ));
    let m = &est.modes;
    lines.push(format!(
        "P04 fit: theta by class {}, ln tau coefficients {}, {} Gauss-Newton steps",
        fixed3(&m.theta_by_class),
        fixed3(&m.tau_by_class),
        m.iterations
    ));
    let t: Vec<String> = est
        .theta
        .iter()
        .map(|x| format!("{:.3} ({:.3})", x.tau, x.se_tau))
        .collect();
    lines.push(format!(
        "ln tau coefficients in the theta fits: {}",
        t.join(", ")
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `format(x, ".4g")` and `format(x, ".3g")` in Python.
    #[test]
    fn py_g_matches_python() {
        for (x, p, want) in [
            (0.0, 4, "0"),
            (-0.0, 4, "-0"),
            (1.0, 4, "1"),
            (2.6155, 4, "2.615"),
            (0.4, 4, "0.4"),
            (12345.0, 4, "1.234e+04"),
            (0.000123456, 4, "0.0001235"),
            (0.0000123456, 4, "1.235e-05"),
            (9.9996, 4, "10"),
            (99995.0, 4, "1e+05"),
            (0.0123, 3, "0.0123"),
            (f64::NAN, 3, "nan"),
            (1e300, 3, "1e+300"),
        ] {
            assert_eq!(py_g(x, p), want, "{x} {p}");
        }
    }
}
