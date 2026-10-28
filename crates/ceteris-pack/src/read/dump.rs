//! `ceteris pack dump-reader NAME [ARGS]`: one reader's output on the fetch cache as canonical
//! JSON, for comparison with the Python reader (`scripts/parity/readers.py`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::icio::{
    ADB_INCOME, CountryIo, MADDISON, Wdi, aggregate, exports, load_wdi, read_adb_income,
    read_maddison,
};
use super::natural_earth::{province_names, read_codes, us_states};
use super::people::{
    Observed, U_STAR_YEARS, load_observed, read_barro_lee, read_ilostat, read_ilostat_mean,
    read_pwt, read_wic,
};
use super::wdi::{read_wdi, read_wdi_regions};
use super::xlsx::{self, Cell};
use super::{ByYear, YEAR};
use crate::fetch;
use crate::json::Json;

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
];

const ICIO: &str = "1995-2000_SML.zip";

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
        _ => {
            return Err(format!(
                "unknown reader '{name}'; readers: {}",
                NAMES.join(", ")
            ));
        }
    })
}
