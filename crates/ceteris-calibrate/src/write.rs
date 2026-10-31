//! The calibration tables (`specs/econ/SPEC.md`, Calibration table), keyed by pack province, as
//! `ceteris_econ::calibration` reads them; the reference inputs read back from them; the
//! summary as JSON.
//!
//! | file | columns |
//! |---|---|
//! | `econ_calib` | province u32, good u8, a, va0, l0, a_va, price, output, sales, profit, coef_0 to coef_9, stock_0 to stock_9 f64; schema metadata `calibration` (JSON: format, r, border_nu (P05 per good, ADRs 0092, 0276), passes, sinkhorn_residual, static_digest) |
//! | `econ_calib_cells` | province u32, good u8, active bool, price, consumer_price, gamma, lead, demand_elasticity, supply_elasticity, block_limit_default, block_limit_frictionless, supply, demand, demand_int, demand_hh, sales, inventory f64 |
//! | `econ_calib_provinces` | province u32, w0, employment, income f64 |
//! | `econ_calib_pops` | province u32, edu u8, money, income, wage f64 |
//! | `econ_calib_pipeline` | buyer u8 (0 pops, 1 enterprises), province u32, key u8 (edu or good), good u8, ahead u8, units f64 |
//! | `econ_calib_countries` | country u32, u_star, rebate, pigl_scale, pigl_sigma (the within-pop dispersion, ADR 0094), pigl_omega_0 to pigl_omega_2, pigl_within_0 to pigl_within_9 f64 |
//!
//! Every table carries `calibration_format` [`FORMAT`] in its schema metadata.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt64Type};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, RecordBatch, UInt8Array, UInt32Array,
};
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_ipc::writer::FileWriter;
use arrow_schema::{DataType, Field, Schema};
use ceteris_reference::demand::{N_CATEGORIES, Weights};
use ceteris_reference::io::{Inputs, Mat};

use crate::border::at_border_nu;
use crate::json::Json;
use crate::steady::{FORMAT, SLOTS, Steady};

fn u32s(v: impl IntoIterator<Item = usize>) -> ArrayRef {
    Arc::new(UInt32Array::from_iter_values(
        v.into_iter()
            .map(|i| u32::try_from(i).expect("index fits u32")),
    ))
}

fn u8s(v: impl IntoIterator<Item = usize>) -> ArrayRef {
    Arc::new(UInt8Array::from_iter_values(
        v.into_iter().map(|i| u8::try_from(i).expect("key fits u8")),
    ))
}

fn f64s(v: impl IntoIterator<Item = f64>) -> ArrayRef {
    Arc::new(Float64Array::from_iter_values(v))
}

/// One table, every field nullable as pyarrow writes them, `calibration_format` and `extra` in
/// the schema metadata.
fn write_table(
    path: &Path,
    columns: Vec<(String, ArrayRef)>,
    extra: Option<(&str, String)>,
) -> Result<(), String> {
    let fields: Vec<Field> = columns
        .iter()
        .map(|(name, a)| Field::new(name, a.data_type().clone(), true))
        .collect();
    let mut metadata = BTreeMap::from([("calibration_format".to_owned(), FORMAT.to_owned())]);
    if let Some((k, v)) = extra {
        metadata.insert(k.to_owned(), v);
    }
    // arrow-rs writes schema metadata in sorted key order, so the bytes are deterministic.
    let schema = Arc::new(Schema::new_with_metadata(fields, metadata));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        columns.into_iter().map(|(_, a)| a).collect(),
    )
    .map_err(|e| e.to_string())?;
    let err = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = File::create(path).map_err(|e| err(&e))?;
    let mut w = FileWriter::try_new(BufWriter::new(file), &schema).map_err(|e| err(&e))?;
    w.write(&batch).map_err(|e| err(&e))?;
    w.finish().map_err(|e| err(&e))?;
    w.into_inner()
        .map_err(|e| err(&e))?
        .flush()
        .map_err(|e| err(&e))
}

/// A pipeline's rows (buyer x good) and each buyer's province and key.
type Buyers<'a> = (&'a [[f64; SLOTS]], &'a [usize], &'a [usize]);

fn named(name: &str, a: ArrayRef) -> (String, ArrayRef) {
    (name.to_owned(), a)
}

/// Writes the six calibration tables of `s` into `out`, created if needed, keyed by
/// `pack_province` (P,).
pub fn write(out: &Path, inp: &Inputs, s: &Steady, pack_province: &[usize]) -> Result<(), String> {
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    let edu = inp
        .pop_edu
        .as_ref()
        .ok_or("the inputs carry no pop education")?;
    if pack_province.len() != n_p {
        return Err(format!(
            "{} pack provinces for {n_p} provinces",
            pack_province.len()
        ));
    }
    fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let (ep, eg) = (&inp.ent_province, &inp.ent_good);
    let n_e = eg.len();
    let pack = |p: &[usize]| u32s(p.iter().map(|&p| pack_province[p]));
    let s_l = inp.ent_s_l();
    let stock = s.stock();
    let column = |m: &Mat, k: usize| f64s((0..m.rows).map(|r| m[(r, k)]));

    let meta = Json::object([
        ("format", Json::Str(FORMAT.into())),
        (
            "r",
            Json::List(s.r.iter().map(|&x| Json::Float(x)).collect()),
        ),
        (
            "border_nu",
            Json::List(inp.border_nu.iter().map(|&x| Json::Float(x)).collect()),
        ),
        ("passes", Json::Int(s.info.passes as i64)),
        ("sinkhorn_residual", Json::Float(s.info.sinkhorn_residual())),
        ("static_digest", Json::Str(inp.static_digest.clone())),
    ]);
    let mut ent = vec![
        named("province", pack(ep)),
        named("good", u8s(eg.iter().copied())),
        named("a", f64s(s.a.iter().copied())),
        named("va0", f64s(s.va0.iter().copied())),
        named("l0", f64s(s.l0.iter().copied())),
        named("a_va", f64s(s.a_va.iter().copied())),
        named(
            "price",
            f64s((0..n_e).map(|e| {
                if inp.traded[eg[e]] {
                    s.price[(ep[e], eg[e])]
                } else {
                    1.0
                }
            })),
        ),
        named("output", f64s(s.output.iter().copied())),
        named("sales", f64s(s.value.iter().copied())),
        named("profit", f64s((0..n_e).map(|e| (1.0 - s_l[e]) * s.va0[e]))),
    ];
    ent.extend((0..n_g).map(|k| (format!("coef_{k}"), column(&s.coef, k))));
    ent.extend((0..n_g).map(|k| (format!("stock_{k}"), column(&stock, k))));
    write_table(
        &out.join("econ_calib.arrow"),
        ent,
        Some(("calibration", meta.compact())),
    )?;

    let cell_p: Vec<usize> = (0..n_p * n_g).map(|i| i / n_g).collect();
    let flat = |m: &Mat| f64s(m.data.iter().copied());
    let inventory = s.inventory();
    let sales = s.sales();
    write_table(
        &out.join("econ_calib_cells.arrow"),
        vec![
            named("province", pack(&cell_p)),
            named("good", u8s((0..n_p * n_g).map(|i| i % n_g))),
            named(
                "active",
                Arc::new(BooleanArray::from_iter(
                    inp.active.iter().flatten().map(|&a| Some(a)),
                )),
            ),
            named("price", flat(&s.price)),
            named("consumer_price", flat(&s.consumer)),
            named("gamma", flat(&s.gamma)),
            named("lead", flat(&s.lead)),
            named("demand_elasticity", flat(&s.demand_elasticity)),
            named("supply_elasticity", flat(&s.supply_elasticity)),
            named("block_limit_default", flat(&s.block_limit[0])),
            named("block_limit_frictionless", flat(&s.block_limit[1])),
            named("supply", flat(&s.supply)),
            named("demand", flat(&sales)),
            named("demand_int", flat(&s.orders[0])),
            named("demand_hh", flat(&s.orders[1])),
            named("sales", flat(&sales)),
            named("inventory", flat(&inventory)),
        ],
        None,
    )?;

    let bincount = |index: &[usize], w: &[f64]| {
        let mut out = vec![0.0; n_p];
        for (&i, &w) in index.iter().zip(w) {
            out[i] += w;
        }
        out
    };
    let provinces: Vec<usize> = (0..n_p).collect();
    write_table(
        &out.join("econ_calib_provinces.arrow"),
        vec![
            named("province", pack(&provinces)),
            named("w0", f64s(s.w0.iter().copied())),
            named("employment", f64s(bincount(ep, &s.l0))),
            named("income", f64s(bincount(&inp.pop_province, &s.pop_income))),
        ],
        None,
    )?;

    write_table(
        &out.join("econ_calib_pops.arrow"),
        vec![
            named("province", pack(&inp.pop_province)),
            named("edu", u8s(edu.iter().copied())),
            named("money", f64s(s.pop_money())),
            named("income", f64s(s.pop_income.iter().copied())),
            named("wage", f64s(s.pop_wage.iter().copied())),
        ],
        None,
    )?;

    let (mut buyer, mut province, mut key, mut good, mut ahead, mut units) =
        (vec![], vec![], vec![], vec![], vec![], vec![]);
    let buyers: [Buyers; 2] = [
        (&s.pop_pipeline, &inp.pop_province, edu),
        (&s.ent_pipeline, ep, eg),
    ];
    for (kind, (pipeline, at, keys)) in buyers.into_iter().enumerate() {
        for (row, slots) in pipeline.iter().enumerate() {
            let (b, g) = (row / n_g, row % n_g);
            for (j, &u) in slots.iter().enumerate().filter(|&(_, &u)| u > 0.0) {
                buyer.push(kind);
                province.push(at[b]);
                key.push(keys[b]);
                good.push(g);
                ahead.push(j + 1);
                units.push(u);
            }
        }
    }
    write_table(
        &out.join("econ_calib_pipeline.arrow"),
        vec![
            named("buyer", u8s(buyer)),
            named("province", pack(&province)),
            named("key", u8s(key)),
            named("good", u8s(good)),
            named("ahead", u8s(ahead)),
            named("units", f64s(units)),
        ],
        None,
    )?;

    let w = &s.weights;
    let mut countries = vec![
        named("country", u32s(0..n_c)),
        named("u_star", f64s(s.u_star.iter().copied())),
        named("rebate", f64s(s.rebate.iter().copied())),
        named("pigl_scale", f64s(w.scale.iter().copied())),
        named("pigl_sigma", f64s(w.sigma.iter().copied())),
    ];
    countries.extend((0..N_CATEGORIES).map(|c| (format!("pigl_omega_{c}"), column(&w.omega, c))));
    countries.extend((0..n_g).map(|g| (format!("pigl_within_{g}"), column(&w.within, g))));
    write_table(&out.join("econ_calib_countries.arrow"), countries, None)
}

/// One Arrow table, its batches concatenated column by column on read.
pub(crate) struct Table {
    name: String,
    batches: Vec<RecordBatch>,
    metadata: BTreeMap<String, String>,
}

impl Table {
    pub(crate) fn open(path: &Path) -> Result<Table, String> {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let reader = FileReader::try_new(file, None).map_err(|e| format!("{name}: {e}"))?;
        let metadata = reader
            .schema()
            .metadata()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let batches = reader
            .collect::<Result<_, _>>()
            .map_err(|e| format!("{name}: {e}"))?;
        Ok(Table {
            name,
            batches,
            metadata,
        })
    }

    fn column(&self, col: &str, to: &DataType) -> Result<Vec<ArrayRef>, String> {
        self.batches
            .iter()
            .map(|b| {
                let a = b
                    .column_by_name(col)
                    .ok_or_else(|| format!("{}: no column {col}", self.name))?;
                if a.null_count() > 0 {
                    return Err(format!("{}.{col}: nulls", self.name));
                }
                cast(a, to).map_err(|e| format!("{}.{col}: {e}", self.name))
            })
            .collect()
    }

    pub(crate) fn f64(&self, col: &str) -> Result<Vec<f64>, String> {
        Ok(self
            .column(col, &DataType::Float64)?
            .iter()
            .flat_map(|a| a.as_primitive::<Float64Type>().values().to_vec())
            .collect())
    }

    pub(crate) fn strings(&self, col: &str) -> Result<Vec<String>, String> {
        Ok(self
            .column(col, &DataType::Utf8)?
            .iter()
            .flat_map(|a| {
                a.as_string::<i32>()
                    .iter()
                    .map(|s| s.unwrap_or_default().to_owned())
                    .collect::<Vec<_>>()
            })
            .collect())
    }

    pub(crate) fn bool(&self, col: &str) -> Result<Vec<bool>, String> {
        Ok(self
            .column(col, &DataType::Boolean)?
            .iter()
            .flat_map(|a| a.as_boolean().values().iter().collect::<Vec<_>>())
            .collect())
    }

    pub(crate) fn rows(&self) -> usize {
        self.batches.iter().map(RecordBatch::num_rows).sum()
    }

    pub(crate) fn usize(&self, col: &str) -> Result<Vec<usize>, String> {
        Ok(self
            .column(col, &DataType::UInt64)?
            .iter()
            .flat_map(|a| a.as_primitive::<UInt64Type>().values().to_vec())
            .map(|v| v as usize)
            .collect())
    }
}

/// (C,) u* of the pack's `econ_countries.arrow`, in its row order (the export's country order).
pub fn read_u_star(pack: &Path) -> Result<Vec<f64>, String> {
    Table::open(&pack.join("econ_countries.arrow"))?.f64("u_star")
}

/// (P,) the pack province of each province of an export from a pack (`pack_provinces.arrow`).
pub fn read_pack_provinces(export: &Path) -> Result<Vec<usize>, String> {
    Table::open(&export.join("pack_provinces.arrow"))?.usize("pack")
}

/// P05 of each of `n_goods` goods in the calibration tables installed in `pack` (ADRs 0092,
/// 0276): the border friction their steady state was solved at.
pub fn read_border_nu(pack: &Path, n_goods: usize) -> Result<Vec<f64>, String> {
    let ent = Table::open(&pack.join("econ_calib.arrow"))?;
    if ent.metadata.get("calibration_format").map(String::as_str) != Some(FORMAT) {
        return Err(format!(
            "econ_calib.arrow: calibration format is not {FORMAT}"
        ));
    }
    let meta: serde_json::Value = serde_json::from_str(
        ent.metadata
            .get("calibration")
            .ok_or("econ_calib.arrow has no calibration metadata")?,
    )
    .map_err(|e| format!("econ_calib.arrow: {e}"))?;
    border_nu_of(&meta, n_goods)
}

fn border_nu_of(meta: &serde_json::Value, n_goods: usize) -> Result<Vec<f64>, String> {
    meta["border_nu"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .map(serde_json::Value::as_f64)
                .collect::<Option<Vec<_>>>()
        })
        .filter(|v| v.len() == n_goods)
        .ok_or_else(|| "econ_calib.arrow: border_nu is not one number per good".to_owned())
}

/// [`crate::steady::reference_inputs`] and [`crate::steady::reference_point`] from the
/// calibration tables installed in `pack`, for the export `inp` of that pack's data state
/// (`pack_province` its `pack_provinces` column), without calibrating again; errs for tables of
/// other inputs.
pub fn read_reference(
    pack: &Path,
    inp: &Inputs,
    pack_province: &[usize],
) -> Result<(Inputs, Vec<f64>), String> {
    let read = |name: &str| -> Result<Table, String> {
        let t = Table::open(&pack.join(name))?;
        if t.metadata.get("calibration_format").map(String::as_str) != Some(FORMAT) {
            return Err(format!("{name}: calibration format is not {FORMAT}"));
        }
        Ok(t)
    };
    let ent = read("econ_calib.arrow")?;
    let meta: serde_json::Value = serde_json::from_str(
        ent.metadata
            .get("calibration")
            .ok_or("econ_calib.arrow has no calibration metadata")?,
    )
    .map_err(|e| format!("econ_calib.arrow: {e}"))?;
    if meta["static_digest"].as_str() != Some(inp.static_digest.as_str()) {
        return Err("the calibration tables belong to other inputs".into());
    }
    let border_nu = border_nu_of(&meta, inp.n_goods())?;
    // the export holds the data state's border friction; the calibration was solved at its own
    let calibrated = at_border_nu(inp, &border_nu)?;
    let inp = &calibrated;
    let n_g = inp.n_goods();
    let rows = |t: &Table, key: &str| -> Result<BTreeMap<(usize, usize), usize>, String> {
        let (p, k) = (t.usize("province")?, t.usize(key)?);
        Ok(p.into_iter()
            .zip(k)
            .enumerate()
            .map(|(r, pk)| (pk, r))
            .collect())
    };
    let missing = |what: &str| format!("the calibration tables have no row for {what}");
    let row = rows(&ent, "good")?;
    let e: Vec<usize> = inp
        .ent_province
        .iter()
        .zip(&inp.ent_good)
        .map(|(&p, &g)| {
            row.get(&(pack_province[p], g))
                .copied()
                .ok_or_else(|| missing("an enterprise"))
        })
        .collect::<Result<_, _>>()?;
    let col = |name: &str| -> Result<Vec<f64>, String> {
        let v = ent.f64(name)?;
        Ok(e.iter().map(|&r| v[r]).collect())
    };
    let coef_cols = (0..n_g)
        .map(|k| col(&format!("coef_{k}")))
        .collect::<Result<Vec<_>, _>>()?;
    let coef = Mat::from_fn(e.len(), n_g, |r, k| coef_cols[k][r]);

    let provinces = read("econ_calib_provinces.arrow")?;
    let at: BTreeMap<usize, usize> = provinces
        .usize("province")?
        .into_iter()
        .enumerate()
        .map(|(r, p)| (p, r))
        .collect();
    let w0_rows = provinces.f64("w0")?;
    let w0: Vec<f64> = pack_province
        .iter()
        .map(|p| {
            at.get(p)
                .map(|&r| w0_rows[r])
                .ok_or_else(|| missing("a province"))
        })
        .collect::<Result<_, _>>()?;

    let countries = read("econ_calib_countries.arrow")?;
    let n_c = countries.f64("pigl_scale")?.len();
    let by_column = |prefix: &str, n: usize| -> Result<Mat, String> {
        let cols = (0..n)
            .map(|j| countries.f64(&format!("{prefix}{j}")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Mat::from_fn(n_c, n, |c, j| cols[j][c]))
    };
    let weights = Weights {
        scale: countries.f64("pigl_scale")?,
        omega: by_column("pigl_omega_", N_CATEGORIES)?,
        within: by_column("pigl_within_", n_g)?,
        sigma: countries.f64("pigl_sigma")?,
    };

    let cells = read("econ_calib_cells.arrow")?;
    let at = rows(&cells, "good")?;
    let n_p = inp.n_provinces();
    let cell = |name: &str| -> Result<Vec<f64>, String> {
        let v = cells.f64(name)?;
        (0..n_p * n_g)
            .map(|i| {
                at.get(&(pack_province[i / n_g], i % n_g))
                    .map(|&r| v[r])
                    .ok_or_else(|| missing("a cell"))
            })
            .collect()
    };
    let price = cell("price")?;
    let gamma = cell("gamma")?;

    let mut reference = inp.clone();
    reference.ent_a = col("a")?;
    reference.ent_va0 = col("va0")?;
    reference.ent_l0 = col("l0")?;
    reference.w0.clone_from(&w0);
    reference.u_star = countries.f64("u_star")?;
    reference.calibrated_coef = Some(coef);
    reference.calibrated_a_va = Some(col("a_va")?);
    reference.calibrated_weights = Some(weights);
    reference.gamma = Some(Mat {
        rows: n_p,
        cols: n_g,
        data: gamma,
    });
    let mut x: Vec<f64> = inp.active_cells().iter().map(|&i| price[i].ln()).collect();
    x.extend(w0.iter().map(|w| w.ln()));
    Ok((reference, x))
}
