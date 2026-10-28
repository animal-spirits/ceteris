//! The 2000 economic tables of pack format 3 (specs/econ/SPEC.md, Pack additions), built from the
//! P1 tables of a pack and the fetch cache.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::{Array, ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray};
use arrow_ipc::reader::FileReader;

use super::{goods, icio, people};
use crate::raster::geotiff;
use crate::raster::population::{BLOCK_ROWS, CellOwner, province_population};
use crate::read::YEAR;
use crate::read::icio::{aggregate, load_wdi};
use crate::read::natural_earth::read_codes;
use crate::read::people::load_observed;
use crate::table;

pub const WORLDPOP: &str = "ppp_2000_1km_Aggregated.tif";
pub const ADMIN0: &str = "ne_10m_admin_0_countries.zip";
pub const BACI: &str = "BACI_HS92_V202601.zip";
pub const HS_KEY: &str = "OECD-Bilateral-Trade-in-Goods-End-use-Conversion-Key.xlsx";
/// P02 inventory target coverage s_g in months, the prior's start (trade SPEC, Parameters;
/// TradeParams::coverage).
pub const COVERAGE: [f64; goods::NAMES.len()] = [3.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 0.0, 0.0, 0.0];

/// The written tables: file name and SHA-256 of its bytes, in writing order.
#[derive(Debug)]
pub struct BuildReport {
    pub tables: Vec<(String, [u8; 32])>,
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

/// `econ_goods.arrow`: name, whether the good is traded, tonnes per USD and inventory coverage.
fn goods_columns(weight: [f64; goods::NAMES.len()]) -> Vec<(String, ArrayRef)> {
    let n = goods::NAMES.len();
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
    ]
}

/// Writes the econ tables of `pack` (its P1 tables) into `out`, reading the sources from `cache`.
pub fn build(pack: &Path, out: &Path, cache: &Path) -> Result<BuildReport, String> {
    let [codes, sovereign] = strings(&pack.join("countries.arrow"), ["code", "sovereign"])?;
    let [country] = strings(&pack.join("provinces.arrow"), ["country"])?;
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
    let world = icio::impute_pack(
        &countries,
        &aggregate(&cache.join(icio::ICIO), YEAR)?,
        &wdi,
        &obs,
        &iso,
        &worldpop,
    )?;

    let weight = goods::weights_from_baci(&cache.join(BACI), &cache.join(HS_KEY))?;

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut countries_columns: Vec<(String, ArrayRef)> =
        vec![("code".into(), Arc::new(StringArray::from(codes.clone())))];
    countries_columns.extend(icio::country_columns(&codes, &world, &wdi, &iso)?);
    countries_columns.extend(people::country_columns(&codes, &labour)?);
    let tables = vec![
        write(out, "econ_goods.arrow", goods_columns(weight))?,
        write(out, "econ_countries.arrow", countries_columns)?,
        write(out, "econ_io.arrow", icio::io_columns(&codes, &world)?)?,
        write(
            out,
            "econ_provinces.arrow",
            people::province_columns(&country, &population, &labour)?,
        )?,
        write_batch(
            out,
            "econ_pops.arrow",
            &people::pops(&country, &population, &labour)?,
        )?,
    ];
    Ok(BuildReport { tables })
}
