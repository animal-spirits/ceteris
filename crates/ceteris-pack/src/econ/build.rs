//! The 2000 economic tables of pack format 3 (specs/econ/SPEC.md, Pack additions), built from the
//! P1 tables of a pack and the fetch cache.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array,
};
use arrow_ipc::reader::FileReader;

use super::downscale::{self, Downscaled, GEM, Geography, Provinces, RESOURCES};
use super::gateways::{self, Edges, Nodes};
use super::theta::{self, ClassTable};
use super::{factors, goods, icio, people, price_level, tariffs, transfers};
use crate::json::Json;
use crate::numeric::pairwise_sum;
use crate::raster::geotiff;
use crate::raster::population::{BLOCK_ROWS, CellOwner, province_population};
use crate::read::YEAR;
use crate::read::baci::{read_hs_cpa, trade_by_country, trade_by_good};
use crate::read::icio::{aggregate, home_use, load_wdi};
use crate::read::natural_earth::{iso3_by_iso2, read_codes};
use crate::read::people::{load_observed, parts};
use crate::read::tariffs::{DTA, GRAVITY, read_cepii, read_dta};
use crate::read::wdi::{read_rates, read_wdi};
use crate::read::{sea, wid};
use crate::table;

/// The written tables, in writing order.
pub const TABLES: [&str; 8] = [
    "econ_goods.arrow",
    "econ_countries.arrow",
    "econ_io.arrow",
    "econ_provinces.arrow",
    "econ_pops.arrow",
    "econ_tariffs.arrow",
    "gateways.arrow",
    "econ_home_shares.arrow",
];
pub const WORLDPOP: &str = "ppp_2000_1km_Aggregated.tif";
pub use crate::read::natural_earth::ADMIN0;
pub const BACI: &str = "BACI_HS92_V202601.zip";
pub const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
/// P02 inventory target coverage s_g in months, the prior's start (trade SPEC, Parameters;
/// TradeParams::coverage).
pub const COVERAGE: [f64; goods::NAMES.len()] = [3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0];

/// The written tables: file name and SHA-256 of its bytes, in writing order.
#[derive(Debug)]
pub struct BuildReport {
    pub tables: Vec<(String, [u8; 32])>,
    pub stats: BuildStats,
    /// Human-readable summaries of the built tables.
    pub summary: Vec<String>,
}

/// Counts and shares of the built tables, the `econ` stats of the pack manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildStats {
    pub gateways: usize,
    /// Gateways by kind, in `gateways::KINDS` order.
    pub gateway_kinds: [usize; gateways::KINDS.len()],
    pub tariff_importers_own: usize,
    pub tariff_agreement_pairs: usize,
    /// Rounded to 4 decimals, as are the shares below.
    pub dose_match_rate: f64,
    pub provinces_with_va: usize,
    pub va_imputed_share: f64,
}

impl BuildStats {
    fn new(
        gw: &[gateways::Gateway],
        rates: &tariffs::Tariffs,
        matched: &downscale::DoseMatch,
        placed: &Downscaled,
    ) -> Self {
        let mut gateway_kinds = [0; gateways::KINDS.len()];
        for g in gw {
            gateway_kinds[usize::from(g.kind)] += 1;
        }
        let pairs: usize = rates
            .agreement
            .iter()
            .map(|row| row.iter().filter(|&&a| a).count())
            .sum();
        let va: Vec<f64> = placed.va.iter().flatten().copied().collect();
        let imputed: Vec<f64> = placed
            .va
            .iter()
            .flatten()
            .zip(placed.imputed.iter().flatten())
            .filter_map(|(&v, &i)| i.then_some(v))
            .collect();
        BuildStats {
            gateways: gw.len(),
            gateway_kinds,
            tariff_importers_own: rates.imputed.iter().filter(|&&i| !i).count(),
            tariff_agreement_pairs: pairs / 2,
            dose_match_rate: round4(matched.rate()),
            provinces_with_va: placed
                .va
                .iter()
                .filter(|v| pairwise_sum(&v[..]) > 0.0)
                .count(),
            va_imputed_share: round4(pairwise_sum(&imputed) / pairwise_sum(&va)),
        }
    }

    /// The stats as the manifest writes them.
    pub fn json(&self) -> Json {
        let count = |n: usize| Json::Int(i64::try_from(n).expect("count fits i64"));
        Json::object([
            ("gateways", count(self.gateways)),
            (
                "gateway_kinds",
                Json::object(
                    gateways::KINDS
                        .iter()
                        .zip(self.gateway_kinds)
                        .map(|(k, n)| (k, count(n))),
                ),
            ),
            ("tariff_importers_own", count(self.tariff_importers_own)),
            ("tariff_agreement_pairs", count(self.tariff_agreement_pairs)),
            ("dose_match_rate", Json::Float(self.dose_match_rate)),
            ("provinces_with_va", count(self.provinces_with_va)),
            ("va_imputed_share", Json::Float(self.va_imputed_share)),
        ])
    }
}

/// Python's `round(x, 4)`: the decimal nearest the exact binary value.
fn round4(x: f64) -> f64 {
    format!("{x:.4}").parse().expect("formatted float parses")
}

/// String columns of a pack table, every batch in order.
fn strings<const N: usize>(path: &Path, columns: [&str; N]) -> Result<[Vec<String>; N], String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let reader =
        FileReader::try_new(File::open(path).map_err(|e| fail(&e))?, None).map_err(|e| fail(&e))?;
    let mut out: [Vec<String>; N] = std::array::from_fn(|_| Vec::new());
    for batch in reader {
        let batch = batch.map_err(|e| fail(&e))?;
        for (column, values) in columns.iter().zip(&mut out) {
            let a = batch
                .column_by_name(column)
                .ok_or_else(|| fail(&format!("no column {column}")))?;
            let a = a
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| fail(&format!("{column} is not a string column")))?;
            if a.null_count() > 0 {
                return Err(fail(&format!("null in {column}")));
            }
            values.extend(a.iter().flatten().map(str::to_owned));
        }
    }
    Ok(out)
}

/// One boolean column of a pack table, every batch in order.
fn flags(path: &Path, column: &str) -> Result<Vec<bool>, String> {
    Ok(table::arrays::<BooleanArray>(path, column)?
        .iter()
        .flat_map(|a| a.values().iter())
        .collect())
}

/// The provinces table columns the downscaling proxies read.
pub fn read_geography(pack: &Path) -> Result<Geography, String> {
    let path = pack.join("provinces.arrow");
    let [id, country] = strings(&path, ["id", "country"])?;
    let mut res: [Vec<u32>; RESOURCES.len()] = Default::default();
    for (r, name) in res.iter_mut().zip(RESOURCES) {
        *r = table::values::<UInt32Type>(&path, &format!("res_{name}"))?;
    }
    Ok(Geography {
        id,
        country,
        coastal: flags(&path, "coastal")?,
        lon: table::values::<Float64Type>(&path, "lon")?,
        lat: table::values::<Float64Type>(&path, "lat")?,
        res,
    })
}

/// `econ_provinces.arrow` columns of the downscaled VA (million USD per year), its imputed flag
/// and proxy level by good.
fn va_columns(placed: &Downscaled) -> Vec<(String, ArrayRef)> {
    let n = goods::NAMES.len();
    let mut cols: Vec<(String, ArrayRef)> = Vec::new();
    for g in 0..n {
        cols.push((
            format!("va_{g}"),
            Arc::new(placed.va.iter().map(|v| v[g]).collect::<Float64Array>()),
        ));
    }
    for g in 0..n {
        cols.push((
            format!("va_imputed_{g}"),
            Arc::new(
                placed
                    .imputed
                    .iter()
                    .map(|v| Some(v[g]))
                    .collect::<BooleanArray>(),
            ),
        ));
    }
    for g in 0..n {
        cols.push((
            format!("va_level_{g}"),
            Arc::new(
                placed
                    .level
                    .iter()
                    .map(|v| v[g] as u8)
                    .collect::<UInt8Array>(),
            ),
        ));
    }
    cols
}

fn read_graph(pack: &Path) -> Result<(Nodes, Edges), String> {
    let (nodes, edges) = (pack.join("nodes.arrow"), pack.join("edges.arrow"));
    Ok((
        Nodes {
            kind: table::values::<UInt8Type>(&nodes, "kind")?,
            province: table::values::<UInt32Type>(&nodes, "province")?,
            lon: table::values::<Float64Type>(&nodes, "lon")?,
            lat: table::values::<Float64Type>(&nodes, "lat")?,
        },
        Edges {
            a: table::values::<UInt32Type>(&edges, "a")?,
            b: table::values::<UInt32Type>(&edges, "b")?,
            mode: table::values::<UInt8Type>(&edges, "mode")?,
        },
    ))
}

fn write(
    out: &Path,
    name: &str,
    columns: Vec<(String, ArrayRef)>,
) -> Result<(String, [u8; 32]), String> {
    write_batch(out, name, &table::batch(columns)?)
}

fn write_batch(out: &Path, name: &str, batch: &RecordBatch) -> Result<(String, [u8; 32]), String> {
    let path = out.join(name);
    let digest = table::write(&path, batch, &BTreeMap::new())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((name.to_owned(), digest))
}

/// `econ_goods.arrow`: name, whether the good is traded, tonnes per USD, inventory coverage,
/// and the good's theta class with that class's harmonic and arithmetic mean theta (ADRs 0023
/// and 0101; 0 for local goods).
fn goods_columns(
    weight: [f64; goods::NAMES.len()],
    (of, arithmetic, harmonic): &ClassTable,
) -> Vec<(String, ArrayRef)> {
    let n = goods::NAMES.len();
    let per_good = |class_value: &[f64]| -> ArrayRef {
        Arc::new(
            (0..n)
                .map(|g| of.get(g).map_or(0.0, |&c| class_value[usize::from(c)]))
                .collect::<Float64Array>(),
        )
    };
    vec![
        (
            "name".into(),
            Arc::new(StringArray::from_iter_values(goods::NAMES)),
        ),
        (
            "traded".into(),
            Arc::new((0..n).map(|g| g < goods::TRADED).collect::<BooleanArray>()),
        ),
        (
            "weight".into(),
            Arc::new(weight.into_iter().collect::<Float64Array>()),
        ),
        (
            "coverage".into(),
            Arc::new(COVERAGE.into_iter().collect::<Float64Array>()),
        ),
        (
            "theta_class".into(),
            Arc::new(UInt8Array::from_iter_values(
                (0..n).map(|g| of.get(g).copied().unwrap_or(0)),
            )),
        ),
        ("theta_harmonic".into(), per_good(harmonic)),
        ("theta_arithmetic".into(), per_good(arithmetic)),
    ]
}

/// Writes the econ tables of `pack` (its P1 tables) into `out`, reading the sources from `cache`.
pub fn build(pack: &Path, out: &Path, cache: &Path) -> Result<BuildReport, String> {
    let [codes, sovereign] = strings(&pack.join("countries.arrow"), ["code", "sovereign"])?;
    let geo = read_geography(pack)?;
    let country = geo.country.clone();
    let admin0 = cache.join(ADMIN0);

    let (iso, _) = read_codes(&admin0)?;
    let obs = load_observed(cache, &admin0)?;
    let countries: Vec<(String, String)> = codes.iter().cloned().zip(sovereign).collect();
    let labour = people::country_labour(&countries, &obs, &iso)?;
    let mut owner = CellOwner::read(&pack.join("cells.arrow"))?;
    let raw = province_population(
        &mut geotiff::open(&cache.join(WORLDPOP))?,
        &mut owner,
        BLOCK_ROWS,
    )?;
    if raw.len() != country.len() {
        return Err(format!(
            "cells.arrow names {} provinces, provinces.arrow has {}",
            raw.len(),
            country.len()
        ));
    }
    let mut worldpop: BTreeMap<String, f64> = BTreeMap::new();
    for (c, p) in country.iter().zip(&raw) {
        *worldpop.entry(c.clone()).or_insert(0.0) += p;
    }
    let totals = people::national_population(&codes, &obs, &iso);
    let population = people::scale_to_national(&raw, &country, &totals);

    let wdi = load_wdi(cache)?;
    let hs = read_hs_cpa(&cache.join(HS_KEY))?;
    let by_good = trade_by_good(&cache.join(BACI), &goods::hs_goods(&hs), &iso, &codes)?;
    let real = aggregate(&cache.join(icio::ICIO), YEAR)?;
    let world = icio::impute_pack(&countries, &real, &wdi, &obs, &iso, &worldpop, &by_good)?;

    let pwt_levels = price_level::relative(&obs.price_level_pwt, "PWT")?;
    let [gdp, ppp, pop] = price_level::WDI_PRICE_LEVEL.map(|n| read_wdi(&cache.join(n)));
    let gdp_by_year = gdp?;
    let wdi_level = price_level::wdi_implied(&gdp_by_year, &ppp?, &pop?);
    let screen = price_level::Screen::fit(&pwt_levels, &wdi.income)?;
    let levels = price_level::price_levels(
        &codes,
        &|c| parts(c, &iso),
        &|c| world.get(c).and_then(|e| e.donor.clone()),
        &pwt_levels,
        &price_level::relative(&wdi_level, "WDI")?,
        &wdi.income,
        &screen,
    )?;

    let working_age = population
        .iter()
        .zip(&country)
        .map(|(p, c)| {
            labour
                .get(c)
                .map(|l| p * l.working_age_share)
                .ok_or_else(|| format!("no labour inputs for {c}"))
        })
        .collect::<Result<Vec<f64>, String>>()?;
    let gem = GEM.map(|n| cache.join(n));
    let gem = gem
        .iter()
        .all(|p| p.exists())
        .then(|| (gem[0].clone(), gem[1].clone()));
    let (proxies, matched) = downscale::load_proxies(
        cache,
        &geo,
        &mut owner,
        &population,
        &obs.population,
        &iso,
        gem,
    )?;
    let placed = downscale::allocate(
        &world,
        &Provinces {
            country: country.clone(),
            population: population.clone(),
            working_age,
        },
        &proxies,
    )?;

    let weight = goods::weights_from_baci(&cache.join(BACI), &cache.join(HS_KEY))?;
    let classes = theta::class_table(&cache.join(BACI), &cache.join(HS_KEY))?;

    let trade = trade_by_country(&cache.join(BACI), &iso, &codes)?;
    let agreements = tariffs::pack_pairs(
        &read_cepii(&cache.join(GRAVITY))?
            .union(&read_dta(&cache.join(DTA))?)
            .cloned()
            .collect(),
        &codes,
        &iso,
    );
    let sovereigns: BTreeMap<String, String> = countries.iter().cloned().collect();
    let rates = tariffs::table(
        &read_rates(cache)?,
        &agreements,
        &codes,
        &iso,
        &obs,
        &sovereigns,
    )?;
    let (nodes, edges) = read_graph(pack)?;
    let gw = gateways::select(&nodes, &edges, &country, &trade, gateways::TOTAL)?;

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut countries_columns: Vec<(String, ArrayRef)> =
        vec![("code".into(), Arc::new(StringArray::from(codes.clone())))];
    countries_columns.extend(icio::country_columns(&codes, &world, &wdi, &iso)?);
    countries_columns.extend(people::country_columns(&codes, &labour)?);
    countries_columns.extend(price_level::columns(&levels));
    let (pack_gdp, _) = icio::country_gdp(&codes, &world, &wdi, &iso)?;
    let imbalances = transfers::transfers(
        &codes,
        &iso,
        &pack_gdp,
        &read_wdi(&cache.join(transfers::CURRENT_ACCOUNT))?,
        &gdp_by_year,
    );
    countries_columns.extend(transfers::columns(&imbalances));
    // sector labour shares and wealth shares (ADRs 0307, 0308) where SEA and WID are cached
    let (sea_path, wid_path) = (cache.join(sea::SEA), cache.join(wid::WID));
    if sea_path.exists() && wid_path.exists() {
        let iso2 = iso3_by_iso2(&admin0)?;
        let wid_iso3: BTreeMap<String, [f64; 3]> = wid::read_wid(&wid_path)?
            .into_iter()
            .filter_map(|(a2, w)| Some((iso2.get(&a2)?.clone(), w)))
            .collect();
        let f = factors::factors(
            &codes,
            &|c| labour.get(c).map(|l| l.labour_share),
            &|c| world.get(c).map(|e| e.table.va),
            &|c| parts(c, &iso),
            &obs.gdp_per_capita,
            &obs.population,
            &obs.region,
            &sea::read_sea(&sea_path)?,
            &wid_iso3,
        )?;
        countries_columns.extend(factors::columns(&f));
    }
    let [
        goods_t,
        countries_t,
        io_t,
        provinces_t,
        pops_t,
        tariffs_t,
        gateways_t,
        home_shares_t,
    ] = TABLES;
    let tables = vec![
        write(out, goods_t, goods_columns(weight, &classes))?,
        write(out, countries_t, countries_columns)?,
        write(out, io_t, icio::io_columns(&codes, &world)?)?,
        write(out, provinces_t, {
            let mut cols = people::province_columns(&country, &population, &labour)?;
            cols.extend(va_columns(&placed));
            cols
        })?,
        write_batch(out, pops_t, &people::pops(&country, &population, &labour)?)?,
        write(out, tariffs_t, tariffs::columns(&rates)?)?,
        write(out, gateways_t, gateways::columns(&gw))?,
        write(
            out,
            home_shares_t,
            icio::home_share_columns(&codes, &home_use(&cache.join(icio::ICIO), YEAR)?, &iso)?,
        )?,
    ];
    Ok(BuildReport {
        tables,
        stats: BuildStats::new(&gw, &rates, &matched, &placed),
        summary: vec![
            icio::balance_report(&world, &by_good, real.get("ROW")),
            downscale::report(&placed, &matched),
            tariffs::report(&rates, &codes, &trade),
            gateways::report(&gw, &codes),
            price_level::report(&levels, &screen),
            transfers::report(&imbalances, &codes, &pack_gdp),
        ],
    })
}
