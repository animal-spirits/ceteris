//! 2000 population, education, labour and Gini (specs/econ/SPEC.md, Pack additions; ADR 0014).
//!
//! Provinces get WorldPop 2000 summed through the H3 cells, scaled to the WDI national totals
//! (UN WPP's where WDI has none).
//! Countries get education shares (WIC, else Barro-Lee), participation, unemployment and the
//! natural rate u* (ILOSTAT; ADR 0089), the working-age share (WDI), the labour share (PWT) and
//! the Gini (SWIID, else WDI; ADR 0098). A country with no value from any source takes the value
//! of its nearest donor (same World Bank region, closest log GDP per capita) and is flagged
//! imputed.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array, UInt32Array,
};

use crate::numeric::{median, pairwise_sum, py_sum};
use crate::read::people::{N_EDU, Observed, parts};
use crate::read::wdi::Names;
use crate::table;

/// Where a country's Gini comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiniSource {
    Swiid,
    Wdi,
    Donor,
}

impl GiniSource {
    pub fn as_str(self) -> &'static str {
        match self {
            GiniSource::Swiid => "swiid",
            GiniSource::Wdi => "wdi",
            GiniSource::Donor => "donor",
        }
    }
}

/// One country's 2000 labour inputs; each `*_imputed` flag is set when the value is a donor's or
/// a fallback source's.
#[derive(Clone, Debug, PartialEq)]
pub struct Labour {
    /// Labour force over the population aged 15 to 64.
    pub participation: f64,
    /// Unemployed over labour force.
    pub unemployment: f64,
    /// Natural rate: mean unemployment rate over `U_STAR_YEARS`.
    pub u_star: f64,
    /// Shares of the working-age population: below secondary, secondary, tertiary.
    pub edu_shares: [f64; N_EDU],
    /// Compensation of employees over GDP.
    pub labour_share: f64,
    /// Population aged 15 to 64 over the whole population.
    pub working_age_share: f64,
    /// Fraction in [0, 1].
    pub gini: f64,
    pub gini_source: GiniSource,
    pub participation_imputed: bool,
    pub unemployment_imputed: bool,
    pub u_star_imputed: bool,
    pub edu_imputed: bool,
    pub labour_share_imputed: bool,
    pub working_age_imputed: bool,
    pub gini_imputed: bool,
}

/// A value that split economies combine by a population-weighted mean.
trait Weighted: Copy {
    const ZERO: Self;
    fn add(self, other: Self) -> Self;
    fn mul(self, w: f64) -> Self;
    fn div(self, w: f64) -> Self;
}

impl Weighted for f64 {
    const ZERO: Self = 0.0;
    fn add(self, other: Self) -> Self {
        self + other
    }
    fn mul(self, w: f64) -> Self {
        self * w
    }
    fn div(self, w: f64) -> Self {
        self / w
    }
}

impl Weighted for [f64; N_EDU] {
    const ZERO: Self = [0.0; N_EDU];
    fn add(self, other: Self) -> Self {
        std::array::from_fn(|i| self[i] + other[i])
    }
    fn mul(self, w: f64) -> Self {
        self.map(|x| x * w)
    }
    fn div(self, w: f64) -> Self {
        self.map(|x| x / w)
    }
}

/// The single value itself, else the sum of value times weight in order over the sum of weights.
fn weighted<T: Weighted>(values: &[T], weights: &[f64]) -> T {
    if let [one] = values {
        return *one;
    }
    let total = values
        .iter()
        .zip(weights)
        .fold(T::ZERO, |acc, (v, w)| acc.add(v.mul(*w)));
    total.div(pairwise_sum(weights))
}

/// Observed economy (a key of `observed`, not in `exclude`, with a GDP per person) in `region`
/// with the closest log GDP per person, the earlier code on a tie. Without `log_income` the
/// target is the median of the candidates; without a candidate in `region`, every candidate counts.
pub fn donor<T>(
    region: Option<&str>,
    log_income: Option<f64>,
    observed: &BTreeMap<String, T>,
    obs: &Observed,
    exclude: &[String],
) -> Result<String, String> {
    let pool: Vec<&String> = observed
        .keys()
        .filter(|k| !exclude.contains(k) && obs.gdp_per_capita.contains_key(*k))
        .collect();
    let near: Vec<&String> = match region.filter(|r| !r.is_empty()) {
        Some(r) => pool
            .iter()
            .copied()
            .filter(|k| obs.region.get(*k).map(String::as_str) == Some(r))
            .collect(),
        None => Vec::new(),
    };
    let pool = if near.is_empty() { pool } else { near };
    let logs: Vec<f64> = pool
        .iter()
        .map(|k| libm::log(obs.gdp_per_capita[*k]))
        .collect();
    let target = match log_income {
        Some(x) => x,
        None => median(logs.clone()).ok_or("no donor candidates")?,
    };
    let mut best: Option<(f64, &String)> = None;
    for (k, l) in pool.iter().zip(&logs) {
        let d = (l - target).abs();
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, k));
        }
    }
    best.map(|(_, k)| k.clone())
        .ok_or_else(|| "no donor candidates".to_owned())
}

/// Lookups of one pack country's source economies in the observed series.
struct Country<'a> {
    parts: Vec<String>,
    sovereign_parts: Vec<String>,
    obs: &'a Observed,
}

impl Country<'_> {
    fn lookup<T: Weighted>(&self, series: &BTreeMap<String, T>) -> Option<T> {
        let found: Vec<&String> = self
            .parts
            .iter()
            .filter(|p| series.contains_key(*p))
            .collect();
        if found.is_empty() {
            return None;
        }
        let values: Vec<T> = found.iter().map(|p| series[*p]).collect();
        let weights: Vec<f64> = found
            .iter()
            .map(|p| self.obs.population.get(*p).copied().unwrap_or(1.0))
            .collect();
        Some(weighted(&values, &weights))
    }

    fn region(&self) -> Option<&str> {
        self.parts
            .iter()
            .chain(&self.sovereign_parts)
            .find_map(|p| self.obs.region.get(p))
            .map(String::as_str)
    }

    /// The value of the first series that has the country, with that series' index, else the
    /// donor's value (the earlier series winning for a code in several) with index `series.len()`.
    fn value<T: Weighted>(&self, series: &[&BTreeMap<String, T>]) -> Result<(T, usize), String> {
        for (i, s) in series.iter().enumerate() {
            if let Some(v) = self.lookup(s) {
                return Ok((v, i));
            }
        }
        let mut merged: BTreeMap<String, T> = BTreeMap::new();
        for s in series.iter().rev() {
            merged.extend(s.iter().map(|(k, v)| (k.clone(), *v)));
        }
        let income = self.lookup(&self.obs.gdp_per_capita).map(libm::log);
        let d = donor(self.region(), income, &merged, self.obs, &self.parts)?;
        Ok((merged[&d], series.len()))
    }
}

/// Labour inputs for each pack country (`(code, sovereign code)`), from data where any source has
/// the country.
///
/// A country the sources split (`PARTS`) takes the population-weighted mean of its parts.
/// Education comes from WIC, else Barro-Lee. The Gini comes from SWIID, else WDI (flagged
/// imputed), else a donor (flagged imputed).
pub fn country_labour(
    countries: &[(String, String)],
    obs: &Observed,
    iso: &Names,
) -> Result<BTreeMap<String, Labour>, String> {
    let mut out = BTreeMap::new();
    for (code, sovereign) in countries {
        let c = Country {
            parts: parts(code, iso),
            sovereign_parts: parts(sovereign, iso),
            obs,
        };
        let (participation, p_i) = c.value(&[&obs.participation])?;
        let (unemployment, u_i) = c.value(&[&obs.unemployment])?;
        let (u_star, s_i) = c.value(&[&obs.natural_unemployment])?;
        let (working_age, w_i) = c.value(&[&obs.working_age])?;
        let (labour_share, l_i) = c.value(&[&obs.labour_share])?;
        let (edu, e_i) = c.value::<[f64; N_EDU]>(&[&obs.edu_wic, &obs.edu_barro_lee])?;
        let (gini, g_i) = c.value(&[&obs.gini_swiid, &obs.gini_wdi])?;
        let edu_total = pairwise_sum(&edu);
        out.insert(
            code.clone(),
            Labour {
                participation,
                unemployment,
                u_star,
                edu_shares: edu.map(|x| x / edu_total),
                labour_share,
                working_age_share: working_age,
                gini,
                gini_source: [GiniSource::Swiid, GiniSource::Wdi, GiniSource::Donor][g_i],
                participation_imputed: p_i == 1,
                unemployment_imputed: u_i == 1,
                u_star_imputed: s_i == 1,
                edu_imputed: e_i == 2,
                labour_share_imputed: l_i == 1,
                working_age_imputed: w_i == 1,
                gini_imputed: g_i > 0,
            },
        );
    }
    Ok(out)
}

fn rows<'a>(
    codes: &[String],
    labour: &'a BTreeMap<String, Labour>,
) -> Result<Vec<&'a Labour>, String> {
    codes
        .iter()
        .map(|c| {
            labour
                .get(c)
                .ok_or_else(|| format!("no labour inputs for {c}"))
        })
        .collect()
}

fn floats(values: impl IntoIterator<Item = f64>) -> ArrayRef {
    Arc::new(values.into_iter().collect::<Float64Array>())
}

fn flags(values: impl IntoIterator<Item = bool>) -> ArrayRef {
    Arc::new(values.into_iter().map(Some).collect::<BooleanArray>())
}

/// `econ_countries.arrow` columns of the labour inputs, in `codes` order.
pub fn country_columns(
    codes: &[String],
    labour: &BTreeMap<String, Labour>,
) -> Result<Vec<(String, ArrayRef)>, String> {
    let rows = rows(codes, labour)?;
    let column = |f: fn(&Labour) -> f64| floats(rows.iter().map(|r| f(r)));
    let flag = |f: fn(&Labour) -> bool| flags(rows.iter().map(|r| f(r)));
    let mut out = vec![
        ("participation", column(|r| r.participation)),
        ("unemployment", column(|r| r.unemployment)),
        ("u_star", column(|r| r.u_star)),
        ("working_age_share", column(|r| r.working_age_share)),
        ("labour_share", column(|r| r.labour_share)),
        ("gini", column(|r| r.gini)),
        ("edu_low", column(|r| r.edu_shares[0])),
        ("edu_secondary", column(|r| r.edu_shares[1])),
        ("edu_tertiary", column(|r| r.edu_shares[2])),
    ];
    out.extend([
        ("participation_imputed", flag(|r| r.participation_imputed)),
        ("unemployment_imputed", flag(|r| r.unemployment_imputed)),
        ("u_star_imputed", flag(|r| r.u_star_imputed)),
        ("working_age_imputed", flag(|r| r.working_age_imputed)),
        ("labour_share_imputed", flag(|r| r.labour_share_imputed)),
        ("edu_imputed", flag(|r| r.edu_imputed)),
        ("gini_imputed", flag(|r| r.gini_imputed)),
    ]);
    out.push((
        "gini_source",
        Arc::new(StringArray::from_iter_values(
            rows.iter().map(|r| r.gini_source.as_str()),
        )),
    ));
    Ok(out.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

/// WDI population in 2000 (UN World Population Prospects) of each pack country, summing split
/// economies, else the UN WPP total (WDI leaves out Western Sahara); countries without a positive
/// total in either are left out.
pub fn national_population(codes: &[String], obs: &Observed, iso: &Names) -> BTreeMap<String, f64> {
    let total = |series: &BTreeMap<String, f64>, c: &str| {
        py_sum(
            parts(c, iso)
                .iter()
                .map(|p| series.get(p).copied().unwrap_or(0.0)),
        )
    };
    codes
        .iter()
        .filter_map(|c| {
            let wdi = total(&obs.population, c);
            let t = if wdi > 0.0 {
                wdi
            } else {
                total(&obs.population_wpp, c)
            };
            (t > 0.0).then(|| (c.clone(), t))
        })
        .collect()
}

/// Scales each country's provinces to its national total, keeping their shares (WorldPop's UN
/// adjustment).
///
/// WorldPop 2000 differs from the UN estimates by -12 to +38% for large countries, and H3 cell
/// ownership moves a few percent across borders. A country without a total, or without people in
/// WorldPop, is unchanged.
pub fn scale_to_national(
    population: &[f64],
    country: &[String],
    totals: &BTreeMap<String, f64>,
) -> Vec<f64> {
    let mut members: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, c) in country.iter().enumerate() {
        members.entry(c).or_default().push(i);
    }
    let mut out = population.to_vec();
    for (c, here) in members {
        let have = pairwise_sum(&here.iter().map(|&i| population[i]).collect::<Vec<_>>());
        if let Some(total) = totals.get(c)
            && have > 0.0
        {
            let factor = total / have;
            for i in here {
                out[i] *= factor;
            }
        }
    }
    out
}

/// `econ_provinces.arrow` columns of the people: population and working-age population (15 to 64).
pub fn province_columns(
    country: &[String],
    population: &[f64],
    labour: &BTreeMap<String, Labour>,
) -> Result<Vec<(String, ArrayRef)>, String> {
    let rows = rows(country, labour)?;
    Ok(vec![
        ("population".into(), floats(population.iter().copied())),
        (
            "working_age".into(),
            floats(
                population
                    .iter()
                    .zip(&rows)
                    .map(|(p, r)| p * r.working_age_share),
            ),
        ),
        (
            "working_age_imputed".into(),
            flags(rows.iter().map(|r| r.working_age_imputed)),
        ),
    ])
}

/// Pops: province-major, three education rows each.
///
/// A province splits its people (children included) by its country's education shares of the
/// working-age population; its working-age people and labour force follow. The row is imputed
/// when any share it uses is.
pub fn pops(
    country: &[String],
    population: &[f64],
    labour: &BTreeMap<String, Labour>,
) -> Result<RecordBatch, String> {
    let rows = rows(country, labour)?;
    let n = rows.len();
    let (mut people, mut adults, mut force) = (
        Vec::with_capacity(n * N_EDU),
        Vec::with_capacity(n * N_EDU),
        Vec::with_capacity(n * N_EDU),
    );
    for (p, r) in population.iter().zip(&rows) {
        let working_age = p * r.working_age_share;
        for share in r.edu_shares {
            people.push(p * share);
            adults.push(working_age * share);
            force.push(working_age * share * r.participation);
        }
    }
    let province = u32::try_from(n).map_err(|_| "too many provinces for uint32")?;
    let columns: Vec<(&str, ArrayRef)> = vec![
        (
            "province",
            Arc::new(UInt32Array::from_iter_values(
                (0..province).flat_map(|i| [i; N_EDU]),
            )),
        ),
        (
            "edu",
            Arc::new(UInt8Array::from_iter_values(
                (0..n).flat_map(|_| 0..N_EDU as u8),
            )),
        ),
        ("population", floats(people)),
        ("working_age", floats(adults)),
        ("labour_force", floats(force)),
        (
            "imputed",
            flags(rows.iter().flat_map(|r| {
                [r.edu_imputed || r.working_age_imputed || r.participation_imputed; N_EDU]
            })),
        ),
    ];
    table::batch(
        columns
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect(),
    )
}
