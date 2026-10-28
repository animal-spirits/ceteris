//! `ceteris pack dump-reader NAME [ARGS]`: one reader's output on the fetch cache as canonical
//! JSON, for comparison with the Python reader (`scripts/parity/readers.py`).

use std::collections::BTreeMap;
use std::fs::File;
use std::path::PathBuf;

use arrow_array::StringArray;
use arrow_ipc::reader::FileReader;

use super::baci::{baci_by_product, baci_values, read_hs_cpa, trade_by_country};
use super::downscale::{Asset, DOSE, SEDS, read_dose, read_gem_coal, read_gem_oil_gas, read_seds};
use super::icio::{
    ADB_INCOME, CountryIo, MADDISON, Wdi, aggregate, exports, load_wdi, read_adb_income,
    read_maddison,
};
use super::natural_earth::{province_names, read_codes, us_states};
use super::people::{
    Observed, PARTS, U_STAR_YEARS, load_observed, read_barro_lee, read_ilostat, read_ilostat_mean,
    read_pwt, read_wic,
};
use super::tariffs::{DTA, GRAVITY, read_cepii, read_dta};
use super::wdi::{read_rates, read_wdi, read_wdi_regions};
use super::xlsx::{self, Cell};
use super::{ByYear, WINDOW, YEAR};
use crate::econ::build::{WORLDPOP as WORLDPOP_TIF, read_geography};
use crate::econ::downscale::{GEM, Proxies, load_proxies};
use crate::econ::goods::hs_goods;
use crate::econ::people::{national_population, scale_to_national};
use crate::econ::theta::{by_good, caliendo_parro_with_value, class_bands, classes, sector_values};
use crate::json::Json;
use crate::raster::geotiff;
use crate::raster::population::{
    BLOCK_ROWS, CellOwner, cell_of, province_population, spam_physical_area,
};
use crate::{fetch, sources};

pub const NAMES: &[&str] = &[
    "xlsx FILE SHEET",
    "codes",
    "ilostat-participation",
    "ilostat-unemployment",
    "ilostat-natural",
    "wic",
    "barro-lee",
    "wdi INDICATOR",
    "wdi-regions",
    "pwt",
    "observed",
    "province-names",
    "us-states",
    "adb-income",
    "maddison",
    "load-wdi",
    "icio-aggregate",
    "icio-exports",
    "hs-cpa",
    "hs-goods",
    "parts",
    "baci-by-product",
    "baci-values",
    "theta",
    "theta-bands",
    "theta-fixture",
    "trade-by-country",
    "rates",
    "cepii",
    "dta",
    "dose",
    "seds",
    "gem-coal",
    "gem-oil-gas",
    "population worldpop|mapspam|kummu",
    "point-owners coal|oil-gas",
    "proxies",
];

const ICIO: &str = "1995-2000_SML.zip";
const GEM_COAL: &str = "Global Coal Mine Tracker, August 2026.xlsx";
const GEM_OIL_GAS: &str = "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx";
const BACI: &str = "BACI_HS92_V202601.zip";
const WORLDPOP: &str = "ppp_2000_1km_Aggregated.tif";
const SPAM: &str = "spam2000v3.0.7_global_physical-area.geotiff.zip";
const KUMMU: &str = "kummu_gdp_ppp_2000_5arcmin.tif";
const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";

fn cache(name: &str) -> PathBuf {
    fetch::path_of(name)
}

fn admin0() -> PathBuf {
    cache("ne_10m_admin_0_countries.zip")
}

fn admin1() -> PathBuf {
    cache("ne_10m_admin_1_states_provinces.zip")
}

fn series(m: BTreeMap<String, ByYear>) -> Json {
    Json::object(m.into_iter().map(|(k, v)| (k, Json::from(v))))
}

fn cell(c: Cell) -> Json {
    match c {
        Cell::Number(x) => Json::Float(x),
        Cell::Text(s) => Json::Str(s),
    }
}

fn observed(o: Observed) -> Json {
    Json::object([
        ("participation", Json::from(o.participation)),
        ("unemployment", o.unemployment.into()),
        ("natural_unemployment", o.natural_unemployment.into()),
        ("working_age", o.working_age.into()),
        ("edu_wic", o.edu_wic.into()),
        ("edu_barro_lee", o.edu_barro_lee.into()),
        ("labour_share", o.labour_share.into()),
        ("gini_wdi", o.gini_wdi.into()),
        ("gini_swiid", o.gini_swiid.into()),
        ("gdp_per_capita", o.gdp_per_capita.into()),
        ("population", o.population.into()),
        ("region", o.region.into()),
    ])
}

fn country_io(c: CountryIo) -> Json {
    Json::object([
        ("output", Json::from(c.output)),
        ("va", c.va.into()),
        ("io", Json::list(c.io)),
        ("final_demand_shares", c.final_demand_shares.into()),
        ("coal_share", c.coal_share.into()),
    ])
}

fn wdi(w: Wdi) -> Json {
    Json::object([
        ("gdp", Json::from(w.gdp)),
        ("income", w.income.into()),
        ("income_source", w.income_source.into()),
        ("maddison", w.maddison.into()),
        ("agriculture", w.agriculture.into()),
        ("industry", w.industry.into()),
        ("manufacturing", w.manufacturing.into()),
        ("services", w.services.into()),
    ])
}

fn assets(a: Vec<Asset>) -> Json {
    Json::List(
        a.into_iter()
            .map(|a| {
                Json::List(vec![
                    a.lat.into(),
                    a.lon.into(),
                    a.weight.into(),
                    a.dated.into(),
                    a.offshore.into(),
                    a.country.into(),
                ])
            })
            .collect(),
    )
}

/// Codes of the pack's countries table, in its order.
fn pack_codes() -> Result<Vec<String>, String> {
    let path = sources::workspace_root().join("data/packs/2000/countries.arrow");
    let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let reader = FileReader::try_new(file, None).map_err(|e| e.to_string())?;
    let mut codes = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| e.to_string())?;
        let column = batch
            .column_by_name("code")
            .ok_or("countries.arrow has no code column")?;
        let column = column
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("code is not a string column")?;
        codes.extend(column.iter().map(|c| c.unwrap_or_default().to_owned()));
    }
    Ok(codes)
}

/// The output of reader `name` with its `args`.
pub fn dump(name: &str, args: &[String]) -> Result<Json, String> {
    let cache_dir = cache("");
    let arg = |i: usize| {
        args.get(i)
            .map(String::as_str)
            .ok_or(format!("{name} needs more arguments"))
    };
    Ok(match name {
        "xlsx" => Json::List(
            xlsx::sheet(&cache(arg(0)?), arg(1)?)?
                .map(|r| Json::object(r.into_iter().map(|(k, c)| (k, cell(c)))))
                .collect(),
        ),
        "codes" => {
            let (iso, m49) = read_codes(&admin0())?;
            Json::list([Json::from(iso), m49.into()])
        }
        "ilostat-participation" => read_ilostat(
            &cache("ilostat_EAP_2WAP_SEX_AGE_RT_A_2000.csv"),
            "AGE_YTHADULT_Y15-64",
        )?
        .into(),
        "ilostat-unemployment" => read_ilostat(
            &cache("ilostat_UNE_2EAP_SEX_AGE_RT_A_2000.csv"),
            "AGE_YTHADULT_YGE15",
        )?
        .into(),
        "ilostat-natural" => read_ilostat_mean(
            &cache("ilostat_UNE_2EAP_SEX_AGE_RT_A_1995-2005.csv"),
            "AGE_YTHADULT_YGE15",
            U_STAR_YEARS.0,
            U_STAR_YEARS.1,
        )?
        .into(),
        "wic" => {
            let (_, m49) = read_codes(&admin0())?;
            let m49_to_iso = m49.into_iter().map(|(i, m)| (m, i)).collect();
            read_wic(&cache("BPJresult_AGE_V01.csv"), &m49_to_iso)?.into()
        }
        "barro-lee" => read_barro_lee(&cache("BL_v3_MF1564.csv"))?.into(),
        "wdi" => series(read_wdi(&cache(&format!("wdi_{}.zip", arg(0)?)))?),
        "wdi-regions" => read_wdi_regions(&cache("wdi_SI.POV.GINI.zip"))?.into(),
        "pwt" => {
            let p = read_pwt(&cache("pwt110.xlsx"))?;
            Json::list([
                series(p.labour_share),
                series(p.income),
                series(p.population),
            ])
        }
        "observed" => observed(load_observed(&cache_dir, &admin0())?),
        "province-names" => province_names(&admin1())?.into(),
        "us-states" => us_states(&admin1())?.into(),
        "adb-income" => series(read_adb_income(&cache(ADB_INCOME))?),
        "maddison" => series(read_maddison(&cache(MADDISON))?),
        "load-wdi" => wdi(load_wdi(&cache_dir)?),
        "icio-aggregate" => Json::object(
            aggregate(&cache(ICIO), YEAR)?
                .into_iter()
                .map(|(k, c)| (k, country_io(c))),
        ),
        "icio-exports" => exports(&cache(ICIO), YEAR)?.into(),
        "hs-cpa" => read_hs_cpa(&cache(HS_KEY))?.into(),
        "hs-goods" => Json::object(hs_goods(&read_hs_cpa(&cache(HS_KEY))?).into_iter().map(
            |(hs, g)| {
                (
                    hs,
                    Json::Int(i64::try_from(g).expect("good index fits i64")),
                )
            },
        )),
        "parts" => {
            Json::object(PARTS.map(|(code, parts)| (code, Json::list(parts.iter().copied()))))
        }
        "baci-by-product" => baci_by_product(&cache(BACI))?.into(),
        "baci-values" => baci_values(&cache(BACI))?.into(),
        "theta" => theta_dump(false)?,
        "theta-bands" => theta_dump(true)?,
        "theta-fixture" => theta_fixture()?,
        "trade-by-country" => {
            let (iso, _) = read_codes(&admin0())?;
            trade_by_country(&cache(BACI), &iso, &pack_codes()?)?.into()
        }
        "rates" => Json::list(read_rates(&cache_dir)?),
        "cepii" => read_cepii(&cache(GRAVITY))?.into(),
        "dta" => read_dta(&cache(DTA))?.into(),
        "dose" => {
            let population = read_wdi(&cache("wdi_SP.POP.TOTL.zip"))?
                .into_iter()
                .filter_map(|(k, v)| Some((k, *v.get(&YEAR)?)))
                .collect();
            Json::object(
                read_dose(&cache(DOSE), &population, YEAR, WINDOW)?
                    .into_iter()
                    .map(|(k, d)| {
                        (
                            k,
                            Json::object([
                                (
                                    "values",
                                    Json::from(d.values.into_iter().collect::<BTreeMap<_, _>>()),
                                ),
                                ("coverage", d.coverage.into()),
                            ]),
                        )
                    }),
            )
        }
        "seds" => read_seds(&cache(SEDS), YEAR)?.into(),
        "gem-coal" => assets(read_gem_coal(&cache(GEM_COAL))?),
        "gem-oil-gas" => assets(read_gem_oil_gas(&cache(GEM_OIL_GAS))?),
        "population" => {
            let mut owner =
                CellOwner::read(&sources::workspace_root().join("data/packs/2000/cells.arrow"))?;
            let total = match arg(0)? {
                "worldpop" => province_population(
                    &mut geotiff::open(&cache(WORLDPOP))?,
                    &mut owner,
                    BLOCK_ROWS,
                )?,
                "kummu" => {
                    province_population(&mut geotiff::open(&cache(KUMMU))?, &mut owner, BLOCK_ROWS)?
                }
                "mapspam" => province_population(
                    &mut spam_physical_area(&cache(SPAM))?,
                    &mut owner,
                    BLOCK_ROWS,
                )?,
                other => {
                    return Err(format!(
                        "unknown raster '{other}': worldpop, mapspam or kummu"
                    ));
                }
            };
            Json::list(total)
        }
        "point-owners" => {
            let assets = match arg(0)? {
                "coal" => read_gem_coal(&cache(GEM_COAL))?,
                "oil-gas" => read_gem_oil_gas(&cache(GEM_OIL_GAS))?,
                other => return Err(format!("unknown assets '{other}': coal or oil-gas")),
            };
            point_owners(&assets)?
        }
        "proxies" => proxies()?,
        _ => {
            return Err(format!(
                "unknown reader '{name}'; readers: {}",
                NAMES.join(", ")
            ));
        }
    })
}

fn pack_dir() -> PathBuf {
    sources::workspace_root().join("data/packs/2000")
}

/// Each asset's H3 cell at the pack's base resolution and the province owning its point.
fn point_owners(assets: &[Asset]) -> Result<Json, String> {
    let mut owner = CellOwner::read(&pack_dir().join("cells.arrow"))?;
    let mut out = Vec::with_capacity(assets.len());
    for a in assets {
        let cell = u64::from(cell_of(a.lat, a.lon, owner.base)?);
        let cell = i64::try_from(cell).map_err(|e| format!("cell {cell}: {e}"))?;
        out.push(Json::list([cell, owner.of_point(a.lat, a.lon)?]));
    }
    Ok(Json::List(out))
}

/// Every downscaling proxy and the DOSE match counts, from the inputs the econ build gives them.
fn proxies() -> Result<Json, String> {
    let pack = pack_dir();
    let cache_dir = cache("");
    let (iso, _) = read_codes(&admin0())?;
    let obs = load_observed(&cache_dir, &admin0())?;
    let codes = pack_codes()?;
    let geo = read_geography(&pack)?;
    let mut owner = CellOwner::read(&pack.join("cells.arrow"))?;
    let raw = province_population(
        &mut geotiff::open(&cache(WORLDPOP_TIF))?,
        &mut owner,
        BLOCK_ROWS,
    )?;
    let population =
        scale_to_national(&raw, &geo.country, &national_population(&codes, &obs, &iso));
    let gem = GEM.map(cache);
    let gem = gem
        .iter()
        .all(|p| p.exists())
        .then(|| (gem[0].clone(), gem[1].clone()));
    let (p, m) = load_proxies(
        &cache_dir,
        &geo,
        &mut owner,
        &population,
        &obs.population,
        &iso,
        gem,
    )?;
    let Proxies {
        crops,
        oil_gas,
        oil_gas_undated,
        eia,
        coal,
        sites,
        dose,
        dose_rest,
        gdp,
    } = p;
    let count = |n: usize| Json::Int(n as i64);
    Ok(Json::object([
        (
            "proxies",
            Json::object([
                ("crops", Json::from(crops)),
                ("oil_gas", oil_gas.into()),
                ("oil_gas_undated", oil_gas_undated.into()),
                ("eia", eia.into()),
                ("coal", coal.into()),
                ("sites", sites.into()),
                ("dose", dose.into()),
                ("dose_rest", dose_rest.into()),
                ("gdp", gdp.into()),
            ]),
        ),
        (
            "match",
            Json::object([
                ("matched", count(m.matched)),
                ("regions", count(m.regions)),
                ("gva_share", m.gva_share.into()),
                (
                    "by_country",
                    Json::object(
                        m.by_country
                            .into_iter()
                            .map(|(k, (a, b))| (k, Json::list([count(a), count(b)]))),
                    ),
                ),
            ]),
        ),
    ]))
}

/// Theta per good and trade value before and after the local goods are zeroed, the value by
/// Caliendo-Parro sector, the classes of the traded goods and their thetas; with `bands`, only
/// the class bands, whose matmul order numpy leaves to BLAS.
fn theta_dump(bands: bool) -> Result<Json, String> {
    let hs_cpa = read_hs_cpa(&cache(HS_KEY))?;
    let lines = hs_goods(&hs_cpa);
    let value = baci_values(&cache(BACI))?;
    let (theta_raw, value_raw) = by_good(&hs_cpa, &lines, &value);
    let sectors = sector_values(&hs_cpa, &lines, &value);
    let (theta, total) = caliendo_parro_with_value(&cache(BACI), &cache(HS_KEY))?;
    let (of, class_theta) = classes(&theta[..7], &total[..7])?;
    if bands {
        let (centre, se) = class_bands(&sectors[..7], &of);
        return Ok(Json::object([
            ("centre", Json::from(centre)),
            ("se", se.into()),
        ]));
    }
    Ok(Json::object([
        ("theta_raw", Json::from(theta_raw)),
        ("value_raw", value_raw.into()),
        ("sector_values", Json::list(sectors)),
        ("theta", theta.into()),
        ("value", total.into()),
        ("class_of", Json::list(of.iter().map(|c| i64::from(*c)))),
        ("class_theta", class_theta.into()),
    ]))
}

/// Classes of the p2 fixture's traded thetas weighted by its weight column.
fn theta_fixture() -> Result<Json, String> {
    let path = sources::workspace_root().join("tests/golden/p2-fixture/goods.arrow");
    let file = File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let reader = FileReader::try_new(file, None).map_err(|e| e.to_string())?;
    let (mut theta, mut weight) = (Vec::new(), Vec::new());
    for batch in reader {
        let batch = batch.map_err(|e| e.to_string())?;
        for (name, out) in [("theta", &mut theta), ("weight", &mut weight)] {
            let column = batch
                .column_by_name(name)
                .and_then(|a| a.as_any().downcast_ref::<arrow_array::Float64Array>())
                .ok_or_else(|| format!("{}: no float column {name}", path.display()))?;
            out.extend(column.iter().flatten());
        }
    }
    let (of, class_theta) = classes(&theta[..7], &weight[..7])?;
    Ok(Json::object([
        ("class_of", Json::list(of.iter().map(|c| i64::from(*c)))),
        ("class_theta", class_theta.into()),
    ]))
}
