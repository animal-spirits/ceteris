//! P04d from 2000 air value shares (ADR 0277; geo SPEC, Air rate from mode shares): the air part
//! of the trade kernel's gateway-pair sum, fitted to the data's share by a Bernoulli
//! quasi-likelihood for fractional data (Papke and Wooldridge 1996).

use std::collections::BTreeMap;
use std::path::Path;

use ceteris_pack::econ::goods::hs_goods;
use ceteris_pack::read::baci::read_hs_cpa_version;
use ceteris_pack::read::natural_earth::{ADMIN0, Dbf, read_codes};
use ceteris_reference::io::pairwise_sum;
use rayon::prelude::*;

use ceteris_trade::{TradeParams, kappa};

use crate::costs::{
    AIR, Graph, N_TRADED, Quantities, Transport, bulk_quantities, edge_quantities, tau,
};
use crate::dta::read_dta;
use crate::gravity::{TIME_COST, pack_index};
use crate::write::Table;

/// One gateway pair of a cell: `tau(r) = beta + slope * r`, with `slope = w_g f_road q[air]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Crossing {
    pub a: usize,
    pub b: usize,
    pub beta: f64,
    pub slope: f64,
}

impl Crossing {
    fn air(&self) -> bool {
        self.slope > 0.0
    }
}

/// One (importer, origin, good) cell: origin access `v` and destination access `u` per gateway,
/// the gateway pairs, the good's theta and the data's air share of value.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub cluster: usize,
    pub good: usize,
    pub share: f64,
    pub weight: f64,
    pub theta: f64,
    pub v: Vec<f64>,
    pub u: Vec<f64>,
    pub crossings: Vec<Crossing>,
}

/// A cell's share in logs: ln S, ln (1 - S), and g = d ln N / dr, N the air part of the sum.
/// Non-air crossings do not depend on r, so dS/dr = S (1 - S) g and the quasi-likelihood's
/// score is w (s - S) g: no ratio of underflowing shares is formed.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LogShare {
    ln_s: f64,
    ln_1s: f64,
    g: f64,
}

/// ln of a sum of exp(terms) and the terms' softmax-weighted mean of `d`.
fn log_sum(terms: &[(f64, f64)]) -> (f64, f64) {
    let m = terms.iter().fold(f64::NEG_INFINITY, |a, t| a.max(t.0));
    let (mut z, mut zd) = (0.0, 0.0);
    for &(l, d) in terms {
        let e = (l - m).exp();
        z += e;
        zd += e * d;
    }
    (m + z.ln(), zd / z)
}

impl Cell {
    fn log_share(&self, r: f64) -> LogShare {
        let (mut air, mut other) = (Vec::new(), Vec::new());
        for x in &self.crossings {
            let (va, ub) = (self.v[x.a], self.u[x.b]);
            if va <= 0.0 || ub <= 0.0 {
                continue;
            }
            let tau = x.beta + x.slope * r;
            let ln = va.ln() - self.theta * tau.ln() + ub.ln();
            if x.air() {
                air.push((ln, -self.theta * x.slope / tau));
            } else {
                other.push((ln, 0.0));
            }
        }
        let (ln_n, g) = log_sum(&air);
        let (ln_m, _) = log_sum(&other);
        let hi = ln_n.max(ln_m);
        let ln_d = hi + ((ln_n - hi).exp() + (ln_m - hi).exp()).ln();
        LogShare {
            ln_s: ln_n - ln_d,
            ln_1s: ln_m - ln_d,
            g,
        }
    }

    /// The model's air share and its derivative in r.
    pub fn model_share(&self, r: f64) -> (f64, f64) {
        let l = self.log_share(r);
        let (s, s1) = (l.ln_s.exp(), l.ln_1s.exp());
        (s, s * s1 * l.g)
    }

    /// False when the share is 0 or 1 at every r: no air crossing, or no other.
    fn informative(&self) -> bool {
        let reached = |air: bool| {
            self.crossings
                .iter()
                .any(|x| x.air() == air && self.v[x.a] * self.u[x.b] > 0.0)
        };
        reached(true) && reached(false)
    }

    /// True when some crossing of the cell is reached from both ends.
    fn reached(&self) -> bool {
        self.crossings
            .iter()
            .any(|x| self.v[x.a] * self.u[x.b] > 0.0)
    }

    /// [`Self::quasi_loglik`] with ln S and ln (1 - S) at least ln [`PROFILE_CLIP`].
    fn clipped_quasi_loglik(&self, r: f64) -> f64 {
        let l = self.log_share(r);
        let floor = PROFILE_CLIP.ln();
        self.weight * (self.share * l.ln_s.max(floor) + (1.0 - self.share) * l.ln_1s.max(floor))
    }

    fn quasi_loglik(&self, r: f64) -> f64 {
        let l = self.log_share(r);
        self.weight * (self.share * l.ln_s + (1.0 - self.share) * l.ln_1s)
    }
}

fn quasi_loglik(cells: &[&Cell], r: f64) -> f64 {
    cells.iter().map(|c| c.quasi_loglik(r)).sum()
}

// ---------------------------------------------------------------- data: 2000 import shares

/// The fetched files (ADR 0277, Build), under `data/raw/air-shares-2000`.
pub const US_2000: &str = "us-census-schott/imp_detl_2000_12n.dta";
pub const US_2002: &str = "us-census-schott/imp_detl_2002_12n.dta";
pub const CENSUS_COUNTRIES: &str = "census-country.txt";
pub const EU_MODES_2002: &str = "eurostat-comext/extracted/trhs_200252.dat";
pub const EU_VALUES: &str = "eurostat-ds045409/ds045409_eu15_imports_hs6_2000_2002.csv";
const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
/// BTiGE key versions: HS 1996 (2000 data) and HS 2002 (2002 data).
pub const HS_1996: u8 = 1;
pub const HS_2002: u8 = 2;
const EU15: [&str; 15] = [
    "AT", "BE", "DE", "DK", "ES", "FI", "FR", "GB", "GR", "IE", "IT", "LU", "NL", "PT", "SE",
];
/// Comext partner codes inside intra-EU trade, dropped so totals match EXT_EU.
const INTRA_SPECIAL: [&str; 2] = ["QV", "QY"];
/// Comext transport mode: air, and not declared.
const MODE_AIR: &str = "4";
const MODE_UNDECLARED: &str = "0";
/// USD per EUR, 2000 average of the ECB reference rate, to weight EU cells beside US ones.
pub const USD_PER_EUR_2000: f64 = 0.9236;

/// Air and total import value of one (importer, origin, traded good) cell, USD.
#[derive(Clone, Debug, PartialEq)]
pub struct Obs {
    pub importer: usize,
    pub origin: usize,
    pub good: usize,
    pub air: f64,
    pub total: f64,
}

/// What a reader kept and left out, in the file's currency.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Notes {
    pub total: f64,
    pub air: f64,
    pub unmatched_country: f64,
    pub unmatched_codes: Vec<String>,
    pub unmatched_hs: f64,
    pub local_goods: f64,
    /// EU hindcast: 2000 value whose HS line has no 2002 share (the cell's share stands in),
    /// 2000 value of cells with no 2002 record (dropped), and 2002 value with no declared mode.
    pub fallback: f64,
    pub dropped: f64,
    pub undeclared: f64,
}

/// The pack's country codes, in pack order.
pub fn pack_countries(pack: &Path) -> Result<Vec<String>, String> {
    Table::open(&pack.join("countries.arrow"))?.strings("code")
}

/// Census Schedule C: country code to ISO alpha-2, from `Code | Name | ISO Code` rows.
fn census_iso2(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('|').map(str::trim).collect();
            (f.len() == 3 && f[0].len() == 4 && f[0].bytes().all(|c| c.is_ascii_digit()))
                .then(|| (f[0].to_owned(), f[2].to_owned()))
        })
        .filter(|(_, iso)| iso.len() == 2)
        .collect()
}

/// Pack country by ISO alpha-2, through Natural Earth's alpha-2 to alpha-3 codes.
fn pack_by_iso2(cache: &Path, codes: &[String]) -> Result<BTreeMap<String, usize>, String> {
    let admin0 = cache.join(ADMIN0);
    let (iso, _) = read_codes(&admin0)?;
    let index = pack_index(codes, &iso);
    Ok(Dbf::from_zip(&admin0)?
        .select(["ISO_A2_EH", "ISO_A3_EH"])?
        .into_iter()
        .filter_map(|[a2, a3]| {
            let (a2, a3) = (a2?, a3?);
            (a2 != "-99").then_some(())?;
            Some((a2.to_owned(), *index.get(a3)?))
        })
        .collect())
}

/// Traded or local good of each HS 6-digit line of the key's `version`.
fn goods_of(cache: &Path, version: u8) -> Result<BTreeMap<i64, usize>, String> {
    let key = read_hs_cpa_version(&cache.join(HS_KEY), version)?;
    Ok(hs_goods(&key)
        .into_iter()
        .filter(|(k, _)| !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()))
        .map(|(k, g)| (k.parse().expect("digits"), g))
        .collect())
}

/// Six digits as a number; None for Comext's special codes.
fn hs6(code: &str) -> Option<i64> {
    (code.len() == 6 && code.bytes().all(|c| c.is_ascii_digit())).then(|| code.parse().ok())?
}

/// US general imports of one year by origin and traded good, summed over districts: air is
/// `air_val_yr`, the total `gen_val_yr`, land and vessel the rest. HS lines go to goods by the
/// BTiGE key at `hs_version`; origins to pack countries by ISO code.
pub fn us_imports(
    data: &Path,
    file: &str,
    hs_version: u8,
    cache: &Path,
    codes: &[String],
) -> Result<(Vec<Obs>, Notes), String> {
    let path = data.join(file);
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let d = read_dta(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let numbers = |n: &str| {
        d.get(n)
            .and_then(|c| c.numbers())
            .ok_or_else(|| format!("{file}: no numeric {n}"))
    };
    let (hs10, total, air) = (
        numbers("commodity")?,
        numbers("gen_val_yr")?,
        numbers("air_val_yr")?,
    );
    let cty = d
        .get("cty_code")
        .and_then(|c| c.strings())
        .ok_or_else(|| format!("{file}: no text cty_code"))?;
    let countries_path = data.join(CENSUS_COUNTRIES);
    let text = std::fs::read_to_string(&countries_path)
        .map_err(|e| format!("{}: {e}", countries_path.display()))?;
    let iso2 = census_iso2(&text);
    let by_iso2 = pack_by_iso2(cache, codes)?;
    let us = *by_iso2.get("US").ok_or("no US in the pack")?;
    let good_of = goods_of(cache, hs_version)?;

    let mut cells: BTreeMap<(usize, usize), (f64, f64)> = BTreeMap::new();
    let mut notes = Notes::default();
    for i in 0..hs10.len() {
        let (t, a) = (total[i], air[i]);
        if t.is_nan() || t <= 0.0 {
            continue;
        }
        let a = if a.is_nan() { 0.0 } else { a };
        notes.total += t;
        notes.air += a;
        let origin = iso2.get(&cty[i]).and_then(|c| by_iso2.get(c)).copied();
        let Some(origin) = origin.filter(|&o| o != us) else {
            notes.unmatched_country += t;
            if !notes.unmatched_codes.contains(&cty[i]) {
                notes.unmatched_codes.push(cty[i].clone());
            }
            continue;
        };
        let Some(&good) = good_of.get(&((hs10[i] / 1e4).floor() as i64)) else {
            notes.unmatched_hs += t;
            continue;
        };
        if good >= N_TRADED {
            notes.local_goods += t;
            continue;
        }
        let c = cells.entry((origin, good)).or_default();
        c.0 += a;
        c.1 += t;
    }
    notes.unmatched_codes.sort();
    let obs = cells
        .into_iter()
        .map(|((origin, good), (air, total))| Obs {
            importer: us,
            origin,
            good,
            air,
            total,
        })
        .collect();
    Ok((obs, notes))
}

/// Per traded good, the log-odds change of the US air share from `later` back to `earlier`,
/// import-value weighted over the origins with that good in both years.
pub fn trend(earlier: &[Obs], later: &[Obs]) -> [f64; N_TRADED] {
    let by_cell = |obs: &[Obs]| -> BTreeMap<(usize, usize), (f64, f64)> {
        obs.iter()
            .map(|o| ((o.origin, o.good), (o.air, o.total)))
            .collect()
    };
    let (a, b) = (by_cell(earlier), by_cell(later));
    std::array::from_fn(|g| {
        let (mut sa, mut sb) = ([0.0; 2], [0.0; 2]);
        for ((o, h), x) in &a {
            if *h != g {
                continue;
            }
            if let Some(y) = b.get(&(*o, g)) {
                sa[0] += x.0;
                sa[1] += x.1;
                sb[0] += y.0;
                sb[1] += y.1;
            }
        }
        let logit = |s: [f64; 2]| (s[0] / (s[1] - s[0])).ln();
        let d = logit(sa) - logit(sb);
        if d.is_finite() { d } else { 0.0 }
    })
}

/// Comma-separated rows of `path`, header skipped, each split into fields.
fn csv_rows(path: &Path, mut f: impl FnMut(&[&str])) -> Result<(), String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut lines = std::io::BufReader::new(file).lines();
    lines.next();
    for line in lines {
        let line = line.map_err(|e| format!("{}: {e}", path.display()))?;
        let fields: Vec<&str> = line.split(',').collect();
        f(&fields);
    }
    Ok(())
}

/// EU15 imports from outside the EU15, hindcast to 2000 (ADR 0277, Amendment 2026-11-01): each
/// (member, origin, HS6) line keeps its 2002 air share of declared value, weighted by its 2000
/// value. A 2000 line with no 2002 share takes its (member, origin, good) cell's 2002 share
/// (HS 1996 and HS 2002 differ on some lines); a cell with no 2002 record is dropped. Each
/// good's shares are then shifted in log-odds by `shift` (the US change, [`trend`]). Values in
/// USD at [`USD_PER_EUR_2000`].
pub fn eu_hindcast(
    data: &Path,
    cache: &Path,
    codes: &[String],
    shift: &[f64; N_TRADED],
) -> Result<(Vec<Obs>, Notes), String> {
    let by_iso2 = pack_by_iso2(cache, codes)?;
    let member = |c: &str| EU15.contains(&c).then(|| by_iso2.get(c).copied()).flatten();
    let origin = |c: &str| -> Option<usize> {
        (c.len() == 2 && !EU15.contains(&c) && !INTRA_SPECIAL.contains(&c))
            .then(|| by_iso2.get(c).copied())
            .flatten()
    };
    let (g96, g02) = (goods_of(cache, HS_1996)?, goods_of(cache, HS_2002)?);
    let mut notes = Notes::default();

    // 2002: air and declared value by (member, origin, line) and by (member, origin, good).
    let mut line02: BTreeMap<(usize, usize, i64), (f64, f64)> = BTreeMap::new();
    let mut cell02: BTreeMap<(usize, usize, usize), (f64, f64)> = BTreeMap::new();
    csv_rows(&data.join(EU_MODES_2002), |f| {
        let [rep, par, prod, flow, mode, _, value, ..] = f else {
            return;
        };
        if *flow != "1" {
            return;
        }
        let (Some(d), Some(o), Some(k)) = (member(rep), origin(par), hs6(prod)) else {
            return;
        };
        let v: f64 = value.parse().unwrap_or(0.0);
        if *mode == MODE_UNDECLARED {
            notes.undeclared += v;
            return;
        }
        let air = if *mode == MODE_AIR { v } else { 0.0 };
        let l = line02.entry((d, o, k)).or_default();
        l.0 += air;
        l.1 += v;
        if let Some(&g) = g02.get(&k).filter(|&&g| g < N_TRADED) {
            let c = cell02.entry((d, o, g)).or_default();
            c.0 += air;
            c.1 += v;
        }
    })?;

    // 2000: value by line, and the hindcast share by cell.
    let mut cells: BTreeMap<(usize, usize, usize), (f64, f64)> = BTreeMap::new();
    csv_rows(&data.join(EU_VALUES), |f| {
        let [_, _, _, rep, par, prod, flow, ind, year, value, ..] = f else {
            return;
        };
        if *flow != "1" || *year != "2000" || *ind != "VALUE_IN_EUROS" {
            return;
        }
        let (Some(d), Some(k)) = (member(rep), hs6(prod)) else {
            return;
        };
        let v: f64 = value.parse().unwrap_or(0.0);
        if v <= 0.0 {
            return;
        }
        let Some(o) = origin(par) else {
            if par.len() == 2 && !EU15.contains(par) && !INTRA_SPECIAL.contains(par) {
                notes.unmatched_country += v;
                if !notes.unmatched_codes.iter().any(|c| c == par) {
                    notes.unmatched_codes.push((*par).to_owned());
                }
            }
            return;
        };
        notes.total += v;
        let Some(&g) = g96.get(&k) else {
            notes.unmatched_hs += v;
            return;
        };
        if g >= N_TRADED {
            notes.local_goods += v;
            return;
        }
        let share = match line02.get(&(d, o, k)).filter(|l| l.1 > 0.0) {
            Some(l) => l.0 / l.1,
            None => match cell02.get(&(d, o, g)).filter(|c| c.1 > 0.0) {
                Some(c) => {
                    notes.fallback += v;
                    c.0 / c.1
                }
                None => {
                    notes.dropped += v;
                    return;
                }
            },
        };
        let c = cells.entry((d, o, g)).or_default();
        c.0 += v * share;
        c.1 += v;
    })?;
    notes.unmatched_codes.sort();
    let obs = cells
        .into_iter()
        .map(|((importer, origin, good), (air, total))| {
            let s = air / total;
            let shifted = if s > 0.0 && s < 1.0 {
                1.0 / (1.0 + (-((s / (1.0 - s)).ln() + shift[good])).exp())
            } else {
                s
            };
            notes.air += shifted * total;
            Obs {
                importer,
                origin,
                good,
                air: shifted * total * USD_PER_EUR_2000,
                total: total * USD_PER_EUR_2000,
            }
        })
        .collect();
    Ok((obs, notes))
}

// ---------------------------------------------------------------- the pack's gateway pairs

/// Least-iceberg legs of the pack graph at one [`Transport`] (ADR 0279, whatever its
/// `path_choice`): per tree set, each province's access quantities to its country's gateways and
/// the crossing quantities between all gateways; `set[g]` is good g's set. A set is one distinct
/// (bulk rate, kappa_g) as the kernel's good legs, its trees on `c + kappa_g h` and, for a bulk
/// class, on the edge quantities at the bulk rate ([`bulk_quantities`]), so that [`tau`] at `p`
/// prices them as the kernel does.
struct Legs {
    gateways_of: Vec<Vec<usize>>,
    provinces_of: Vec<Vec<usize>>,
    set: [usize; N_TRADED],
    access: Vec<Vec<Vec<Option<Quantities>>>>,
    crossing: Vec<Vec<Vec<Option<Quantities>>>>,
    weight_out: Vec<[f64; N_TRADED]>,
    weight_in: Vec<f64>,
}

fn legs(pack: &Path, p: &Transport, weight: &[f64], trade: &TradeParams) -> Result<Legs, String> {
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
    let bulk = trade
        .bulk_rate()
        .map(|(classes, sea, port)| (classes, bulk_quantities(&mode, &q_edge, sea, port)));
    let key = |g: usize| {
        let on_bulk = bulk
            .as_ref()
            .is_some_and(|b| b.0[usize::from(trade.theta_class[g])]);
        (on_bulk, kappa(weight[g], trade.time_cost[g]).to_bits())
    };
    let mut keys: Vec<(bool, u64)> = Vec::new();
    let set = std::array::from_fn(|g| {
        let k = key(g);
        keys.iter().position(|&x| x == k).unwrap_or_else(|| {
            keys.push(k);
            keys.len() - 1
        })
    });
    let (ea, eb) = (edges.usize("a")?, edges.usize("b")?);
    let gnode = gw.usize("node")?;
    let gcountry = gw.usize("country")?;
    let provinces_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..np).filter(|&s| country[s] == c).collect())
        .collect();
    let gateways_of: Vec<Vec<usize>> = (0..nc)
        .map(|c| (0..gnode.len()).filter(|&i| gcountry[i] == c).collect())
        .collect();
    let reached = |dist: &[f64], q: &[Quantities], n: usize| dist[n].is_finite().then_some(q[n]);
    let (mut access, mut crossing) = (Vec::new(), Vec::new());
    for &(on_bulk, k) in &keys {
        let q = match &bulk {
            Some((_, b)) if on_bulk => b,
            _ => &q_edge,
        };
        let graph = Graph::by(nodes.rows(), &ea, &eb, q, p, f64::from_bits(k));
        access.push(
            (0..np)
                .into_par_iter()
                .map(|s| {
                    let (dist, q) = graph.tree(s);
                    gateways_of[country[s]]
                        .iter()
                        .map(|&g| reached(&dist, &q, gnode[g]))
                        .collect()
                })
                .collect(),
        );
        crossing.push(
            gnode
                .par_iter()
                .map(|&s| {
                    let (dist, q) = graph.tree(s);
                    gnode.iter().map(|&n| reached(&dist, &q, n)).collect()
                })
                .collect(),
        );
    }
    let weight_out = (0..np)
        .map(|s| std::array::from_fn(|g| va_cols[g][s]))
        .collect();
    let weight_in = (0..np)
        .map(|s| pairwise_sum(&(0..10).map(|g| va_cols[g][s]).collect::<Vec<_>>()))
        .collect();
    Ok(Legs {
        gateways_of,
        provinces_of,
        set,
        access,
        crossing,
        weight_out,
        weight_in,
    })
}

/// The cells of `obs` on the legs at `p`: access sums by gateway, as the kernel's
/// `v'_a` and `u'_b` without the origin price weights, with exporter weights the good's VA (all
/// VA where the country makes none of it) and importer weights all VA, as in
/// [`crate::costs::country_costs`]. Cells whose countries have no gateway are left out.
fn cells(
    l: &Legs,
    p: &Transport,
    theta: &[f64; N_TRADED],
    weight: &[f64],
    obs: &[Obs],
) -> Vec<Cell> {
    let road = p.per_tkm[0];
    let r = p.per_tkm[AIR as usize] / road;
    obs.iter()
        .filter_map(|o| {
            let g = o.good;
            let (w, lam, th) = (weight[g], TIME_COST[g], theta[g]);
            let (access, crossing) = (&l.access[l.set[g]], &l.crossing[l.set[g]]);
            let k = |q: &Option<Quantities>| q.map_or(0.0, |q| tau(&q, w, lam, p).powf(-th));
            let (po, pd) = (&l.provinces_of[o.origin], &l.provinces_of[o.importer]);
            let (go, gd) = (&l.gateways_of[o.origin], &l.gateways_of[o.importer]);
            if go.is_empty() || gd.is_empty() {
                return None;
            }
            let makes = po.iter().any(|&s| l.weight_out[s][g] > 0.0);
            let v = (0..go.len())
                .map(|a| {
                    po.iter()
                        .map(|&s| {
                            let wo = if makes {
                                l.weight_out[s][g]
                            } else {
                                l.weight_in[s]
                            };
                            wo * k(&access[s][a])
                        })
                        .sum()
                })
                .collect();
            let u = (0..gd.len())
                .map(|b| pd.iter().map(|&s| l.weight_in[s] * k(&access[s][b])).sum())
                .collect();
            let mut crossings = Vec::new();
            for (a, &ga) in go.iter().enumerate() {
                for (b, &gb) in gd.iter().enumerate() {
                    if let Some(q) = crossing[ga][gb] {
                        let slope = w * road * q[AIR as usize];
                        crossings.push(Crossing {
                            a,
                            b,
                            beta: tau(&q, w, lam, p) - slope * r,
                            slope,
                        });
                    }
                }
            }
            Some(Cell {
                cluster: o.importer * l.provinces_of.len() + o.origin,
                good: g,
                share: o.air / o.total,
                weight: o.total,
                theta: th,
                v,
                u,
                crossings,
            })
        })
        .collect()
}

/// The cells of `obs` on the per-good least-iceberg legs solved with air at `r` times road.
fn cells_at(
    pack: &Path,
    obs: &[Obs],
    theta: &[f64; N_TRADED],
    base: &Transport,
    weight: &[f64],
    r: f64,
) -> Result<Vec<Cell>, String> {
    let mut p = base.clone();
    p.per_tkm[AIR as usize] = r * p.per_tkm[0];
    let legs = legs(pack, &p, weight, &TradeParams::default())?;
    Ok(cells(&legs, &p, theta, weight, obs))
}

/// Golden-section tolerance in ln r.
pub const GOLDEN_TOL: f64 = 1e-4;
/// Bootstrap replications, their seed (core Philox streams, one per replication) and the step of
/// the fine grid they are evaluated on, in ln r (1/32 octave).
pub const BOOT_REPLICATIONS: u32 = 200;
pub const BOOT_SEED: u64 = 0x0277;
pub const FINE_STEP: f64 = std::f64::consts::LN_2 / 32.0;

/// The path-consistent estimate of r = P04d / P04a (ADR 0277 amendment).
#[derive(Clone, Debug, PartialEq)]
pub struct AirEstimate {
    pub ratio: f64,
    /// The fixed-set quasi-likelihood at `ratio`.
    pub quasi_loglik: f64,
    /// The profile grid's best point's neighbours, the golden section's bracket.
    pub bracket: (f64, f64),
    /// Paths solved: grid, golden section, fine grid and curvature points.
    pub evaluations: usize,
    /// Standard errors of ln r: clustered sandwich from the profile's curvature at `ratio`, and
    /// the cluster bootstrap's standard deviation, with its 90% percentile interval of r and the
    /// replications whose maximum fell on a bracket edge.
    pub se_ln_curvature: f64,
    pub se_ln_bootstrap: f64,
    pub interval: (f64, f64),
    pub at_edge: usize,
    /// Cells with a reached crossing, their clusters, and the informative cells at `ratio`.
    pub reached: usize,
    pub clusters: usize,
    pub informative: usize,
    /// Import-value weighted air share over the informative cells at `ratio`, model and data,
    /// overall and per good (cells, model, data).
    pub model_share: f64,
    pub data_share: f64,
    pub by_good: [(usize, f64, f64); N_TRADED],
}

/// The index of the first largest of `v`: ties go to the lowest r.
fn first_max(v: &[f64]) -> usize {
    let mut best = 0;
    for (i, &x) in v.iter().enumerate() {
        if x > v[best] {
            best = i;
        }
    }
    best
}

/// Each cluster's fixed-set quasi-likelihood (clipped shares, every cell with a reached
/// crossing), in the order of `clusters`.
fn by_cluster(cells: &[Cell], r: f64, clusters: &[usize]) -> Vec<f64> {
    let mut q = vec![0.0; clusters.len()];
    for c in cells.iter().filter(|c| c.reached()) {
        let i = clusters
            .binary_search(&c.cluster)
            .expect("a reached cluster");
        q[i] += c.clipped_quasi_loglik(r);
    }
    q
}

/// The value-weighted shares of `cells` at `r` over the informative cells.
fn shares_at(cells: &[Cell], r: f64) -> (usize, f64, f64, [(usize, f64, f64); N_TRADED]) {
    let mean = |used: &[&Cell]| {
        let total: f64 = used.iter().map(|c| c.weight).sum();
        let model = used
            .iter()
            .map(|c| c.weight * c.model_share(r).0)
            .sum::<f64>()
            / total;
        let data = used.iter().map(|c| c.weight * c.share).sum::<f64>() / total;
        (model, data)
    };
    let used: Vec<&Cell> = cells.iter().filter(|c| c.informative()).collect();
    let (model, data) = mean(&used);
    let by_good = std::array::from_fn(|g| {
        let of: Vec<&Cell> = used.iter().copied().filter(|c| c.good == g).collect();
        let (m, d) = mean(&of);
        (of.len(), m, d)
    });
    (used.len(), model, data, by_good)
}

/// Maximises the fixed-set quasi-likelihood over ln r with the paths solved again at every r
/// (`cells_at`): the best point of `grid` (ties to the lowest r; a maximum at the grid's edge
/// fails), golden section in ln r between its neighbours to [`GOLDEN_TOL`] in a fixed number of
/// steps, then the errors. The bootstrap resamples (importer, origin) clusters with replacement
/// and maximises each replication on a fine grid across the bracket ([`FINE_STEP`]; a cell's share
/// at r does not depend on the resample), refined by a parabola through the best point and its
/// neighbours. Fails when r does not exceed 1 (DESIGN 5.3, road < air).
pub fn profile_fit(
    mut cells_at: impl FnMut(f64) -> Result<Vec<Cell>, String>,
    grid: &[f64],
    seed: u64,
) -> Result<AirEstimate, String> {
    let first = cells_at(grid[0])?;
    let mut clusters: Vec<usize> = first
        .iter()
        .filter(|c| c.reached())
        .map(|c| c.cluster)
        .collect();
    clusters.sort_unstable();
    clusters.dedup();
    if clusters.is_empty() {
        return Err("air share: no cell has a reached crossing".into());
    }
    let reached = first.iter().filter(|c| c.reached()).count();
    // the first grid point is solved above
    let mut evaluations = 1;
    let mut eval = |x: f64| -> Result<Vec<f64>, String> {
        evaluations += 1;
        Ok(by_cluster(&cells_at(x.exp())?, x.exp(), &clusters))
    };
    let total = |q: &[f64]| q.iter().sum::<f64>();

    let mut q_grid = vec![total(&by_cluster(&first, grid[0], &clusters))];
    for &r in &grid[1..] {
        q_grid.push(total(&eval(r.ln())?));
    }
    let best = first_max(&q_grid);
    if best == 0 || best + 1 == grid.len() {
        return Err(format!(
            "air share: the profile's maximum is at the grid's edge, r {}",
            grid[best]
        ));
    }
    let (lo, hi) = (grid[best - 1].ln(), grid[best + 1].ln());

    // golden section; equal values keep the lower point
    let inv = (5f64.sqrt() - 1.0) / 2.0;
    let steps = ((GOLDEN_TOL / (hi - lo)).ln() / inv.ln()).ceil() as usize;
    let (mut a, mut b) = (lo, hi);
    let (mut c, mut d) = (b - inv * (b - a), a + inv * (b - a));
    let (mut fc, mut fd) = (total(&eval(c)?), total(&eval(d)?));
    for _ in 0..steps {
        if fc >= fd {
            (b, d, fd) = (d, c, fc);
            c = b - inv * (b - a);
            fc = total(&eval(c)?);
        } else {
            (a, c, fc) = (c, d, fd);
            d = a + inv * (b - a);
            fd = total(&eval(d)?);
        }
    }
    // the profile is piecewise where paths switch: keep the grid's best if it is higher
    let mut candidates = [(c, fc), (d, fd), (grid[best].ln(), q_grid[best])];
    candidates.sort_by(|x, y| x.0.total_cmp(&y.0));
    let qs: Vec<f64> = candidates.iter().map(|x| x.1).collect();
    let (x_hat, q_hat) = candidates[first_max(&qs)];

    // clustered sandwich from the profile's curvature at x_hat, over one fine step
    let h = FINE_STEP;
    let (qm, q0, qp) = (eval(x_hat - h)?, eval(x_hat)?, eval(x_hat + h)?);
    let curvature = -(total(&qp) - 2.0 * total(&q0) + total(&qm)) / (h * h);
    let meat: f64 = qp
        .iter()
        .zip(&qm)
        .map(|(p, m)| ((p - m) / (2.0 * h)).powi(2))
        .sum();
    let se_ln_curvature = meat.sqrt() / curvature;

    // the bootstrap on the fine grid across the bracket
    let points = ((hi - lo) / h).round() as usize;
    let fine: Vec<Vec<f64>> = (0..=points)
        .map(|k| eval(lo + k as f64 * h))
        .collect::<Result<_, _>>()?;
    let n = clusters.len();
    let mut draws: Vec<f64> = Vec::with_capacity(BOOT_REPLICATIONS as usize);
    let mut at_edge = 0;
    for rep in 0..BOOT_REPLICATIONS {
        let mut s = ceteris_core::rng::Seed(seed).stream(
            0,
            ceteris_core::schedule::SystemId(0),
            ceteris_core::entity::Kind(0),
            ceteris_core::entity::EntityId(rep),
        );
        let mut m = vec![0u32; n];
        for _ in 0..n {
            m[((s.uniform() * n as f64) as usize).min(n - 1)] += 1;
        }
        let f: Vec<f64> = fine
            .iter()
            .map(|q| q.iter().zip(&m).map(|(q, &m)| q * f64::from(m)).sum())
            .collect();
        let j = first_max(&f);
        let x = if j == 0 || j == points {
            at_edge += 1;
            lo + j as f64 * h
        } else {
            let den = f[j - 1] - 2.0 * f[j] + f[j + 1];
            let shift = if den < 0.0 {
                0.5 * (f[j - 1] - f[j + 1]) / den
            } else {
                0.0
            };
            lo + (j as f64 + shift) * h
        };
        draws.push(x);
    }
    let mean = draws.iter().sum::<f64>() / draws.len() as f64;
    let var = draws.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (draws.len() - 1) as f64;
    let mut sorted = draws.clone();
    sorted.sort_by(f64::total_cmp);
    let pct = |p: f64| sorted[(p * (sorted.len() - 1) as f64).round() as usize].exp();

    let ratio = x_hat.exp();
    let at_hat = cells_at(ratio)?;
    evaluations += 1;
    let (informative, model_share, data_share, by_good) = shares_at(&at_hat, ratio);
    if ratio <= 1.0 {
        return Err(format!(
            "P04d: air / road {ratio} does not exceed 1 (road < air)"
        ));
    }
    Ok(AirEstimate {
        ratio,
        quasi_loglik: q_hat,
        bracket: (lo.exp(), hi.exp()),
        evaluations,
        se_ln_curvature,
        se_ln_bootstrap: var.sqrt(),
        interval: (pct(0.05), pct(0.95)),
        at_edge,
        reached,
        clusters: n,
        informative,
        model_share,
        data_share,
        by_good,
    })
}

/// [`profile_fit`] on the pack: the cells of `obs` on the per-good least-iceberg legs (ADR
/// 0279) at `TradeParams::default()`'s time costs and bulk rate, over [`profile_grid`], with
/// [`BOOT_SEED`].
pub fn estimate(
    pack: &Path,
    obs: &[Obs],
    theta: &[f64; N_TRADED],
    base: &Transport,
) -> Result<AirEstimate, String> {
    let weight = Table::open(&pack.join("econ_goods.arrow"))?.f64("weight")?;
    profile_fit(
        |r| cells_at(pack, obs, theta, base, &weight, r),
        &profile_grid(),
        BOOT_SEED,
    )
}

/// The share clip of [`ProfilePoint::quasi_loglik_reached`].
pub const PROFILE_CLIP: f64 = 1e-12;

/// The profile grid of r = P04d / P04a: 25 points from 0.5 to 32, a quarter octave apart.
pub fn profile_grid() -> [f64; 25] {
    std::array::from_fn(|i| 0.5 * 2f64.powf(i as f64 / 4.0))
}

/// One point of [`profile`]: at r, on the paths solved at r.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfilePoint {
    pub r: f64,
    /// Import-value weighted air share over every cell with a reached crossing (a cell without an
    /// air crossing counts 0, one without another 1), model and data.
    pub model_all: f64,
    pub data_all: f64,
    /// The same over the informative cells, as the fit weighs them.
    pub model_used: f64,
    pub data_used: f64,
    /// The quasi-likelihood over the informative cells, a set that changes with r.
    pub quasi_loglik: f64,
    /// The quasi-likelihood over every cell with a reached crossing, a set fixed across the grid
    /// (reachability does not depend on r), with the model's shares clipped to
    /// [`PROFILE_CLIP`, 1 - `PROFILE_CLIP`] so a cell at 0 or 1 stays finite.
    pub quasi_loglik_reached: f64,
    /// Per good: (informative cells, model share, data share) over the informative cells.
    pub by_good: [(usize, f64, f64); N_TRADED],
}

/// A diagnostic profile (ADR 0277): at every point of [`profile_grid`] the per-good
/// least-iceberg legs are solved with air at r times road, and the model's shares and the
/// quasi-likelihood are evaluated on them, in grid order.
pub fn profile(
    pack: &Path,
    obs: &[Obs],
    theta: &[f64; N_TRADED],
    base: &Transport,
) -> Result<Vec<ProfilePoint>, String> {
    let weight = Table::open(&pack.join("econ_goods.arrow"))?.f64("weight")?;
    profile_grid()
        .into_iter()
        .map(|r| {
            let cs = cells_at(pack, obs, theta, base, &weight, r)?;
            let mean = |used: &[&Cell]| {
                let total: f64 = used.iter().map(|c| c.weight).sum();
                let model = used
                    .iter()
                    .map(|c| c.weight * c.model_share(r).0)
                    .sum::<f64>()
                    / total;
                let data = used.iter().map(|c| c.weight * c.share).sum::<f64>() / total;
                (model, data)
            };
            let all: Vec<&Cell> = cs.iter().filter(|c| c.reached()).collect();
            let used: Vec<&Cell> = cs.iter().filter(|c| c.informative()).collect();
            let (model_all, data_all) = mean(&all);
            let (model_used, data_used) = mean(&used);
            let by_good = std::array::from_fn(|g| {
                let of: Vec<&Cell> = used.iter().copied().filter(|c| c.good == g).collect();
                let (m, d) = mean(&of);
                (of.len(), m, d)
            });
            Ok(ProfilePoint {
                r,
                model_all,
                data_all,
                model_used,
                data_used,
                quasi_loglik: quasi_loglik(&used, r),
                quasi_loglik_reached: all.iter().map(|c| c.clipped_quasi_loglik(r)).sum(),
                by_good,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trend_is_the_log_odds_change_over_shared_origins() {
        let o = |origin, good, air, total| Obs {
            importer: 0,
            origin,
            good,
            air,
            total,
        };
        let earlier = [
            o(1, 4, 30.0, 100.0),
            o(2, 4, 10.0, 100.0),
            o(3, 4, 99.0, 100.0),
        ];
        let later = [o(1, 4, 20.0, 100.0), o(2, 4, 20.0, 100.0)];
        let t = trend(&earlier, &later);
        // Shared origins 1 and 2: 40 of 200 in 2000, 40 of 200 in 2002.
        assert!(t[4].abs() < 1e-15);
        assert_eq!(t[0], 0.0);
        let later = [o(1, 4, 10.0, 100.0), o(2, 4, 10.0, 100.0)];
        let t = trend(&earlier, &later);
        assert!((t[4] - ((40.0_f64 / 160.0).ln() - (20.0_f64 / 180.0).ln())).abs() < 1e-12);
    }

    #[test]
    fn an_underflowing_air_share_stays_finite() {
        // theta 60 and an air tau near 1e6: the air term is about 1e-360, below f64.
        let mut c = cell(0, 9000.0, 2e-4, 60.0);
        c.crossings[2].beta = 1e6;
        c.share = 0.2;
        let (s, ds) = c.model_share(3.0);
        assert!(s == 0.0 && ds == 0.0);
        assert!(c.clipped_quasi_loglik(3.0).is_finite());
        assert!(c.quasi_loglik(3.0).is_finite());
    }

    #[test]
    fn profile_grid_spans_half_to_thirty_two() {
        let g = profile_grid();
        assert_eq!((g[0], g[24]), (0.5, 32.0));
        assert!(
            g.windows(2)
                .all(|w| (w[1] / w[0] - 2f64.powf(0.25)).abs() < 1e-12)
        );
    }

    #[test]
    fn six_digit_lines_only() {
        assert_eq!(hs6("010110"), Some(10110));
        assert_eq!(hs6("0101X"), None);
        assert_eq!(hs6("01011"), None);
    }

    #[test]
    fn reads_schedule_c_rows() {
        let text = "Produced: 31JAN14\nCode | Name | ISO Code\n5700 | China | CN\n1220 | Canada | CA\n0001 | Bad |\n";
        let m = census_iso2(text);
        assert_eq!(m.get("5700").map(String::as_str), Some("CN"));
        assert_eq!(m.len(), 2);
    }

    /// Two origin and two destination gateways; crossings 0-0 and 1-1 by sea, 0-1 by air.
    fn cell(cluster: usize, km: f64, w: f64, theta: f64) -> Cell {
        let sea = 1.0 + w * (16.0 + 0.0121 * km) + 0.002 * km / 25.0 / 24.0;
        let air_beta = 1.0 + w * 120.0 + 0.002 * (km / 500.0 + 24.0) / 24.0;
        Cell {
            cluster,
            good: 0,
            share: 0.0,
            weight: 1.0 + km / 1e4,
            theta,
            v: vec![0.7, 0.3],
            u: vec![0.4, 0.6],
            crossings: vec![
                Crossing {
                    a: 0,
                    b: 0,
                    beta: sea,
                    slope: 0.0,
                },
                Crossing {
                    a: 1,
                    b: 1,
                    beta: sea * 1.1,
                    slope: 0.0,
                },
                Crossing {
                    a: 0,
                    b: 1,
                    beta: air_beta,
                    slope: w * 0.10 * 1.05 * km,
                },
            ],
        }
    }

    fn sample(r: f64) -> Vec<Cell> {
        let mut out = Vec::new();
        for (i, km) in [800.0, 2500.0, 6000.0, 9000.0, 14000.0]
            .into_iter()
            .enumerate()
        {
            for (j, (w, theta)) in [(2e-4, 4.0), (5e-5, 8.0), (1e-3, 2.9)]
                .into_iter()
                .enumerate()
            {
                let mut c = cell(i, km, w, theta);
                c.share = c.model_share(r).0;
                c.weight *= 1.0 + j as f64;
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn share_is_the_gateway_pair_sum() {
        // Province level: K_oa and K_bd for 3 origins and 2 destinations, then the full sum.
        let k_oa = [[0.5, 0.1], [0.2, 0.3], [0.05, 0.4]];
        let k_bd = [[0.6, 0.2], [0.1, 0.7]];
        let (wo, wd) = ([0.5, 0.3, 0.2], [0.4, 0.6]);
        let mut c = cell(0, 6000.0, 2e-4, 4.0);
        c.v = (0..2)
            .map(|a| (0..3).map(|o| wo[o] * k_oa[o][a]).sum())
            .collect();
        c.u = (0..2)
            .map(|b| (0..2).map(|d| wd[d] * k_bd[b][d]).sum())
            .collect();
        let r = 3.0;
        let (mut air, mut all) = (0.0, 0.0);
        for o in 0..3 {
            for d in 0..2 {
                for x in &c.crossings {
                    let k = wo[o]
                        * wd[d]
                        * k_oa[o][x.a]
                        * (x.beta + x.slope * r).powf(-4.0)
                        * k_bd[x.b][d];
                    all += k;
                    if x.slope > 0.0 {
                        air += k;
                    }
                }
            }
        }
        assert!((c.model_share(r).0 - air / all).abs() <= 1e-12);
    }

    #[test]
    fn derivative_matches_a_central_difference() {
        let c = cell(0, 9000.0, 2e-4, 4.0);
        let (r, h) = (3.0, 1e-5);
        let fd = (c.model_share(r + h).0 - c.model_share(r - h).0) / (2.0 * h);
        let d = c.model_share(r).1;
        assert!((fd - d).abs() <= 1e-6 * d.abs(), "{fd} {d}");
    }

    #[test]
    fn share_falls_as_air_gets_dearer() {
        let c = cell(0, 6000.0, 2e-4, 4.0);
        let s: Vec<f64> = [1.0, 2.0, 4.0, 8.0]
            .iter()
            .map(|&r| c.model_share(r).0)
            .collect();
        assert!(s.windows(2).all(|p| p[1] < p[0]), "{s:?}");
    }

    #[test]
    fn a_common_border_factor_cancels() {
        let c = cell(0, 6000.0, 2e-4, 4.0);
        let mut scaled = c.clone();
        // (1 + nu)(1 + t) on every crossing multiplies each K_ab by the same factor.
        let f = 1.3_f64;
        for x in &mut scaled.crossings {
            x.beta *= f;
            x.slope *= f;
        }
        assert!((c.model_share(2.0).0 - scaled.model_share(2.0).0).abs() <= 1e-15);
    }

    /// [`sample`]'s cells with paths that switch with r: cell i's air crossing reroutes over
    /// land once air costs more than `switch[i]` times road, at the factor air has there, so
    /// the set of air crossings changes along the profile.
    fn switching(r: f64, at: f64) -> Vec<Cell> {
        let switch = [1.8, 2.4, 2.9, 3.3, 3.8, 4.6, 6.0];
        let mut out = sample(at);
        for (i, c) in out.iter_mut().enumerate() {
            let s = switch[i % switch.len()];
            let x = &mut c.crossings[2];
            if r > s {
                (x.beta, x.slope) = (x.beta + x.slope * s, 0.0);
            }
        }
        out
    }

    /// Cells of [`switching`] at r with the data's shares from the model on the paths at `truth`.
    fn switching_world(r: f64, truth: f64) -> Vec<Cell> {
        let data = switching(truth, truth);
        let mut cs = switching(r, truth);
        for (c, d) in cs.iter_mut().zip(&data) {
            c.share = d.model_share(truth).0;
        }
        cs
    }

    #[test]
    fn profile_recovers_a_known_ratio() {
        let est = profile_fit(|_| Ok(sample(2.4)), &profile_grid(), BOOT_SEED).unwrap();
        assert!(
            (est.ratio.ln() - 2.4f64.ln()).abs() < 2.0 * GOLDEN_TOL,
            "{est:?}"
        );
        assert!(est.se_ln_bootstrap < 1e-2, "exact shares: {est:?}");
        assert!(
            est.interval.0 <= 2.4 * 1.01 && est.interval.1 >= 2.4 / 1.01,
            "{est:?}"
        );
        assert_eq!((est.reached, est.informative, est.clusters), (15, 15, 5));
    }

    /// The paths switch with r: on the paths at r* = 3 every cell's share is exact, and the
    /// profile, solving the paths at each r, finds r* although several cells lose their air
    /// crossing on either side of it.
    #[test]
    fn profile_recovers_r_when_paths_switch() {
        let truth = 3.0;
        let air_at = |r: f64| {
            switching(r, truth)
                .iter()
                .filter(|c| c.crossings[2].slope > 0.0)
                .count()
        };
        assert!(air_at(2.0) > air_at(truth) && air_at(truth) > air_at(5.0));
        let est = profile_fit(
            |r| Ok(switching_world(r, truth)),
            &profile_grid(),
            BOOT_SEED,
        )
        .unwrap();
        assert!(
            (est.ratio.ln() - truth.ln()).abs() < 2.0 * GOLDEN_TOL,
            "{est:?}"
        );
    }

    #[test]
    fn bootstrap_is_deterministic() {
        let mut cs = switching_world(3.0, 3.0);
        // noise, so that resamples disagree
        for (i, c) in cs.iter_mut().enumerate() {
            c.share = (c.share * (1.0 + 0.3 * ((i * 7 % 5) as f64 - 2.0) / 2.0)).clamp(0.0, 1.0);
        }
        let run = || {
            profile_fit(
                |r| {
                    let mut x = switching_world(r, 3.0);
                    for (c, d) in x.iter_mut().zip(&cs) {
                        c.share = d.share;
                    }
                    Ok(x)
                },
                &profile_grid(),
                BOOT_SEED,
            )
            .unwrap()
        };
        let (a, b) = (run(), run());
        // goods without cells carry NaN shares, so compare the printed values
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
        assert!(
            a.se_ln_bootstrap > 0.0 && a.interval.0 < a.interval.1,
            "{a:?}"
        );
        let other = profile_fit(
            |r| {
                let mut x = switching_world(r, 3.0);
                for (c, d) in x.iter_mut().zip(&cs) {
                    c.share = d.share;
                }
                Ok(x)
            },
            &profile_grid(),
            BOOT_SEED + 1,
        )
        .unwrap();
        assert_eq!(other.ratio, a.ratio);
        assert_ne!(other.se_ln_bootstrap, a.se_ln_bootstrap);
    }

    #[test]
    fn ties_go_to_the_lowest_r_and_an_edge_maximum_fails() {
        assert_eq!(first_max(&[1.0, 3.0, 3.0, 2.0]), 1);
        assert_eq!(first_max(&[2.0, 2.0]), 0);
        // shares the cells cannot move: a flat profile ties everywhere, so its maximum is the
        // first point, at the edge
        let mut flat = sample(2.4);
        for c in &mut flat {
            c.crossings[2].slope = 0.0;
        }
        let e = profile_fit(|_| Ok(flat.clone()), &profile_grid(), BOOT_SEED).unwrap_err();
        assert!(e.contains("grid's edge, r 0.5"), "{e}");
    }

    #[test]
    fn a_ratio_below_road_fails() {
        let e = profile_fit(|_| Ok(sample(0.6)), &profile_grid(), BOOT_SEED).unwrap_err();
        assert!(e.contains("does not exceed 1"), "{e}");
    }

    #[test]
    fn cells_without_both_modes_are_counted_not_fitted() {
        let mut cells = sample(2.4);
        let mut no_air = cell(9, 6000.0, 2e-4, 4.0);
        no_air.crossings.pop();
        no_air.share = 0.3;
        cells.push(no_air);
        let est = profile_fit(|_| Ok(cells.clone()), &profile_grid(), BOOT_SEED).unwrap();
        assert_eq!((est.reached, est.informative), (16, 15));
    }
}
