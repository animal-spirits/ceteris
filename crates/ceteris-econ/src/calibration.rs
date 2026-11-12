//! The calibrated steady state of ADR 0089 (`specs/econ/SPEC.md`, Calibration): the tables
//! `ceteris calibrate` writes, read into the static inputs (A, VA0, L0,
//! a_VA, the coefficients, w0, the PIGL weights, the selection constants Gamma of ADR 0200 and
//! the market elasticities of ADR 0263)
//! and into the state a world starts from
//! ([`Steady`]: prices, stocks, the pipeline, money and the last D, Q and sales).
//!
//! Tables are keyed by pack province (`ProvinceData::pack`; the province itself for a
//! generated world) and must carry `calibration_format` [`FORMAT`] and the static digest of
//! the world they were calibrated from, the uncalibrated inputs under default parameters
//! (the `static_digest` of its export): tables of another format, of other inputs or of
//! other coefficients do not load.

use std::path::Path;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use ceteris_core::rows::Rows;
use ceteris_core::snapshot::hex;
use ceteris_geo::pack::{PackError, f64s, read_table, u8s, u32s};
use ceteris_trade::{Goods, TradeParams};

use crate::demand::{N_CATEGORIES, PiglWeights};
use crate::inputs::{EconInputs, MarketElasticity};
use crate::params::EconParams;
use crate::world::{active_cells, static_digest};

/// The `calibration_format` the loader reads (`ceteris_calibrate::steady::FORMAT`).
/// 9: the block limit gains (ADR 0267), and origins of competitively stored goods sell
/// Q / (1 + delta s) and hold coverage times sales, with the storage response in E_cell and the
/// block map (ADR 0091). Each branch numbered its own tables 8 before the merge. 10: the
/// metadata's `border_nu` is one value per good (ADR 0276).
pub const FORMAT: &str = "10";

/// The state at the start of a tick of the calibrated steady state (stage 5), in economy
/// indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Steady {
    /// Posted price per cell `[province * n_goods + g]`: origin markets of traded goods, 1
    /// for local goods and cells without a market.
    pub price: Vec<f64>,
    /// Last Q, D (by buyer class at traded origins) and sales per cell, D = Q = sales.
    pub supply: Vec<f64>,
    pub demand: Vec<f64>,
    pub demand_int: Vec<f64>,
    pub demand_hh: Vec<f64>,
    pub sales: Vec<f64>,
    /// Merchant stock S per cell, coverage times Q at traded origins.
    pub inventory: Vec<f64>,
    /// Last output, sales and profit per enterprise.
    pub output: Vec<f64>,
    pub ent_sales: Vec<f64>,
    pub profit: Vec<f64>,
    /// Input stock per enterprise, (1 + COVER) months of use.
    pub stock: Rows<f64>,
    /// Money per pop at the start of a tick (dividends and rebates; wages come at E2) and
    /// its income per month.
    pub money: Vec<f64>,
    pub income: Vec<f64>,
    /// Goods in flight: (pipeline buyer, pops then enterprises; good; ticks ahead, 1 to 4;
    /// units), due at the T1 that many ticks ahead.
    pub pipeline: Vec<(u32, u8, u8, f64)>,
}

fn err(msg: impl Into<String>) -> PackError {
    PackError(msg.into())
}

/// One calibration table, its format checked.
fn table(dir: &Path, name: &str) -> Result<RecordBatch, PackError> {
    let t = read_table(&dir.join(name))?;
    match t.schema().metadata().get("calibration_format") {
        Some(f) if f == FORMAT => Ok(t),
        found => Err(err(format!(
            "{name}: calibration format {found:?}, expected {FORMAT}; recalibrate with \
             `ceteris calibrate`"
        ))),
    }
}

fn bools(batch: &RecordBatch, name: &str) -> Result<Vec<bool>, PackError> {
    let column = batch
        .column_by_name(name)
        .ok_or_else(|| err(format!("missing column '{name}'")))?;
    let values = column
        .as_boolean_opt()
        .ok_or_else(|| err(format!("'{name}' is not bool")))?;
    Ok(values.iter().map(|v| v.unwrap_or(false)).collect())
}

fn per_good(batch: &RecordBatch, prefix: &str, n: usize) -> Result<Rows<f64>, PackError> {
    let cols = (0..n)
        .map(|g| f64s(batch, &format!("{prefix}{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Rows::from_flat(
        (0..batch.num_rows())
            .flat_map(|r| cols.iter().map(move |c| c[r]))
            .collect(),
        n,
    ))
}

/// The `static_digest` in the JSON metadata `calibration` of `econ_calib.arrow`.
fn calibrated_digest(t: &RecordBatch) -> Result<String, PackError> {
    let json = t
        .schema()
        .metadata()
        .get("calibration")
        .cloned()
        .ok_or_else(|| err("econ_calib.arrow has no calibration metadata"))?;
    let key = "\"static_digest\"";
    let missing = || err("calibration metadata has no static_digest");
    let after_key = &json[json.find(key).ok_or_else(missing)? + key.len()..];
    let value = after_key
        .trim_start()
        .strip_prefix(':')
        .and_then(|v| v.trim_start().strip_prefix('"'))
        .ok_or_else(missing)?;
    Ok(value.split('"').next().unwrap_or_default().to_owned())
}

/// The P05 per good in the JSON metadata `calibration` of `econ_calib.arrow` (ADRs 0092, 0276):
/// the border friction the steady state was solved at.
fn calibrated_border_nu(t: &RecordBatch, n: usize) -> Result<Vec<f64>, PackError> {
    let json = t
        .schema()
        .metadata()
        .get("calibration")
        .cloned()
        .ok_or_else(|| err("econ_calib.arrow has no calibration metadata"))?;
    let key = "\"border_nu\"";
    let bad = || {
        err(format!(
            "calibration metadata has no border_nu of {n} finite nonnegative values"
        ))
    };
    let after_key = &json[json.find(key).ok_or_else(bad)? + key.len()..];
    let list = after_key
        .trim_start()
        .strip_prefix(':')
        .and_then(|v| v.trim_start().strip_prefix('['))
        .and_then(|v| v.split(']').next())
        .ok_or_else(bad)?;
    let nu: Vec<f64> = list
        .split(',')
        .map(|v| v.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| bad())?;
    if nu.len() != n || nu.iter().any(|v| !(v.is_finite() && *v >= 0.0)) {
        return Err(bad());
    }
    Ok(nu)
}

impl EconInputs {
    /// Whether `dir` holds calibration tables.
    pub fn has_calibration(dir: &Path) -> bool {
        dir.join("econ_calib.arrow").exists()
    }

    /// Reads the calibration tables in `dir` (ADR 0089) into these inputs, which must be the
    /// uncalibrated world the tables were solved from: A, VA0, L0, a_VA, the coefficients
    /// (both the putty-clay vintage and the 2000 column that S09 FixedLeontief reads), w0, the
    /// PIGL weights, the selection constants Gamma (ADR 0200), the market elasticities that cap
    /// each market's price gain (ADR 0263, [`EconInputs::market_elasticity`]), and the steady
    /// state a world starts from ([`EconInputs::steady`]).
    pub fn calibrate(&mut self, dir: &Path) -> Result<(), PackError> {
        let ent_table = table(dir, "econ_calib.arrow")?;
        let digest = |trade: &TradeParams| hex(&static_digest(self, &EconParams::default(), trade));
        let want = digest(&TradeParams::default());
        let found = calibrated_digest(&ent_table)?;
        // a calibration solved under the approved storage values runs with storage on (RFC 0286
        // screen); its digest says so
        let storage = found != want
            && self.goods == Goods::p2()
            && found == digest(&TradeParams::default().with_storage());
        if found != want && !storage {
            return Err(err(format!(
                "the calibration in {} belongs to other inputs (static digest {found}, these \
                 inputs {want}); recalibrate with `ceteris calibrate`",
                dir.display()
            )));
        }
        let ng = self.n_goods();
        let border_nu = calibrated_border_nu(&ent_table, ng)?;
        let (np, n_pops, n_ent) = (self.n_provinces(), self.n_pops(), self.n_enterprises());
        let pack: Vec<u32> = if self.provinces.pack.is_empty() {
            (0..np as u32).collect()
        } else {
            self.provinces.pack.clone()
        };
        let province_of = |node: u32| {
            pack.binary_search(&node).map_err(|_| {
                err(format!(
                    "calibration names pack province {node}, not in the economy"
                ))
            })
        };
        let keys = |t: &RecordBatch, key: &str| -> Result<Vec<(usize, u8)>, PackError> {
            let (p, k) = (u32s(t, "province")?, u8s(t, key)?);
            p.into_iter()
                .zip(k)
                .map(|(p, k)| Ok((province_of(p)?, k)))
                .collect()
        };

        let ent_keys = keys(&ent_table, "good")?;
        let ours: Vec<(usize, u8)> = (0..n_ent)
            .map(|e| {
                let ent = &self.enterprises;
                (ent.province[e] as usize, ent.good[e])
            })
            .collect();
        if ent_keys != ours {
            return Err(err(
                "econ_calib.arrow rows are not the enterprises in order",
            ));
        }
        let coef = per_good(&ent_table, "coef_", ng)?;
        let ent = &mut self.enterprises;
        ent.a = f64s(&ent_table, "a")?;
        ent.va0 = f64s(&ent_table, "va0")?;
        ent.l0 = f64s(&ent_table, "l0")?;
        ent.a_va = f64s(&ent_table, "a_va")?;
        ent.coef = coef.clone();
        ent.column = coef;

        let provinces = table(dir, "econ_calib_provinces.arrow")?;
        let rows: Vec<u32> = u32s(&provinces, "province")?;
        if rows != pack {
            return Err(err(
                "econ_calib_provinces.arrow rows are not the provinces in order",
            ));
        }
        self.provinces.w0 = f64s(&provinces, "w0")?;

        let countries = table(dir, "econ_calib_countries.arrow")?;
        let nc = self.n_countries();
        if u32s(&countries, "country")? != (0..nc as u32).collect::<Vec<_>>() {
            return Err(err(
                "econ_calib_countries.arrow rows are not the countries in order",
            ));
        }
        let scale = f64s(&countries, "pigl_scale")?;
        let sigma = f64s(&countries, "pigl_sigma")?;
        let omega = (0..N_CATEGORIES)
            .map(|c| f64s(&countries, &format!("pigl_omega_{c}")))
            .collect::<Result<Vec<_>, _>>()?;
        let within = per_good(&countries, "pigl_within_", ng)?;
        // government's and investment's spending shares as calibrated (ADR 0099), where the
        // tables have them
        let fixed = |prefix: &str| -> Result<Option<Rows<f64>>, PackError> {
            if countries.column_by_name(&format!("{prefix}0")).is_some() {
                per_good(&countries, prefix, ng).map(Some)
            } else {
                Ok(None)
            }
        };
        let (gov, inv) = (fixed("gov_")?, fixed("inv_")?);
        let row =
            |r: &Option<Rows<f64>>, c: usize| r.as_ref().map_or(Vec::new(), |r| r[c].to_vec());
        self.countries.pigl = Some(
            (0..nc)
                .map(|c| PiglWeights {
                    scale: scale[c],
                    omega: std::array::from_fn(|j| omega[j][c]),
                    within: within[c].to_vec(),
                    category: self.goods.category.clone(),
                    sigma: sigma[c],
                    gov: row(&gov, c),
                    inv: row(&inv, c),
                })
                .collect(),
        );

        let cells = table(dir, "econ_calib_cells.arrow")?;
        let cell_keys = keys(&cells, "good")?;
        let in_order = cell_keys.len() == np * ng
            && cell_keys
                .iter()
                .enumerate()
                .all(|(c, &(i, g))| i == c / ng && usize::from(g) == c % ng);
        if !in_order {
            return Err(err(
                "econ_calib_cells.arrow rows are not the cells in order",
            ));
        }
        if bools(&cells, "active")? != active_cells(self) {
            return Err(err(
                "econ_calib_cells.arrow marks other cells active than the world",
            ));
        }
        let active = active_cells(self);
        let price = f64s(&cells, "price")?
            .into_iter()
            .zip(&active)
            .map(|(p, &on)| if on { p } else { 1.0 })
            .collect();
        let gamma = f64s(&cells, "gamma")?;
        if let Some(c) = gamma.iter().position(|&x| !(x > 0.0 && x.is_finite())) {
            return Err(err(format!(
                "econ_calib_cells.arrow: cell {c} has selection constant {}",
                gamma[c]
            )));
        }

        let market_elasticity = market_elasticity(&cells)?;

        let pops = table(dir, "econ_calib_pops.arrow")?;
        let pop_keys = keys(&pops, "edu")?;
        let our_pops: Vec<(usize, u8)> = (0..n_pops)
            .map(|k| (self.pops.province[k] as usize, self.pops.edu[k]))
            .collect();
        if pop_keys != our_pops {
            return Err(err("econ_calib_pops.arrow rows are not the pops in order"));
        }

        let flight = table(dir, "econ_calib_pipeline.arrow")?;
        let (kind, good, ahead, units) = (
            u8s(&flight, "buyer")?,
            u8s(&flight, "good")?,
            u8s(&flight, "ahead")?,
            f64s(&flight, "units")?,
        );
        let flight_keys = keys(&flight, "key")?;
        let mut pipeline = Vec::with_capacity(units.len());
        for r in 0..units.len() {
            let found = match kind[r] {
                0 => our_pops.binary_search(&flight_keys[r]),
                1 => ours.binary_search(&flight_keys[r]).map(|e| n_pops + e),
                k => return Err(err(format!("econ_calib_pipeline.arrow: buyer kind {k}"))),
            };
            let buyer = found
                .map_err(|_| err(format!("econ_calib_pipeline.arrow row {r}: no such buyer")))?;
            let traded = self.goods.is_traded(usize::from(good[r]));
            if !traded || !(1..=4).contains(&ahead[r]) || units[r].is_nan() || units[r] <= 0.0 {
                return Err(err(format!("econ_calib_pipeline.arrow row {r}: bad entry")));
            }
            pipeline.push((buyer as u32, good[r], ahead[r], units[r]));
        }

        self.provinces.gamma = Rows::from_flat(gamma, ng);
        self.border_nu = Some(border_nu);
        self.storage = storage;
        self.market_elasticity = Some(market_elasticity);
        self.steady = Some(std::sync::Arc::new(Steady {
            price,
            supply: f64s(&cells, "supply")?,
            demand: f64s(&cells, "demand")?,
            demand_int: f64s(&cells, "demand_int")?,
            demand_hh: f64s(&cells, "demand_hh")?,
            sales: f64s(&cells, "sales")?,
            inventory: f64s(&cells, "inventory")?,
            output: f64s(&ent_table, "output")?,
            ent_sales: f64s(&ent_table, "sales")?,
            profit: f64s(&ent_table, "profit")?,
            stock: per_good(&ent_table, "stock_", ng)?,
            money: f64s(&pops, "money")?,
            income: f64s(&pops, "income")?,
            pipeline,
        }));
        Ok(())
    }
}

/// The market elasticities E and eps of `econ_calib_cells.arrow` (ADR 0263), finite and not
/// negative, and the block limit gains (ADR 0267), positive (infinite where no block caps).
fn market_elasticity(cells: &RecordBatch) -> Result<MarketElasticity, PackError> {
    let m = MarketElasticity {
        demand: f64s(cells, "demand_elasticity")?,
        supply: f64s(cells, "supply_elasticity")?,
        limit: [
            f64s(cells, "block_limit_default")?,
            f64s(cells, "block_limit_frictionless")?,
        ],
    };
    for (name, v) in [("demand", &m.demand), ("supply", &m.supply)] {
        if let Some(c) = v.iter().position(|&x| !(x >= 0.0 && x.is_finite())) {
            return Err(err(format!(
                "econ_calib_cells.arrow: cell {c} has {name} elasticity {}",
                v[c]
            )));
        }
    }
    for v in &m.limit {
        if let Some(c) = v.iter().position(|&x| x.is_nan() || x <= 0.0) {
            return Err(err(format!(
                "econ_calib_cells.arrow: cell {c} has block limit gain {}",
                v[c]
            )));
        }
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use ceteris_trade::goods::p2::{N as N_GOODS, TRADED as N_TRADED};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use arrow_array::Float64Array;
    use arrow_ipc::writer::FileWriter;
    use arrow_schema::{DataType, Field, Schema};

    use super::*;

    fn batch(metadata: &[(&str, &str)]) -> RecordBatch {
        let meta: BTreeMap<String, String> = metadata
            .iter()
            .map(|&(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        let schema = Arc::new(Schema::new_with_metadata(
            vec![Field::new("x", DataType::Float64, false)],
            meta,
        ));
        RecordBatch::try_new(schema, vec![Arc::new(Float64Array::from(vec![1.0]))]).unwrap()
    }

    #[test]
    fn digest_is_read_with_or_without_json_whitespace() {
        for json in [
            r#"{"static_digest": "abc"}"#,
            r#"{"static_digest":"abc"}"#,
            r#"{ "other": 1, "static_digest" : "abc" , "n": 2 }"#,
        ] {
            let t = batch(&[("calibration", json)]);
            assert_eq!(calibrated_digest(&t).unwrap(), "abc", "{json}");
        }
    }

    #[test]
    fn border_nu_is_read_per_good() {
        for json in [
            r#"{"border_nu": [0.25, 1.5, 0.0625, 3e-05, 0.25, 0.0, 2, 0.3, 0.3, 0.3], "static_digest": "abc"}"#,
            r#"{ "static_digest":"abc","border_nu" :[ 0.25,1.5 , 0.0625,3e-05,0.25,0.0,2,0.3,0.3,0.3 ] }"#,
        ] {
            let t = batch(&[("calibration", json)]);
            assert_eq!(
                calibrated_border_nu(&t, N_GOODS).unwrap(),
                [0.25, 1.5, 0.0625, 3e-05, 0.25, 0.0, 2.0, 0.3, 0.3, 0.3],
                "{json}"
            );
        }
        for json in [
            r#"{"static_digest": "abc"}"#,
            r#"{"border_nu": [0.25, 1.5, 0.0625, 3e-05]}"#,
            r#"{"border_nu": [0.25, 1.5, 0.0625, 3e-05, 0.25, 0.0, 2, 0.3, 0.3]}"#,
            r#"{"border_nu": [0.25, 1.5, 0.0625, 3e-05, 0.25, 0.0, 2, 0.3, 0.3, -1.0]}"#,
            r#"{"border_nu": [0.25, 1.5, 0.0625, 3e-05, 0.25, 0.0, 2, 0.3, 0.3, "x"]}"#,
        ] {
            let t = batch(&[("calibration", json)]);
            assert!(calibrated_border_nu(&t, N_GOODS).is_err(), "{json}");
        }
    }

    #[test]
    fn digest_missing_is_an_error() {
        assert!(calibrated_digest(&batch(&[])).is_err());
        let t = batch(&[("calibration", r#"{"digest": "abc"}"#)]);
        assert!(calibrated_digest(&t).is_err());
    }

    fn elasticity_cells(demand: f64, supply: f64, limit: f64) -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("demand_elasticity", DataType::Float64, false),
            Field::new("supply_elasticity", DataType::Float64, false),
            Field::new("block_limit_default", DataType::Float64, false),
            Field::new("block_limit_frictionless", DataType::Float64, false),
        ]));
        let column = |x: f64| Arc::new(Float64Array::from(vec![1.0, x])) as _;
        let columns = vec![
            column(demand),
            column(supply),
            column(f64::INFINITY),
            column(limit),
        ];
        RecordBatch::try_new(schema, columns).unwrap()
    }

    #[test]
    fn market_elasticities_must_be_finite_and_not_negative() {
        let cells = |demand: f64, supply: f64| elasticity_cells(demand, supply, f64::INFINITY);
        let m = market_elasticity(&cells(11.57, 0.0)).unwrap();
        assert_eq!((m.demand, m.supply), (vec![1.0, 11.57], vec![1.0, 0.0]));
        for (demand, supply, name) in [
            (f64::NAN, 1.0, "demand"),
            (-0.5, 1.0, "demand"),
            (f64::INFINITY, 1.0, "demand"),
            (1.0, f64::NAN, "supply"),
            (1.0, -1e-12, "supply"),
        ] {
            let message = market_elasticity(&cells(demand, supply))
                .unwrap_err()
                .to_string();
            assert!(
                message.contains(&format!("cell 1 has {name} elasticity")),
                "{message}"
            );
        }
    }

    /// ADR 0267: a block limit gain is positive, infinite where no block caps.
    #[test]
    fn block_limits_must_be_positive() {
        let m = market_elasticity(&elasticity_cells(1.0, 1.0, 0.025)).unwrap();
        assert_eq!(m.limit[1], vec![1.0, 0.025]);
        assert_eq!(m.limit[0], vec![1.0, f64::INFINITY]);
        for b in [0.0, -0.1, f64::NAN] {
            let message = market_elasticity(&elasticity_cells(1.0, 1.0, b))
                .unwrap_err()
                .to_string();
            assert!(message.contains("cell 1 has block limit gain"), "{message}");
        }
    }

    #[test]
    fn table_of_another_calibration_format_is_rejected() {
        let dir = std::env::temp_dir().join(format!("ceteris-calib-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, format: &str| {
            let t = batch(&[("calibration_format", format)]);
            let file = std::fs::File::create(dir.join(name)).unwrap();
            let mut w = FileWriter::try_new(file, &t.schema()).unwrap();
            w.write(&t).unwrap();
            w.finish().unwrap();
        };
        write("current.arrow", FORMAT);
        write("old.arrow", "2");
        let accepted = table(&dir, "current.arrow").is_ok();
        let rejected = table(&dir, "old.arrow");
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(accepted);
        let message = rejected.unwrap_err().to_string();
        assert!(message.contains("calibration format"), "{message}");
    }
}
