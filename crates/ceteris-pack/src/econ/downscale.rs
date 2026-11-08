//! Province value added by good, downscaled from national VA (specs/econ/SPEC.md, Downscaling VA
//! to provinces; ADR 0014).
//!
//! Each country's VA of a good is split over its provinces by the first proxy of the good's chain
//! that has a positive total in the country's eligible provinces (at least `MIN_WORKING_AGE`
//! working-age people):
//!
//! - agriculture: MapSPAM 2000 physical area of all crops (cropland), then population;
//! - crude oil and gas: in the United States, EIA 2000 state production (`eia_oil_gas`);
//!   elsewhere GEM oil and gas fields; then population. GEM is flagged imputed where undated
//!   fields carry most of a country's weight;
//! - coal and other mining: GEM coal mines and P1 mineral sites (weighted by the country's coal
//!   share of the good), then population; without the GEM file, mineral sites alone;
//! - manufacturing (3 to 6): DOSE manufacturing GVA of the regions matched to provinces by exact
//!   name, then gridded GDP, then population;
//! - local goods (7 to 9): population, a base point only.
//!
//! Provinces below the working-age floor get no VA; theirs follows the same proxy over the other
//! provinces. A country with no eligible province puts all its VA in its province with the most
//! working-age people. A row is imputed when a fallback placed it or the national VA is itself
//! imputed (va_basis other than icio).
//!
//! MapSPAM 2000 has no value of production, so cropland stands in for it. DOSE regions carry GADM
//! GID codes and no ISO 3166-2 codes, so they are matched by name only (ADR 0005).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use icu_casemap::CaseMapper;
use icu_normalizer::ComposingNormalizer;

use super::goods::NAMES;
use super::icio::{Basis, Economy};
use crate::numeric::{pairwise_sum, py_sum};
use crate::raster::geotiff;
use crate::raster::population::{BLOCK_ROWS, CellOwner, province_population, spam_physical_area};
use crate::read::downscale::{
    Asset, DOSE, Dose, SEDS, read_dose, read_gem_coal, read_gem_oil_gas, read_seds,
};
use crate::read::icio::N_GOODS;
use crate::read::natural_earth::{province_names, us_states};
use crate::read::wdi::Names;
use crate::read::{WINDOW, YEAR};

/// Provinces with fewer working-age people get no VA.
pub const MIN_WORKING_AGE: f64 = 1000.0;
pub const SPAM: &str = "spam2000v3.0.7_global_physical-area.geotiff.zip";
pub const KUMMU: &str = "kummu_gdp_ppp_2000_5arcmin.tif";
pub use crate::read::natural_earth::ADMIN1;
/// GEM coal mines and oil and gas fields.
pub const GEM: [&str; 2] = [
    "Global Coal Mine Tracker, August 2026.xlsx",
    "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx",
];
/// SEDS federal offshore areas and the states whose coasts they lie off.
pub const OFFSHORE: [(&str, &[&str]); 2] =
    [("X3", &["TX", "LA", "MS", "AL", "FL"]), ("X5", &["CA"])];
pub const RESOURCES: [&str; 5] = ["precious", "base", "ferrous", "energy", "industrial"];
/// GEM oil and gas is flagged imputed in a country where fields with no start or discovery year
/// carry more than this share of its weight.
pub const MAX_UNDATED: f64 = 0.5;
pub const MANUFACTURING: [usize; 4] = [3, 4, 5, 6];

/// The proxy that placed a province's VA of a good; the discriminant is the `va_level_*` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    Crops,
    Gem,
    Mining,
    Sites,
    Dose,
    DoseRest,
    Gdp,
    Population,
    Largest,
    Eia,
}

pub const LEVELS: [Level; 10] = [
    Level::Crops,
    Level::Gem,
    Level::Mining,
    Level::Sites,
    Level::Dose,
    Level::DoseRest,
    Level::Gdp,
    Level::Population,
    Level::Largest,
    Level::Eia,
];

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Crops => "crops",
            Level::Gem => "gem",
            Level::Mining => "mining",
            Level::Sites => "sites",
            Level::Dose => "dose",
            Level::DoseRest => "dose_rest",
            Level::Gdp => "gdp",
            Level::Population => "population",
            Level::Largest => "largest",
            Level::Eia => "eia",
        }
    }
}

pub struct Provinces {
    /// Pack country code per province.
    pub country: Vec<String>,
    pub population: Vec<f64>,
    /// People aged 15 to 64.
    pub working_age: Vec<f64>,
}

/// Province proxies; None marks a source that is not available.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Proxies {
    /// MapSPAM 2000 physical area of all crops (ha).
    pub crops: Option<Vec<f64>>,
    /// GEM oil and gas fields (million boe per year).
    pub oil_gas: Option<Vec<f64>>,
    /// The part of `oil_gas` from fields with no start or discovery year.
    pub oil_gas_undated: Option<Vec<f64>>,
    /// EIA 2000 crude oil and marketed gas (billion Btu), United States provinces only.
    pub eia: Option<Vec<f64>>,
    /// GEM coal mines (Mt per year).
    pub coal: Option<Vec<f64>>,
    /// P1 producing and past-producing mineral sites (USGS MRDS).
    pub sites: Option<Vec<f64>>,
    /// DOSE manufacturing GVA of matched regions, NaN for provinces no region matches.
    pub dose: Option<Vec<f64>>,
    /// By country: DOSE share in regions without a province.
    pub dose_rest: BTreeMap<String, f64>,
    /// Kummu gridded GDP 2000 (PPP).
    pub gdp: Option<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Downscaled {
    pub country: Vec<String>,
    /// Million USD.
    pub va: Vec<[f64; N_GOODS]>,
    pub imputed: Vec<[bool; N_GOODS]>,
    pub level: Vec<[Level; N_GOODS]>,
    /// Countries with VA but no province.
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoseMatch {
    pub matched: usize,
    pub regions: usize,
    /// Share of the DOSE manufacturing GVA in matched regions.
    pub gva_share: f64,
    /// Matched and listed regions by ISO code.
    pub by_country: BTreeMap<String, (usize, usize)>,
}

impl DoseMatch {
    pub fn rate(&self) -> f64 {
        if self.regions == 0 {
            0.0
        } else {
            self.matched as f64 / self.regions as f64
        }
    }
}

/// One proxy's weights over a country's eligible provinces, their imputed flags and levels.
struct Weights {
    w: Vec<f64>,
    imputed: Vec<bool>,
    level: Vec<Level>,
}

impl Weights {
    fn uniform(w: Vec<f64>, imputed: bool, level: Level) -> Self {
        let n = w.len();
        Weights {
            w,
            imputed: vec![imputed; n],
            level: vec![level; n],
        }
    }
}

#[derive(Clone, Copy)]
enum Step {
    /// The proxy of the level, its rows flagged imputed or not.
    Plain(Level, bool),
    Gem,
    Mining,
    Dose,
}

const FALLBACK: Step = Step::Plain(Level::Population, true);
const BASE: Step = Step::Plain(Level::Population, false);
const MAKE: &[Step] = &[Step::Dose, Step::Plain(Level::Gdp, true), FALLBACK];
const CHAINS: [&[Step]; N_GOODS] = [
    &[Step::Plain(Level::Crops, false), FALLBACK],
    &[Step::Plain(Level::Eia, false), Step::Gem, FALLBACK],
    &[Step::Mining, FALLBACK],
    MAKE,
    MAKE,
    MAKE,
    MAKE,
    &[BASE],
    &[BASE],
    &[BASE],
];

/// Above zero; false for NaN, as Python's `not x > 0` reads it.
fn positive(x: f64) -> bool {
    x > 0.0
}

fn gather(x: &[f64], idx: &[usize]) -> Vec<f64> {
    idx.iter().map(|&i| x[i]).collect()
}

struct Chains<'a> {
    p: &'a Proxies,
    population: &'a [f64],
}

impl Chains<'_> {
    fn source(&self, level: Level) -> Option<&[f64]> {
        match level {
            Level::Crops => self.p.crops.as_deref(),
            Level::Eia => self.p.eia.as_deref(),
            Level::Sites => self.p.sites.as_deref(),
            Level::Gdp => self.p.gdp.as_deref(),
            Level::Population => Some(self.population),
            _ => None,
        }
    }

    fn weights(&self, step: Step, idx: &[usize], code: &str, e: &Economy) -> Option<Weights> {
        match step {
            Step::Plain(level, imputed) => self.plain(level, imputed, idx),
            Step::Gem => self.gem(idx),
            Step::Mining => self.mining(idx, e.table.coal_share),
            Step::Dose => self.dose(idx, code),
        }
    }

    fn plain(&self, level: Level, imputed: bool, idx: &[usize]) -> Option<Weights> {
        let w = gather(self.source(level)?, idx);
        (pairwise_sum(&w) > 0.0).then(|| Weights::uniform(w, imputed, level))
    }

    fn gem(&self, idx: &[usize]) -> Option<Weights> {
        let w = gather(self.p.oil_gas.as_deref()?, idx);
        let total = pairwise_sum(&w);
        if !positive(total) {
            return None;
        }
        let undated = self
            .p
            .oil_gas_undated
            .as_deref()
            .is_some_and(|u| pairwise_sum(&gather(u, idx)) > MAX_UNDATED * total);
        Some(Weights::uniform(w, undated, Level::Gem))
    }

    fn mining(&self, idx: &[usize], coal_share: f64) -> Option<Weights> {
        let Some(coal) = self.p.coal.as_deref() else {
            return self.plain(Level::Sites, true, idx);
        };
        let coal = gather(coal, idx);
        let sites = self.p.sites.as_deref().map(|s| gather(s, idx));
        let coal_sum = pairwise_sum(&coal);
        let sites_sum = sites.as_deref().map_or(0.0, pairwise_sum);
        let w = match sites {
            Some(sites) if coal_sum > 0.0 && sites_sum > 0.0 => coal
                .iter()
                .zip(&sites)
                .map(|(c, s)| coal_share * c / coal_sum + (1.0 - coal_share) * s / sites_sum)
                .collect(),
            _ if coal_sum > 0.0 => coal,
            Some(sites) if sites_sum > 0.0 => sites,
            _ => return None,
        };
        Some(Weights::uniform(w, false, Level::Mining))
    }

    /// DOSE shares for matched provinces; the share of unmatched regions goes to the unmatched
    /// provinces by gridded GDP (else population).
    fn dose(&self, idx: &[usize], code: &str) -> Option<Weights> {
        let d = gather(self.p.dose.as_deref()?, idx);
        let matched: Vec<bool> = d.iter().map(|x| x.is_finite()).collect();
        let on: Vec<f64> = d.iter().copied().filter(|x| x.is_finite()).collect();
        let on_sum = pairwise_sum(&on);
        if on.is_empty() || !positive(on_sum) {
            return None;
        }
        let rest = self.p.dose_rest.get(code).copied().unwrap_or(0.0);
        let others: Vec<usize> = idx
            .iter()
            .zip(&matched)
            .filter(|(_, m)| !**m)
            .map(|(i, _)| *i)
            .collect();
        let mut w = vec![0.0; idx.len()];
        if rest > 0.0 && !others.is_empty() {
            let sub = match self.p.gdp.as_deref().map(|g| gather(g, &others)) {
                Some(g) if pairwise_sum(&g) > 0.0 => g,
                _ => gather(self.population, &others),
            };
            let sub_sum = pairwise_sum(&sub);
            let mut sub = sub.into_iter();
            for (k, &m) in matched.iter().enumerate() {
                w[k] = if m {
                    (1.0 - rest) * d[k] / on_sum
                } else {
                    rest * sub.next().unwrap_or(0.0) / sub_sum
                };
            }
        } else {
            for (k, &m) in matched.iter().enumerate() {
                if m {
                    w[k] = d[k];
                }
            }
        }
        Some(Weights {
            w,
            imputed: matched.iter().map(|m| !m).collect(),
            level: matched
                .iter()
                .map(|&m| if m { Level::Dose } else { Level::DoseRest })
                .collect(),
        })
    }
}

/// VA by province and good with its imputed flag and proxy level (see the module notes).
pub fn allocate(
    country_io: &BTreeMap<String, Economy>,
    provinces: &Provinces,
    proxies: &Proxies,
) -> Result<Downscaled, String> {
    let country = &provinces.country;
    let n = country.len();
    let mut va = vec![[0.0; N_GOODS]; n];
    let mut imputed = vec![[false; N_GOODS]; n];
    let mut level = vec![[Level::Population; N_GOODS]; n];
    let chains = Chains {
        p: proxies,
        population: &provinces.population,
    };
    let codes: BTreeSet<&str> = country.iter().map(String::as_str).collect();
    for &code in &codes {
        let Some(e) = country_io.get(code) else {
            continue;
        };
        let here: Vec<usize> = (0..n).filter(|&i| country[i] == code).collect();
        for &i in &here {
            imputed[i] = [e.va_basis != Basis::Icio; N_GOODS];
        }
        let idx: Vec<usize> = here
            .iter()
            .copied()
            .filter(|&i| provinces.working_age[i] >= MIN_WORKING_AGE)
            .collect();
        if idx.is_empty() {
            let mut k = here[0];
            for &i in &here {
                if provinces.working_age[i] > provinces.working_age[k] {
                    k = i;
                }
            }
            va[k] = e.table.va;
            imputed[k] = [true; N_GOODS];
            level[k] = [Level::Largest; N_GOODS];
            continue;
        }
        for (g, chain) in CHAINS.iter().enumerate() {
            let w = chain
                .iter()
                .find_map(|&step| chains.weights(step, &idx, code, e))
                .ok_or_else(|| format!("{code}: no proxy of good {g} has a positive total"))?;
            let total = pairwise_sum(&w.w);
            for (k, &i) in idx.iter().enumerate() {
                va[i][g] = e.table.va[g] * w.w[k] / total;
                imputed[i][g] |= w.imputed[k];
                level[i][g] = w.level[k];
            }
        }
    }
    let dropped = country_io
        .iter()
        .filter(|(k, e)| !codes.contains(k.as_str()) && pairwise_sum(&e.table.va) > 0.0)
        .map(|(k, _)| k.clone())
        .collect();
    Ok(Downscaled {
        country: country.clone(),
        va,
        imputed,
        level,
        dropped,
    })
}

/// Per good: share of the world's VA of that good placed by each proxy level, in `LEVELS` order.
pub fn coverage(out: &Downscaled) -> Vec<Vec<(Level, f64)>> {
    (0..N_GOODS)
        .map(|g| {
            let column: Vec<f64> = out.va.iter().map(|v| v[g]).collect();
            let total = pairwise_sum(&column);
            if !positive(total) {
                return Vec::new();
            }
            let mut by = [0.0; LEVELS.len()];
            for (v, l) in column.iter().zip(&out.level) {
                by[l[g] as usize] += v;
            }
            LEVELS
                .iter()
                .zip(by)
                .filter(|(_, v)| *v > 0.0)
                .map(|(l, v)| (*l, v / total))
                .collect()
        })
        .collect()
}

fn percent(x: f64) -> String {
    format!("{:.1}%", x * 100.0)
}

/// The proxy coverage by good, the DOSE match rate and the countries with VA but no province.
pub fn report(out: &Downscaled, m: &DoseMatch) -> String {
    let mut lines = vec!["Proxy coverage (share of world VA by good):".to_owned()];
    for (g, mut shares) in coverage(out).into_iter().enumerate() {
        shares.sort_by(|a, b| b.1.total_cmp(&a.1));
        let parts: Vec<String> = shares
            .iter()
            .map(|(l, v)| format!("{} {}", l.name(), percent(*v)))
            .collect();
        lines.push(format!("  {}: {}", NAMES[g], parts.join(", ")));
    }
    lines.push(format!(
        "DOSE regions matched: {} of {} ({}); {} of their manufacturing GVA",
        m.matched,
        m.regions,
        percent(m.rate()),
        percent(m.gva_share)
    ));
    if !out.dropped.is_empty() {
        lines.push(format!(
            "Countries with VA but no province: {}",
            out.dropped.join(", ")
        ));
    }
    lines.join("\n")
}

/// Python's `str.isspace`: Unicode whitespace and the four information separators.
fn is_py_space(c: char) -> bool {
    c.is_whitespace() || ('\x1c'..='\x1f').contains(&c)
}

/// A name as matching compares it: NFC, case-folded, without surrounding whitespace.
fn key(name: &str) -> String {
    let nfc = ComposingNormalizer::new_nfc().normalize(name);
    CaseMapper::new()
        .fold_string(&nfc)
        .trim_matches(is_py_space)
        .to_owned()
}

/// Natural Earth adm1_code of a province, without the ".k" of a split region's parts.
pub fn base(province_id: &str) -> &str {
    match province_id.rsplit_once('.') {
        Some((head, tail))
            if !head.is_empty() && !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) =>
        {
            head
        }
        _ => province_id,
    }
}

/// Province DOSE proxy (NaN where no region matches), the unmatched share by pack country, and
/// the match counts.
///
/// A DOSE region matches the one Natural Earth region of its country (`iso`: pack code to ISO)
/// whose primary name equals it, ignoring case; else the one with any name equal to it. A name
/// shared by two regions matches neither, and a region takes one DOSE region at most. The parts
/// of a split region share its value by `weight`. The unmatched share counts the regions DOSE
/// lacks at their population share (`Dose::coverage`).
pub fn match_dose(
    dose: &BTreeMap<String, Dose>,
    ids: &[String],
    country: &[String],
    names: &BTreeMap<String, Vec<String>>,
    weight: &[f64],
    iso: &Names,
) -> (Vec<f64>, BTreeMap<String, f64>, DoseMatch) {
    let iso_of = |c: &str| iso.get(c).cloned().unwrap_or_else(|| c.to_owned());
    let mut proxy = vec![f64::NAN; ids.len()];
    // Provinces by ISO code and base region, in province order.
    let mut bases: BTreeMap<String, Vec<(String, Vec<usize>)>> = BTreeMap::new();
    let mut pack_code: BTreeMap<String, String> = BTreeMap::new();
    for (i, (pid, c)) in ids.iter().zip(country).enumerate() {
        let own = bases.entry(iso_of(c)).or_default();
        let b = base(pid);
        match own.iter_mut().find(|(x, _)| x == b) {
            Some((_, members)) => members.push(i),
            None => own.push((b.to_owned(), vec![i])),
        }
        pack_code.insert(iso_of(c), c.clone());
    }
    let keys: BTreeMap<&str, Vec<String>> = names
        .iter()
        .map(|(b, n)| (b.as_str(), n.iter().map(|x| key(x)).collect()))
        .collect();
    let mut rest = BTreeMap::new();
    let mut by_country = BTreeMap::new();
    let (mut matched_gva, mut total_gva) = (0.0, 0.0);
    let (mut matched, mut regions) = (0, 0);
    for (code, d) in dose {
        let values = &d.values;
        regions += values.len();
        total_gva += py_sum(values.iter().map(|(_, v)| *v));
        let Some(own) = bases.get(code) else {
            by_country.insert(code.clone(), (0, values.len()));
            continue;
        };
        let mut sorted: Vec<&(String, f64)> = values.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        // (region, its value, the base region it matched), in Python's insertion order.
        let mut found: Vec<(&str, f64, &str)> = Vec::new();
        for primary in [true, false] {
            for (region, value) in &sorted {
                if found.iter().any(|f| f.0 == region) {
                    continue;
                }
                let k = key(region);
                let hits: Vec<&str> = own
                    .iter()
                    .map(|(b, _)| b.as_str())
                    .filter(|b| {
                        let all = keys.get(b).map_or(&[][..], Vec::as_slice);
                        let compared = if primary {
                            &all[..all.len().min(1)]
                        } else {
                            all
                        };
                        compared.contains(&k)
                    })
                    .collect();
                if let [hit] = hits[..]
                    && !found.iter().any(|f| f.2 == hit)
                {
                    found.push((region.as_str(), *value, hit));
                }
            }
        }
        for (_, value, b) in &found {
            let members = &own
                .iter()
                .find(|(x, _)| x == b)
                .expect("a found base is one of the country's")
                .1;
            let w = gather(weight, members);
            let total = pairwise_sum(&w);
            for (k, &i) in members.iter().enumerate() {
                proxy[i] = if total > 0.0 {
                    value * (w[k] / total)
                } else {
                    value * (1.0 / members.len() as f64)
                };
            }
        }
        let got = py_sum(found.iter().map(|f| f.1));
        let total = py_sum(values.iter().map(|(_, v)| *v));
        rest.insert(
            pack_code[code].clone(),
            if total > 0.0 {
                1.0 - d.coverage * got / total
            } else {
                0.0
            },
        );
        matched += found.len();
        matched_gva += got;
        by_country.insert(code.clone(), (found.len(), values.len()));
    }
    let gva_share = if total_gva > 0.0 {
        matched_gva / total_gva
    } else {
        0.0
    };
    (
        proxy,
        rest,
        DoseMatch {
            matched,
            regions,
            gva_share,
            by_country,
        },
    )
}

/// Weights of points summed by the province owning their cell; points with no owner are dropped.
pub fn point_sums<'a>(
    points: impl IntoIterator<Item = &'a Asset>,
    owner: &mut CellOwner,
    n: usize,
) -> Result<Vec<f64>, String> {
    let mut out = vec![0.0; n];
    for a in points {
        let k = owner.of_point(a.lat, a.lon)?;
        if k >= 0 {
            let slot = out
                .get_mut(k as usize)
                .ok_or_else(|| format!("cell owner {k} is not one of {n} provinces"))?;
            *slot += a.weight;
        }
    }
    Ok(out)
}

/// The provinces table columns the proxies read.
pub struct Geography {
    pub id: Vec<String>,
    pub country: Vec<String>,
    pub coastal: Vec<bool>,
    pub lon: Vec<f64>,
    pub lat: Vec<f64>,
    /// Mineral site counts by resource group, in `RESOURCES` order.
    pub res: [Vec<u32>; RESOURCES.len()],
}

fn add_shares(out: &mut [f64], idx: &[usize], value: f64, w: &[f64]) {
    let total = pairwise_sum(w);
    for (k, &i) in idx.iter().enumerate() {
        out[i] += value * w[k] / total;
    }
}

/// US province weights for crude oil and gas from EIA 2000 state production, combined by heat
/// content (Btu).
///
/// A state's own production is spread over its provinces by the GEM weight of the onshore fields
/// in them, else by population. Federal offshore production (`OFFSHORE`) goes to the coastal
/// provinces of the adjacent states: each US offshore GEM field in the area adds its weight to
/// the nearest of them; without such fields, by population.
pub fn eia_oil_gas(
    seds: &BTreeMap<String, f64>,
    geo: &Geography,
    states: &BTreeMap<String, String>,
    owner: &mut CellOwner,
    population: &[f64],
    assets: Option<&[Asset]>,
) -> Result<Vec<f64>, String> {
    let n = geo.id.len();
    let state: Vec<&str> = geo
        .id
        .iter()
        .zip(&geo.country)
        .map(|(id, c)| {
            if c == "USA" {
                states.get(base(id)).map_or("", String::as_str)
            } else {
                ""
            }
        })
        .collect();
    let us: Vec<&Asset> = assets
        .unwrap_or_default()
        .iter()
        .filter(|a| a.country == "United States")
        .collect();
    let onshore = point_sums(us.iter().copied().filter(|a| !a.offshore), owner, n)?;
    let mut out = vec![0.0; n];
    for (code, &value) in seds {
        let idx: Vec<usize> = (0..n).filter(|&i| state[i] == code).collect();
        if OFFSHORE.iter().any(|(area, _)| area == code) || value <= 0.0 || idx.is_empty() {
            continue;
        }
        let on = gather(&onshore, &idx);
        let w = if pairwise_sum(&on) > 0.0 {
            on
        } else {
            gather(population, &idx)
        };
        add_shares(&mut out, &idx, value, &w);
    }
    for (area, coast) in OFFSHORE {
        let idx: Vec<usize> = (0..n)
            .filter(|&i| geo.coastal[i] && coast.contains(&state[i]))
            .collect();
        let value = seds.get(area).copied().unwrap_or(0.0);
        if value <= 0.0 || idx.is_empty() {
            continue;
        }
        let mut w = vec![0.0; idx.len()];
        for a in &us {
            // Gulf of Mexico east of 100 W; the Pacific area off California south of 50 N
            // (Alaska is a state's own).
            let inside = if area == "X3" {
                a.lon > -100.0
            } else {
                a.lon <= -100.0 && a.lat < 50.0
            };
            if !(a.offshore && inside) {
                continue;
            }
            let cos = libm::cos(a.lat.to_radians());
            let mut best = (f64::INFINITY, 0);
            for (k, &i) in idx.iter().enumerate() {
                let (dy, dx) = (geo.lat[i] - a.lat, (geo.lon[i] - a.lon) * cos);
                let d = dy * dy + dx * dx;
                if d < best.0 {
                    best = (d, k);
                }
            }
            w[best.1] += a.weight;
        }
        if !positive(pairwise_sum(&w)) {
            w = gather(population, &idx);
        }
        add_shares(&mut out, &idx, value, &w);
    }
    Ok(out)
}

/// P1 producing and past-producing mineral sites per province, all resource groups.
pub fn mineral_sites(res: &[Vec<u32>]) -> Vec<f64> {
    let n = res.first().map_or(0, Vec::len);
    (0..n)
        .map(|i| res.iter().fold(0.0, |s, r| s + f64::from(r[i])))
        .collect()
}

/// MapSPAM physical area summed over its crops (all technologies), per province.
pub fn cropland(spam: &Path, owner: &mut CellOwner) -> Result<Vec<f64>, String> {
    province_population(&mut spam_physical_area(spam)?, owner, BLOCK_ROWS)
}

/// Kummu et al. gridded GDP (PPP, constant 2011 international dollars) of 2000 per province.
pub fn gridded_gdp(path: &Path, owner: &mut CellOwner) -> Result<Vec<f64>, String> {
    province_population(&mut geotiff::open(path)?, owner, BLOCK_ROWS)
}

/// Every proxy from the fetch cache and the P1 tables; `national` is the 2000 population by ISO
/// code (WDI), `gem` is (coal mines, oil and gas fields) or None. Gridded GDP is used when its
/// file is in the cache (it is a manual, optional download).
pub fn load_proxies(
    cache: &Path,
    geo: &Geography,
    owner: &mut CellOwner,
    population: &[f64],
    national: &BTreeMap<String, f64>,
    iso: &Names,
    gem: Option<(PathBuf, PathBuf)>,
) -> Result<(Proxies, DoseMatch), String> {
    let n = geo.id.len();
    let crops = cropland(&cache.join(SPAM), owner)?;
    let kummu = cache.join(KUMMU);
    let gdp = if kummu.exists() {
        Some(gridded_gdp(&kummu, owner)?)
    } else {
        None
    };
    let oil = gem
        .as_ref()
        .map(|(_, oil)| read_gem_oil_gas(oil))
        .transpose()?;
    let split = gdp.as_deref().unwrap_or(population);
    let (dose, dose_rest, matched) = match_dose(
        &read_dose(&cache.join(DOSE), national, YEAR, WINDOW)?,
        &geo.id,
        &geo.country,
        &province_names(&cache.join(ADMIN1))?,
        split,
        iso,
    );
    let (oil_gas, oil_gas_undated) = match &oil {
        Some(oil) => (
            Some(point_sums(oil, owner, n)?),
            Some(point_sums(oil.iter().filter(|a| !a.dated), owner, n)?),
        ),
        None => (None, None),
    };
    let eia = eia_oil_gas(
        &read_seds(&cache.join(SEDS), YEAR)?,
        geo,
        &us_states(&cache.join(ADMIN1))?,
        owner,
        population,
        oil.as_deref(),
    )?;
    let coal = match &gem {
        Some((coal, _)) => Some(point_sums(&read_gem_coal(coal)?, owner, n)?),
        None => None,
    };
    let proxies = Proxies {
        crops: Some(crops),
        oil_gas,
        oil_gas_undated,
        eia: Some(eia),
        coal,
        sites: Some(mineral_sites(&geo.res)),
        dose: Some(dose),
        dose_rest,
        gdp,
    };
    Ok((proxies, matched))
}
