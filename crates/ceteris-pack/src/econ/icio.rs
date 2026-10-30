//! Country input-output tables for the 10 goods: the ICIO 2000 economies as read, every other
//! pack country imputed from the nearest ICIO economy (specs/econ/SPEC.md, Economies outside
//! ICIO; ADR 0014).

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, StringArray, UInt8Array, UInt32Array};

use super::people::donor;
use crate::numeric::{median, pairwise_sum, py_sum};
use crate::read::icio::{CountryIo, N_GOODS, Wdi};
use crate::read::people::{Observed, parts};
use crate::read::wdi::Names;

/// The ICIO package of the 2000 tables in the fetch cache.
pub const ICIO: &str = "1995-2000_SML.zip";
/// WDI sector groups as goods: agriculture; industry other than manufacturing (mining,
/// utilities, construction); manufacturing; services.
pub const GROUPS: [&[usize]; 4] = [&[0], &[1, 2, 7, 8], &[3, 4, 5, 6], &[9]];
/// Territories whose people and product WDI counts in the parent's totals, by pack code
/// (territory, parent).
pub const INSIDE: [(&str, &str); 3] = [("SOL", "SOM"), ("ALD", "FIN"), ("CYN", "CYP")];
/// World Bank regions for pack countries the World Bank does not classify (and whose sovereign
/// it does not either).
pub const REGIONS: [(&str, &str); 3] = [
    ("SAH", "Middle East & North Africa"),
    ("CYN", "Europe & Central Asia"),
    ("SOL", "Sub-Saharan Africa"),
];

/// Where a country's VA level and split come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Icio,
    /// WDI GDP and all four sector shares.
    Wdi,
    /// WDI GDP times the shares it has, the donor's split for the rest.
    WdiGdp,
    /// Population times the donor's VA per person, scaled by income when a source exists.
    Donor,
    /// A territory's population share of its parent.
    Parent,
    /// No GDP, no people.
    None,
}

impl Basis {
    pub fn as_str(self) -> &'static str {
        match self {
            Basis::Icio => "icio",
            Basis::Wdi => "wdi",
            Basis::WdiGdp => "wdi_gdp",
            Basis::Donor => "donor",
            Basis::Parent => "parent",
            Basis::None => "none",
        }
    }
}

/// The income that ranked the donors and scaled a donor-basis level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncomeSource {
    Pwt,
    /// WDI GDP per person, nearest year in 1995 to 2010.
    Wdi,
    /// ADB Key Indicators GDP per person.
    Adb,
    /// Maddison Project GDP per person, 2011 international dollars.
    Maddison,
    /// Region median donor, donor's level unscaled.
    None,
}

impl IncomeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            IncomeSource::Pwt => "pwt",
            IncomeSource::Wdi => "wdi",
            IncomeSource::Adb => "adb",
            IncomeSource::Maddison => "maddison",
            IncomeSource::None => "none",
        }
    }
}

/// One pack country's 2000 structure by good and where it comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct Economy {
    pub table: CountryIo,
    /// ICIO economy an imputed country copies; None for ICIO economies.
    pub donor: Option<String>,
    pub va_basis: Basis,
    /// World Bank region the donor was looked up in.
    pub region: Option<String>,
    pub income_source: IncomeSource,
    /// Share of this economy's own VA given to territories counted inside it.
    pub shed: f64,
}

impl Economy {
    fn own(table: CountryIo) -> Self {
        Economy {
            table,
            donor: None,
            va_basis: Basis::Icio,
            region: None,
            income_source: IncomeSource::None,
            shed: 0.0,
        }
    }

    fn scaled(&self, f: f64) -> Self {
        let mut out = self.clone();
        out.table.output = self.table.output.map(|x| x * f);
        out.table.va = self.table.va.map(|x| x * f);
        out
    }
}

/// `np.mean` of a list.
fn mean(values: &[f64]) -> f64 {
    pairwise_sum(values) / values.len() as f64
}

/// Median over the ICIO economies with WDI GDP of their VA (including product taxes) over GDP.
pub fn median_va_over_gdp(icio: &BTreeMap<String, CountryIo>, wdi: &Wdi) -> f64 {
    median(
        icio.iter()
            .filter_map(|(k, c)| Some(pairwise_sum(&c.va) * 1e6 / wdi.gdp.get(k)?))
            .collect(),
    )
    .unwrap_or(f64::NAN)
}

/// ICIO crude oil and gas VA over WDI oil and gas rents, summed over the ICIO economies with GDP
/// and a rent.
pub fn oil_gas_va_over_rents(icio: &BTreeMap<String, CountryIo>, wdi: &Wdi) -> f64 {
    let (va, rents): (Vec<f64>, Vec<f64>) = icio
        .iter()
        .filter_map(|(k, c)| {
            let r = rents(wdi, k)?;
            Some((c.va[OIL_GAS] * 1e6, wdi.gdp.get(k)? * r))
        })
        .unzip();
    pairwise_sum(&va) / pairwise_sum(&rents)
}

/// The largest share of crude oil and gas in the VA of goods 1 to 8 among the ICIO economies.
pub fn oil_gas_most_of_industry(icio: &BTreeMap<String, CountryIo>) -> f64 {
    icio.values()
        .map(|c| c.va[OIL_GAS] / pairwise_sum(&c.va[1..9]))
        .filter(|x| x.is_finite())
        .fold(0.0, f64::max)
}

/// Oil plus natural gas rents over GDP, with a missing one as zero; None when both are missing.
fn rents(wdi: &Wdi, code: &str) -> Option<f64> {
    match (wdi.oil_rents.get(code), wdi.gas_rents.get(code)) {
        (None, None) => None,
        (o, g) => Some(o.unwrap_or(&0.0) + g.unwrap_or(&0.0)),
    }
}

/// A country's VA shares of GDP: agriculture, industry, manufacturing, services; and its oil and
/// gas rents over GDP.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SectorShares {
    pub agriculture: Option<f64>,
    pub industry: Option<f64>,
    pub manufacturing: Option<f64>,
    pub services: Option<f64>,
    pub rents: Option<f64>,
}

/// GDP (USD) of the parts and their GDP-weighted VA shares of GDP.
///
/// A share is None when any part with GDP lacks it. All-zero shares (WDI carries zeros for
/// Venezuela in 2000) count as missing, and so does manufacturing above industry.
pub fn sector_shares(wdi: &Wdi, ps: &[String]) -> (f64, SectorShares) {
    let have: Vec<&String> = ps.iter().filter(|p| wdi.gdp.contains_key(*p)).collect();
    let gdp = py_sum(have.iter().map(|p| wdi.gdp[*p]));
    let weights: Vec<f64> = have.iter().map(|p| wdi.gdp[*p]).collect();
    let weighted = |value: &dyn Fn(&str) -> Option<f64>| -> Option<f64> {
        if have.is_empty() {
            return None;
        }
        let values: Option<Vec<f64>> = have.iter().map(|p| value(p)).collect();
        let products: Vec<f64> = values?.iter().zip(&weights).map(|(v, w)| v * w).collect();
        Some(pairwise_sum(&products) / pairwise_sum(&weights))
    };
    let share = |series: &BTreeMap<String, f64>| weighted(&|p| series.get(p).copied());
    let mut s = SectorShares {
        agriculture: share(&wdi.agriculture),
        industry: share(&wdi.industry),
        manufacturing: share(&wdi.manufacturing),
        services: share(&wdi.services),
        rents: weighted(&|p| rents(wdi, p)),
    };
    let positive = py_sum([s.agriculture, s.industry, s.services].map(|x| x.unwrap_or(0.0)));
    if positive <= 0.0 {
        s = SectorShares {
            rents: s.rents,
            ..SectorShares::default()
        };
    }
    if let (Some(m), Some(i)) = (s.manufacturing, s.industry)
        && m > i
    {
        s.manufacturing = None;
    }
    (gdp, s)
}

/// `total` over `goods` by the donor's VA proportions (zero elsewhere); a single good takes it all,
/// even where the donor has none.
fn split(total: f64, goods: &[usize], donor: &CountryIo) -> [f64; N_GOODS] {
    let mut va = [0.0; N_GOODS];
    if let [g] = goods {
        va[*g] = total;
        return va;
    }
    let picked: Vec<f64> = goods.iter().map(|&g| donor.va[g]).collect();
    let sum = pairwise_sum(&picked);
    for &g in goods {
        va[g] = total * donor.va[g] / sum;
    }
    va
}

fn add(a: [f64; N_GOODS], b: [f64; N_GOODS]) -> [f64; N_GOODS] {
    std::array::from_fn(|g| a[g] + b[g])
}

const ALL: [usize; N_GOODS] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
const OIL_GAS: usize = 1;

/// What the imputation of one country's VA reads besides the donor.
struct Level {
    gdp: f64,
    shares: SectorShares,
    people: f64,
    donor_people: f64,
    /// ICIO median VA over GDP.
    ratio: f64,
    /// ICIO crude oil and gas VA over rents.
    oil_gas_ratio: f64,
    /// Largest ICIO share of crude oil and gas in its group.
    oil_gas_cap: f64,
    scale: Option<f64>,
}

/// VA shares of GDP by group of goods, and their basis. With all of agriculture, industry and
/// services the shares are WDI's; with one missing, the goods WDI does not cover take the ICIO
/// median VA over GDP less the shares there are (at least zero).
fn groups(s: SectorShares, ratio: f64) -> (Vec<(f64, Vec<usize>)>, Basis) {
    let mut known = Vec::new();
    if let Some(a) = s.agriculture {
        known.push((a, GROUPS[0].to_vec()));
    }
    match (s.industry, s.manufacturing) {
        (Some(i), Some(m)) => {
            known.push((i - m, GROUPS[1].to_vec()));
            known.push((m, GROUPS[2].to_vec()));
        }
        (Some(i), None) => known.push((i, (1..9).collect())),
        (None, Some(m)) => known.push((m, GROUPS[2].to_vec())),
        (None, None) => {}
    }
    if let Some(v) = s.services {
        known.push((v, GROUPS[3].to_vec()));
    }
    if s.agriculture.is_some() && s.industry.is_some() && s.services.is_some() {
        let basis = if s.manufacturing.is_some() {
            Basis::Wdi
        } else {
            Basis::WdiGdp
        };
        return (known, basis);
    }
    let rest: Vec<usize> = ALL
        .into_iter()
        .filter(|g| !known.iter().any(|(_, goods)| goods.contains(g)))
        .collect();
    let have = py_sum(known.iter().map(|(t, _)| *t));
    known.push(((ratio - have).max(0.0), rest));
    (known, Basis::WdiGdp)
}

/// Takes crude oil and gas out of the group that holds it at `share` of GDP, at most `cap` of the
/// group's, so the group's local goods keep a producer.
/// Where `share` exceeds `cap` of that group and WDI gives manufacturing apart, the two groups count as one
/// (WDI then counts hydrocarbons in manufacturing, as for Algeria and Iran in 2000).
fn oil_gas_apart(
    mut groups: Vec<(f64, Vec<usize>)>,
    share: f64,
    cap: f64,
) -> Vec<(f64, Vec<usize>)> {
    let Some(i) = groups.iter().position(|(_, g)| g.contains(&OIL_GAS)) else {
        return groups;
    };
    if share > cap * groups[i].0
        && let Some(m) = groups.iter().position(|(_, g)| g == GROUPS[2])
    {
        let (t, goods) = groups.remove(m);
        let i = if m < i { i - 1 } else { i };
        groups[i].0 += t;
        groups[i].1.extend(goods);
        groups[i].1.sort_unstable();
    }
    let g = groups
        .iter_mut()
        .find(|(_, goods)| goods.contains(&OIL_GAS))
        .expect("found above");
    let take = share.min(cap * g.0);
    g.0 -= take;
    g.1.retain(|&x| x != OIL_GAS);
    groups.push((take, vec![OIL_GAS]));
    groups
}

/// VA by good (million USD) and its basis, from what WDI and the population give for this country.
fn value_added(donor: &CountryIo, l: &Level) -> ([f64; N_GOODS], Basis) {
    if l.gdp > 0.0 {
        let gdp = l.gdp / 1e6;
        let (mut shares, basis) = groups(l.shares, l.ratio);
        if let Some(r) = l.shares.rents {
            shares = oil_gas_apart(shares, r * l.oil_gas_ratio, l.oil_gas_cap);
        }
        let va = shares
            .iter()
            .map(|(t, g)| split(gdp * t, g, donor))
            .fold([0.0; N_GOODS], add);
        return (va, basis);
    }
    if l.people > 0.0 {
        let per_person = pairwise_sum(&donor.va) / l.donor_people * l.scale.unwrap_or(1.0);
        return (split(l.people * per_person, &ALL, donor), Basis::Donor);
    }
    ([0.0; N_GOODS], Basis::None)
}

/// The settings `impute` takes as constants in the pack build.
pub struct Rules<'a> {
    /// The source economies of a pack country (`PARTS`).
    pub parts: &'a dyn Fn(&str) -> Vec<String>,
    /// Territories counted inside a parent: (territory, parent).
    pub inside: &'a [(&'a str, &'a str)],
    /// Region overrides by pack code.
    pub regions: &'a [(&'a str, &'a str)],
}

fn lookup<'a>(table: &[(&str, &'a str)], code: &str) -> Option<&'a str> {
    table.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
}

/// An `Observed` that ranks donors by `income` within the regions of `obs`.
fn ranked_by(obs: &Observed, income: &BTreeMap<String, f64>) -> Observed {
    Observed {
        region: obs.region.clone(),
        gdp_per_capita: income.clone(),
        ..Observed::default()
    }
}

/// `Economy` for each pack country (`(code, sovereign code)`); ICIO economies keep their own.
///
/// Another country copies the input coefficients and final demand shares of the ICIO economy in
/// its World Bank region with the closest log income, in PWT GDP per person when it has one, else
/// WDI GDP per person (ADB where WDI has none), else Maddison Project GDP per person, compared
/// with the donors' in the same source; with no income at all, the region's median. Only ICIO
/// economies with WDI GDP and population can donate.
///
/// VA by good: WDI GDP times the sector shares, each group split over its goods by the donor's
/// VA (wdi); with one of agriculture, industry or services missing, the shares there are and GDP
/// times the ICIO median VA over GDP less them over the goods they do not cover (wdi_gdp); with
/// no share, GDP times the ICIO median VA over GDP (wdi_gdp). Where WDI has oil or gas rents,
/// crude oil and gas takes GDP times the rents times the ICIO oil and gas VA over rents, out of
/// its group (all of industry where industry less manufacturing is too small) and at most the
/// largest ICIO share of crude oil and gas in industry of it (ADR 0249). With no GDP, population (WDI, else the WorldPop
/// national total) times the donor's VA per person, scaled by own over donor income (donor).
/// Output follows at the donor's output per unit of VA. A territory in `inside` takes its
/// population share of the parent's VA and output out of the parent's (parent).
pub fn impute(
    countries: &[(String, String)],
    icio: &BTreeMap<String, CountryIo>,
    wdi: &Wdi,
    obs: &Observed,
    iso: &Names,
    worldpop: &BTreeMap<String, f64>,
    rules: &Rules,
) -> Result<BTreeMap<String, Economy>, String> {
    let parts_of = rules.parts;
    let eligible: BTreeMap<String, CountryIo> = icio
        .iter()
        .filter(|(c, _)| wdi.gdp.contains_key(*c) && obs.population.contains_key(*c))
        .map(|(c, v)| (c.clone(), v.clone()))
        .collect();
    let by_wdi = ranked_by(obs, &wdi.income);
    let by_maddison = ranked_by(obs, &wdi.maddison);
    let ratio = median_va_over_gdp(icio, wdi);
    let oil_gas_ratio = oil_gas_va_over_rents(icio, wdi);
    let oil_gas_cap = oil_gas_most_of_industry(icio);
    let mut out = BTreeMap::new();
    for (code, sovereign) in countries {
        let own = iso.get(code).unwrap_or(code);
        if let Some(c) = icio.get(own) {
            out.insert(code.clone(), Economy::own(c.clone()));
            continue;
        }
        if lookup(rules.inside, code).is_some() {
            continue;
        }
        let ps = parts_of(code);
        let parents = parts_of(sovereign);
        let region = lookup(rules.regions, code)
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
            .or_else(|| {
                ps.iter()
                    .chain(&parents)
                    .find_map(|p| obs.region.get(p).cloned())
            });
        let found = |m: &BTreeMap<String, f64>| -> Vec<f64> {
            ps.iter().filter_map(|p| m.get(p).copied()).collect()
        };
        let (pwt, income, ppp) = (
            found(&obs.gdp_per_capita),
            found(&wdi.income),
            found(&wdi.maddison),
        );
        let (ranking, level, source) = if !pwt.is_empty() {
            (obs, Some(mean(&pwt)), IncomeSource::Pwt)
        } else if !income.is_empty() {
            let adb = ps
                .iter()
                .filter(|p| wdi.income.contains_key(*p))
                .all(|p| wdi.income_source.get(p).map(String::as_str) == Some("adb"));
            let source = if adb {
                IncomeSource::Adb
            } else {
                IncomeSource::Wdi
            };
            (&by_wdi, Some(mean(&income)), source)
        } else if !ppp.is_empty() {
            (&by_maddison, Some(mean(&ppp)), IncomeSource::Maddison)
        } else {
            (obs, None, IncomeSource::None)
        };
        let name = donor(
            region.as_deref(),
            level.map(f64::ln),
            &eligible,
            ranking,
            &ps,
        )?;
        let d = &eligible[&name];
        let (gdp, shares) = sector_shares(wdi, &ps);
        let mut people = py_sum(
            ps.iter()
                .map(|p| obs.population.get(p).copied().unwrap_or(0.0)),
        );
        if people == 0.0 {
            people = worldpop.get(code).copied().unwrap_or(0.0);
        }
        let level = Level {
            gdp,
            shares,
            people,
            donor_people: obs.population[&name],
            ratio,
            oil_gas_ratio,
            oil_gas_cap,
            scale: level.map(|x| x / ranking.gdp_per_capita[&name]),
        };
        let (va, basis) = value_added(d, &level);
        let output = std::array::from_fn(|g| {
            // A good the donor lacks has no inputs (its io column is zero).
            let per_va = if d.va[g] > 0.0 {
                d.output[g] / d.va[g]
            } else {
                1.0
            };
            va[g] * per_va
        });
        out.insert(
            code.clone(),
            Economy {
                table: CountryIo {
                    output,
                    va,
                    io: d.io,
                    final_demand_shares: d.final_demand_shares,
                    coal_share: d.coal_share,
                },
                donor: Some(name),
                va_basis: basis,
                region,
                income_source: source,
                shed: 0.0,
            },
        );
    }
    let original = out.clone();
    let mut share_of: BTreeMap<&str, f64> = BTreeMap::new();
    for &(code, parent) in rules.inside {
        if !countries.iter().any(|(c, _)| c == code) {
            continue;
        }
        let Some(base) = original.get(parent) else {
            continue;
        };
        let people = py_sum(
            parts_of(parent)
                .iter()
                .map(|p| obs.population.get(p).copied().unwrap_or(0.0)),
        );
        let share = worldpop.get(code).copied().unwrap_or(0.0) / people;
        *share_of.entry(parent).or_insert(0.0) += share;
        let mut t = base.scaled(share);
        t.donor = Some(
            base.donor
                .clone()
                .unwrap_or_else(|| iso.get(parent).map_or(parent, String::as_str).to_owned()),
        );
        t.va_basis = Basis::Parent;
        if let Some(r) = lookup(rules.regions, code) {
            t.region = Some(r.to_owned());
        }
        out.insert(code.to_owned(), t);
    }
    for (parent, share) in share_of {
        let mut p = original[parent].scaled(1.0 - share);
        p.shed = share;
        out.insert(parent.to_owned(), p);
    }
    Ok(out)
}

/// `impute` with the pack's `PARTS`, `INSIDE` and `REGIONS`.
pub fn impute_pack(
    countries: &[(String, String)],
    icio: &BTreeMap<String, CountryIo>,
    wdi: &Wdi,
    obs: &Observed,
    iso: &Names,
    worldpop: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, Economy>, String> {
    let parts_of = |c: &str| parts(c, iso);
    let rules = Rules {
        parts: &parts_of,
        inside: &INSIDE,
        regions: &REGIONS,
    };
    impute(countries, icio, wdi, obs, iso, worldpop, &rules)
}

fn floats(values: impl IntoIterator<Item = f64>) -> ArrayRef {
    Arc::new(values.into_iter().collect::<Float64Array>())
}

fn texts(values: impl IntoIterator<Item = String>) -> ArrayRef {
    Arc::new(StringArray::from_iter_values(values))
}

fn rows<'a>(
    codes: &[String],
    world: &'a BTreeMap<String, Economy>,
) -> Result<Vec<&'a Economy>, String> {
    codes
        .iter()
        .map(|c| {
            world
                .get(c)
                .ok_or_else(|| format!("no input-output table for {c}"))
        })
        .collect()
}

/// `econ_countries.arrow` columns of the input-output tables, in `codes` order, after `code`.
/// VA is million USD per year; GDP is USD, WDI's where the country has its own, else its VA.
pub fn country_columns(
    codes: &[String],
    world: &BTreeMap<String, Economy>,
    wdi: &Wdi,
    iso: &Names,
) -> Result<Vec<(String, ArrayRef)>, String> {
    let rows = rows(codes, world)?;
    let (mut gdp, mut gdp_imputed) = (Vec::new(), Vec::new());
    for (c, r) in codes.iter().zip(&rows) {
        let found: Vec<f64> = parts(c, iso)
            .iter()
            .filter_map(|p| wdi.gdp.get(p).copied())
            .collect();
        let own = !found.is_empty() && r.va_basis != Basis::Parent;
        gdp.push(if own {
            py_sum(found)
        } else {
            pairwise_sum(&r.table.va) * 1e6
        });
        gdp_imputed.push(!own);
    }
    let mut out: Vec<(String, ArrayRef)> = vec![
        (
            "donor".into(),
            texts(rows.iter().map(|r| r.donor.clone().unwrap_or_default())),
        ),
        (
            "va_basis".into(),
            texts(rows.iter().map(|r| r.va_basis.as_str().to_owned())),
        ),
        (
            "income_source".into(),
            texts(rows.iter().map(|r| r.income_source.as_str().to_owned())),
        ),
        (
            "region".into(),
            texts(rows.iter().map(|r| r.region.clone().unwrap_or_default())),
        ),
        (
            "va_imputed".into(),
            Arc::new(
                rows.iter()
                    .map(|r| Some(r.va_basis != Basis::Icio))
                    .collect::<BooleanArray>(),
            ),
        ),
        ("gdp".into(), floats(gdp)),
        (
            "gdp_imputed".into(),
            Arc::new(gdp_imputed.into_iter().map(Some).collect::<BooleanArray>()),
        ),
        (
            "coal_share".into(),
            floats(rows.iter().map(|r| r.table.coal_share)),
        ),
    ];
    for g in 0..N_GOODS {
        out.push((
            format!("fd_{g}"),
            floats(rows.iter().map(|r| r.table.final_demand_shares[g])),
        ));
    }
    for g in 0..N_GOODS {
        out.push((
            format!("va_{g}"),
            floats(rows.iter().map(|r| r.table.va[g])),
        ));
    }
    Ok(out)
}

/// `econ_io.arrow`: a_kg for every country, input k and good g, country-major, g innermost.
pub fn io_columns(
    codes: &[String],
    world: &BTreeMap<String, Economy>,
) -> Result<Vec<(String, ArrayRef)>, String> {
    let rows = rows(codes, world)?;
    let n = u32::try_from(rows.len()).map_err(|_| "too many countries for uint32")?;
    let ng = N_GOODS as u8;
    let cells = N_GOODS * N_GOODS;
    Ok(vec![
        (
            "country".into(),
            Arc::new(UInt32Array::from_iter_values(
                (0..n).flat_map(|c| std::iter::repeat_n(c, cells)),
            )),
        ),
        (
            "input".into(),
            Arc::new(UInt8Array::from_iter_values((0..n).flat_map(|_| {
                (0..ng).flat_map(|k| std::iter::repeat_n(k, N_GOODS))
            }))),
        ),
        (
            "good".into(),
            Arc::new(UInt8Array::from_iter_values(
                (0..n).flat_map(|_| (0..ng).flat_map(|_| 0..ng)),
            )),
        ),
        (
            "a".into(),
            floats(
                rows.iter()
                    .flat_map(|r| r.table.io.iter().flat_map(|row| row.iter().copied())),
            ),
        ),
    ])
}
