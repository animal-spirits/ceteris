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
    Observed, U_STAR_YEARS, load_observed, read_barro_lee, read_ilostat, read_ilostat_mean,
    read_pwt, read_wic,
};
use super::tariffs::{DTA, GRAVITY, read_cepii, read_dta};
use super::wdi::{read_rates, read_wdi, read_wdi_regions};
use super::xlsx::{self, Cell};
use super::{ByYear, WINDOW, YEAR};
use crate::json::Json;
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
    "baci-by-product",
    "baci-values",
    "trade-by-country",
    "rates",
    "cepii",
    "dta",
    "dose",
    "seds",
    "gem-coal",
    "gem-oil-gas",
];

const ICIO: &str = "1995-2000_SML.zip";
const GEM_COAL: &str = "Global Coal Mine Tracker, August 2026.xlsx";
const GEM_OIL_GAS: &str = "Global-Oil-and-Gas-Extraction-Tracker-March-2026.xlsx";
const BACI: &str = "BACI_HS92_V202601.zip";
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
        "baci-by-product" => baci_by_product(&cache(BACI))?.into(),
        "baci-values" => baci_values(&cache(BACI))?.into(),
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
                                ("values", Json::from(d.values)),
                                ("coverage", d.coverage.into()),
                            ]),
                        )
                    }),
            )
        }
        "seds" => read_seds(&cache(SEDS), YEAR)?.into(),
        "gem-coal" => assets(read_gem_coal(&cache(GEM_COAL))?),
        "gem-oil-gas" => assets(read_gem_oil_gas(&cache(GEM_OIL_GAS))?),
        _ => {
            return Err(format!(
                "unknown reader '{name}'; readers: {}",
                NAMES.join(", ")
            ));
        }
    })
}
