//! Arrow export of the P2 world: the only interface between the Rust core and the Python GE
//! reference model (`specs/econ/SPEC.md`, GE reference model), which rebuilds the same
//! economy from it. One record batch per table, fixed column order, no timestamps: the same
//! world gives byte-identical files. Every table carries `export_format` in its schema
//! metadata. Indices are zero-based IDs in the order of the inputs.
//!
//! [`write_inputs`] writes the static economy:
//!
//! | file | columns |
//! |---|---|
//! | `goods` | good u8, name str, traded bool, tariff_group i8 (-1 for local goods), weight f64, coverage f64, time_cost f64, theta f64 (P01, 0 for local goods), theta_class u8 (ADR 0023, 0 for local goods), category u8 (PIGL category, 0 A, 1 M, 2 S, ADR 0021) |
//! | `gateways` | gateway u32, country u32 |
//! | `countries` | country u32, s_l f64, unemployment f64, gini f64, omega_0 to omega_9 f64 |
//! | `country_io` | country u32, input u8 (k), sector u8 (g), a f64 (a_kg: units of k per unit of g) |
//! | `tariffs` | importer u32, exporter u32, group u8, rate f32 |
//! | `provinces` | province u32, country u32, population f64, w0 f64 |
//! | `pack_provinces` | province u32, pack u32 (`ProvinceData::pack`); written only for a world loaded from a pack, which the calibration keys by pack province |
//! | `pops` | pop u32, province u32, edu u8, labour_force f64, population f64 |
//! | `enterprises` | enterprise u32, province u32, good u8, a f64, va0 f64, l0 f64, va_target f64 |
//! | `active` | province u32, good u8, active bool (cells with a market, as the world computes them: origin markets of traded goods, province markets of local goods) |
//! | `legs` | kind u8 (0 domestic, 1 access, 2 crossing), from u32, to u32, cost_per_t f64, hours f64 |
//! | `kernel_domestic` | good u8, o u32, d u32, k f32, tau f64 |
//! | `kernel_access` | good u8, province u32, gateway u32, k f32, tau f64 |
//! | `kernel_crossing` | good u8, a u32, b u32, k f32, tau f64, border_tau f64, lag u8, tariff f32 |
//! | `pigl_check` | draw u32, country u32, money f64 (mean spending per person), sigma f64, price_0 to price_9 f64 (some infinite), x_0 to x_9 f64: `demand::pigl` with the country's weights on [`PIGL_CHECK_DRAWS`] random draws, which the reference reproduces |
//! | `params.json` | both parameter sets by field name, the register IDs, constants |
//!
//! Kernel rows list the traded goods in order, then origins in order, then the entries of the
//! row in kernel order. [`write_run`] writes the results of a run; see its documentation.
//!
//! `ceteris run --history [--history-every N]` also writes the per-tick series of
//! [`crate::history`] next to the run files: `run_history_world` (one row per sampled tick),
//! `run_history_provinces` (per tick and province) and `run_history_markets` (per tick,
//! province and good), with the conventions of `run_prices` and `run_wages`. They are not part
//! of [`write_run`], so the default output is unchanged.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    ArrayRef, BooleanArray, Float32Array, Float64Array, Int8Array, RecordBatch, StringArray,
    UInt8Array, UInt32Array,
};
use arrow_ipc::writer::FileWriter;
use arrow_schema::{Field, Schema};
use ceteris_trade::goods::{NAME, TRADED, tariff_group};
use ceteris_trade::source::WEIGHT_FLOOR;
use ceteris_trade::{Kernel, N_GOODS, Rationing, TradeParams};

use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;

use crate::demand::{CATEGORY, pigl};
use crate::flows::WINDOW;
use crate::inputs::EconInputs;
use crate::params::{EconParams, Technology};
use crate::world::{
    EconWorld, MINOR_PER_UNIT, active_cells, base_spend, household_preferences, static_digest,
};

/// Bumped when a table or `params.json` changes shape or meaning; the reference checks it.
/// 2: `active` marks origin markets of traded goods, `params.json` has `rationing`,
/// `run_prices` has `price_index` (ADRs 0015 and 0016). 3: theta per good, in `goods` and as
/// a list in `params.json`, and `goods.theta_class` (ADRs 0018 and 0023). 4: `params.json`
/// has `expectation_gain`, `run_prices` and `run_wages` have `clip_ticks` (ADR 0019). 5: PIGL
/// demand (ADR 0021): `goods.category`, `countries.gini`, the `pigl_*` parameters in
/// `params.json`, the `pigl_check` table. 6: `params.json` has `technology` (S09) and
/// `rationing` may be `mixed` (ADR 0020).
pub const EXPORT_FORMAT: &str = "6";

/// Rows of the `pigl_check` table.
pub const PIGL_CHECK_DRAWS: usize = 100;

#[derive(Debug)]
pub struct ExportError(pub String);

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "export: {}", self.0)
    }
}

impl std::error::Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError(e.to_string())
    }
}

impl From<arrow_schema::ArrowError> for ExportError {
    fn from(e: arrow_schema::ArrowError) -> Self {
        ExportError(e.to_string())
    }
}

pub(crate) type Column = (String, ArrayRef);

fn col(name: impl Into<String>, array: impl arrow_array::Array + 'static) -> Column {
    (name.into(), Arc::new(array))
}

pub(crate) fn u8s(name: &str, v: impl IntoIterator<Item = u8>) -> Column {
    col(name, UInt8Array::from_iter_values(v))
}

pub(crate) fn u32s(name: &str, v: impl IntoIterator<Item = u32>) -> Column {
    col(name, UInt32Array::from_iter_values(v))
}

fn f32s(name: &str, v: impl IntoIterator<Item = f32>) -> Column {
    col(name, Float32Array::from_iter_values(v))
}

pub(crate) fn f64s(name: &str, v: impl IntoIterator<Item = f64>) -> Column {
    col(name, Float64Array::from_iter_values(v))
}

pub(crate) fn bools(name: &str, v: impl IntoIterator<Item = bool>) -> Column {
    col(name, BooleanArray::from_iter(v.into_iter().map(Some)))
}

pub(crate) fn write_table(dir: &Path, name: &str, columns: Vec<Column>) -> Result<(), ExportError> {
    let fields: Vec<Field> = columns
        .iter()
        .map(|(n, a)| Field::new(n, a.data_type().clone(), false))
        .collect();
    let metadata: std::collections::HashMap<String, String> =
        [("export_format".to_owned(), EXPORT_FORMAT.to_owned())].into();
    let schema = Arc::new(Schema::new_with_metadata(fields, metadata));
    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        columns.into_iter().map(|(_, a)| a).collect(),
    )?;
    let file = BufWriter::new(File::create(dir.join(format!("{name}.arrow")))?);
    let mut writer = FileWriter::try_new(file, &schema)?;
    writer.write(&batch)?;
    writer.finish()?;
    writer.into_inner()?.flush()?;
    Ok(())
}

fn hex(hash: &[u8; 32]) -> String {
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// A JSON number; the shortest text that parses back to the same f64.
fn num(x: f64) -> String {
    assert!(x.is_finite(), "non-finite parameter {x}");
    format!("{x:?}")
}

fn nums(xs: &[f64]) -> String {
    let items: Vec<String> = xs.iter().map(|&x| num(x)).collect();
    format!("[{}]", items.join(", "))
}

/// The static economy: tables as in the module documentation, and `params.json` with both
/// parameter sets keyed by field name (register IDs under `register`), the sourcing weight
/// floor, the money minor-unit scale and the digest of everything a snapshot leaves out.
/// `kernel` must be the kernel built from `inputs` and `trade`.
pub fn write_inputs(
    dir: &Path,
    inputs: &EconInputs,
    kernel: &Kernel,
    params: &EconParams,
    trade: &TradeParams,
) -> Result<(), ExportError> {
    fs::create_dir_all(dir)?;
    let (np, nc) = (inputs.n_provinces(), inputs.n_countries());
    let net = &inputs.network;

    write_table(
        dir,
        "goods",
        vec![
            u8s("good", 0..N_GOODS as u8),
            col("name", StringArray::from(NAME.to_vec())),
            bools("traded", TRADED),
            col(
                "tariff_group",
                Int8Array::from_iter_values(
                    (0..N_GOODS).map(|g| if TRADED[g] { tariff_group(g) as i8 } else { -1 }),
                ),
            ),
            f64s("weight", inputs.weight),
            f64s("coverage", trade.coverage),
            f64s("time_cost", trade.time_cost),
            f64s("theta", trade.theta),
            u8s("theta_class", trade.theta_class),
            u8s("category", CATEGORY.map(|c| c as u8)),
        ],
    )?;
    write_table(
        dir,
        "gateways",
        vec![
            u32s("gateway", 0..net.n_gateways() as u32),
            u32s("country", net.gateway_country.iter().map(|&c| u32::from(c))),
        ],
    )?;

    let c = &inputs.countries;
    let mut columns = vec![
        u32s("country", 0..nc as u32),
        f64s("s_l", c.s_l.iter().copied()),
        f64s("unemployment", c.unemployment.iter().copied()),
        f64s("gini", c.gini.iter().copied()),
    ];
    for g in 0..N_GOODS {
        columns.push(f64s(&format!("omega_{g}"), c.omega.iter().map(|o| o[g])));
    }
    write_table(dir, "countries", columns)?;
    let cells = || {
        (0..nc).flat_map(|c| (0..N_GOODS).flat_map(move |k| (0..N_GOODS).map(move |g| (c, k, g))))
    };
    write_table(
        dir,
        "country_io",
        vec![
            u32s("country", cells().map(|(c, _, _)| c as u32)),
            u8s("input", cells().map(|(_, k, _)| k as u8)),
            u8s("sector", cells().map(|(_, _, g)| g as u8)),
            f64s("a", cells().map(|(c, k, g)| inputs.a(c, k, g))),
        ],
    )?;
    let rates = || {
        (0..nc)
            .flat_map(|imp| (0..nc).flat_map(move |exp| (0..2).map(move |group| (imp, exp, group))))
    };
    write_table(
        dir,
        "tariffs",
        vec![
            u32s("importer", rates().map(|(i, _, _)| i as u32)),
            u32s("exporter", rates().map(|(_, e, _)| e as u32)),
            u8s("group", rates().map(|(_, _, g)| g as u8)),
            f32s("rate", rates().map(|(i, e, g)| inputs.wedges.rate(i, e, g))),
        ],
    )?;

    let p = &inputs.provinces;
    write_table(
        dir,
        "provinces",
        vec![
            u32s("province", 0..np as u32),
            u32s("country", p.country.iter().map(|&c| u32::from(c))),
            f64s("population", p.population.iter().copied()),
            f64s("w0", p.w0.iter().copied()),
        ],
    )?;
    if !p.pack.is_empty() {
        write_table(
            dir,
            "pack_provinces",
            vec![
                u32s("province", 0..np as u32),
                u32s("pack", p.pack.iter().copied()),
            ],
        )?;
    }
    let pp = &inputs.pops;
    write_table(
        dir,
        "pops",
        vec![
            u32s("pop", 0..inputs.n_pops() as u32),
            u32s("province", pp.province.iter().copied()),
            u8s("edu", pp.edu.iter().copied()),
            f64s("labour_force", pp.labour_force.iter().copied()),
            f64s("population", pp.population.iter().copied()),
        ],
    )?;
    let e = &inputs.enterprises;
    write_table(
        dir,
        "enterprises",
        vec![
            u32s("enterprise", 0..inputs.n_enterprises() as u32),
            u32s("province", e.province.iter().copied()),
            u8s("good", e.good.iter().copied()),
            f64s("a", e.a.iter().copied()),
            f64s("va0", e.va0.iter().copied()),
            f64s("l0", e.l0.iter().copied()),
            f64s("va_target", e.va_target.iter().copied()),
        ],
    )?;
    write_table(dir, "active", active_columns(&active_cells(inputs)))?;
    write_table(dir, "pigl_check", pigl_check(inputs, params))?;

    let legs: Vec<(u8, &ceteris_trade::Leg)> = [&net.domestic, &net.access, &net.crossings]
        .into_iter()
        .enumerate()
        .flat_map(|(kind, legs)| legs.iter().map(move |l| (kind as u8, l)))
        .collect();
    write_table(
        dir,
        "legs",
        vec![
            u8s("kind", legs.iter().map(|l| l.0)),
            u32s("from", legs.iter().map(|l| l.1.from)),
            u32s("to", legs.iter().map(|l| l.1.to)),
            f64s("cost_per_t", legs.iter().map(|l| l.1.cost_per_t)),
            f64s("hours", legs.iter().map(|l| l.1.hours)),
        ],
    )?;

    write_kernel(dir, kernel)?;
    fs::write(
        dir.join("params.json"),
        params_json(static_digest(inputs, params, trade), params, trade),
    )?;
    Ok(())
}

fn active_columns(active: &[bool]) -> Vec<Column> {
    vec![
        u32s("province", (0..active.len()).map(|c| (c / N_GOODS) as u32)),
        u8s("good", (0..active.len()).map(|c| (c % N_GOODS) as u8)),
        bools("active", active.iter().copied()),
    ]
}

fn write_kernel(dir: &Path, kernel: &Kernel) -> Result<(), ExportError> {
    let traded = || (0..N_GOODS).filter(|&g| TRADED[g]);

    // (good, row, entry col, k, tau) for the two plain legs
    let plain = |pick: fn(
        &ceteris_trade::kernel::GoodKernel,
    ) -> &ceteris_trade::kernel::Csr<ceteris_trade::kernel::Entry>| {
        let mut rows = Vec::new();
        for g in traded() {
            let csr = pick(kernel.good(g));
            for r in 0..csr.n_rows() {
                rows.extend(
                    csr.row(r)
                        .iter()
                        .map(|e| (g as u8, r as u32, e.col, e.k, e.tau)),
                );
            }
        }
        rows
    };
    let columns = |names: [&str; 2], rows: &[(u8, u32, u32, f32, f64)]| {
        vec![
            u8s("good", rows.iter().map(|r| r.0)),
            u32s(names[0], rows.iter().map(|r| r.1)),
            u32s(names[1], rows.iter().map(|r| r.2)),
            f32s("k", rows.iter().map(|r| r.3)),
            f64s("tau", rows.iter().map(|r| r.4)),
        ]
    };
    write_table(
        dir,
        "kernel_domestic",
        columns(["o", "d"], &plain(|gk| &gk.domestic)),
    )?;
    write_table(
        dir,
        "kernel_access",
        columns(["province", "gateway"], &plain(|gk| &gk.access)),
    )?;

    let mut rows = Vec::new();
    for g in traded() {
        let csr = &kernel.good(g).crossings;
        for a in 0..csr.n_rows() {
            rows.extend(csr.row(a).iter().map(|e| (g as u8, a as u32, *e)));
        }
    }
    write_table(
        dir,
        "kernel_crossing",
        vec![
            u8s("good", rows.iter().map(|r| r.0)),
            u32s("a", rows.iter().map(|r| r.1)),
            u32s("b", rows.iter().map(|r| r.2.col)),
            f32s("k", rows.iter().map(|r| r.2.k)),
            f64s("tau", rows.iter().map(|r| r.2.tau)),
            f64s("border_tau", rows.iter().map(|r| r.2.border_tau)),
            u8s("lag", rows.iter().map(|r| r.2.lag)),
            f32s("tariff", rows.iter().map(|r| r.2.tariff)),
        ],
    )
}

/// [`PIGL_CHECK_DRAWS`] draws of `demand::pigl`: a country, prices e^U(-2, 2) with every
/// tenth draw's good `draw % 7` unreachable, money per person the country's base spending
/// times e^U(-7, 7) (far enough to clamp), sigma U(0, 1.2).
fn pigl_check(inputs: &EconInputs, params: &EconParams) -> Vec<Column> {
    let (weights, _) = household_preferences(inputs, params);
    let base = base_spend(inputs, params);
    let nc = inputs.n_countries();
    let mut rows = Vec::with_capacity(PIGL_CHECK_DRAWS);
    for r in 0..PIGL_CHECK_DRAWS {
        let mut s = Seed(21).stream(0, SystemId(0), Kind(0), EntityId(r as u32));
        let c = (s.next_u32() as usize) % nc;
        let mut price: [f64; N_GOODS] = std::array::from_fn(|_| libm::exp(4.0 * s.uniform() - 2.0));
        if r % 10 == 0 {
            price[r % 7] = f64::INFINITY;
        }
        let money = base[c] * libm::exp(14.0 * s.uniform() - 7.0);
        let sigma = 1.2 * s.uniform();
        let mut x = [0.0; N_GOODS];
        pigl(&weights[c], &price, money, sigma, params, &mut x);
        rows.push((c, money, sigma, price, x));
    }
    let mut columns = vec![
        u32s("draw", 0..PIGL_CHECK_DRAWS as u32),
        u32s("country", rows.iter().map(|r| r.0 as u32)),
        f64s("money", rows.iter().map(|r| r.1)),
        f64s("sigma", rows.iter().map(|r| r.2)),
    ];
    for g in 0..N_GOODS {
        columns.push(f64s(&format!("price_{g}"), rows.iter().map(|r| r.3[g])));
    }
    for g in 0..N_GOODS {
        columns.push(f64s(&format!("x_{g}"), rows.iter().map(|r| r.4[g])));
    }
    columns
}

fn params_json(digest: [u8; 32], p: &EconParams, t: &TradeParams) -> String {
    let register = [
        ("theta", "P01"),
        ("theta_class", "P01"),
        ("coverage", "P02"),
        ("kappa1", "P03a"),
        ("kappa2", "P03b"),
        ("c_max", "P03c"),
        ("border_nu", "P05"),
        ("sigma_demand", "P06"),
        ("pigl_sigma_b", "P46"),
        ("pigl_phi", "P46"),
        ("pigl_gamma", "P46"),
        ("pigl_theta", "P46"),
        ("pigl_eps", "P47"),
        ("sigma_fl", "P08"),
        ("kappa_w", "P39"),
        ("delta_w", "P40"),
        ("eta", "P41"),
        ("edu_eff", "P42"),
        ("time_cost", "P43"),
        ("expectation_gain", "P45"),
        ("rationing", "S11"),
        ("technology", "S09"),
    ];
    let register: Vec<String> = register
        .iter()
        .map(|(name, id)| format!("    \"{name}\": \"{id}\""))
        .collect();
    let econ = [
        ("sigma_demand", num(p.sigma_demand)),
        ("pigl_sigma_b", num(p.pigl.sigma_b)),
        ("pigl_phi", num(p.pigl.phi)),
        ("pigl_gamma", num(p.pigl.gamma)),
        ("pigl_theta", nums(&p.pigl.theta)),
        ("pigl_eps", num(p.pigl.eps)),
        ("sigma_fl", num(p.sigma_fl)),
        ("kappa_w", num(p.kappa_w)),
        ("delta_w", num(p.delta_w)),
        ("eta", num(p.eta)),
        ("edu_eff", nums(&p.edu_eff)),
        ("frictionless", p.frictionless.to_string()),
        (
            "rationing",
            match p.rationing {
                Rationing::Priority => "\"priority\"",
                Rationing::Proportional => "\"proportional\"",
                Rationing::Mixed => "\"mixed\"",
            }
            .to_string(),
        ),
        (
            "technology",
            match p.technology {
                Technology::PuttyClay => "\"putty_clay\"",
                Technology::FixedLeontief => "\"fixed_leontief\"",
            }
            .to_string(),
        ),
    ];
    let trade = [
        ("theta", nums(&t.theta)),
        ("theta_class", format!("{:?}", t.theta_class)),
        ("coverage", nums(&t.coverage)),
        ("kappa1", num(t.kappa1)),
        ("kappa2", num(t.kappa2)),
        ("c_max", num(t.c_max)),
        ("expectation_gain", num(t.expectation_gain)),
        ("border_nu", num(t.border_nu)),
        ("time_cost", nums(&t.time_cost)),
    ];
    let object = |items: &[(&str, String)]| {
        let lines: Vec<String> = items
            .iter()
            .map(|(k, v)| format!("    \"{k}\": {v}"))
            .collect();
        format!("{{\n{}\n  }}", lines.join(",\n"))
    };
    format!(
        "{{\n  \"format\": \"{EXPORT_FORMAT}\",\n  \"static_digest\": \"{}\",\n  \"n_goods\": {N_GOODS},\n  \"minor_per_unit\": {},\n  \"weight_floor\": {},\n  \"register\": {{\n{}\n  }},\n  \"econ\": {},\n  \"trade\": {}\n}}\n",
        hex(&digest),
        num(MINOR_PER_UNIT),
        num(WEIGHT_FLOOR),
        register.join(",\n"),
        object(&econ),
        object(&trade),
    )
}

/// The results of a run, at the world's current tick:
///
/// | file | columns |
/// |---|---|
/// | `run_prices` | province u32, good u8, price f64, active bool, price_index f64 (the delivered unit price q_d of traded goods, the local price of local goods, at the last E3), clip_ticks u32 (ticks the price step clip bound) |
/// | `run_wages` | province u32, wage f64, clip_ticks u32 (ticks the wage step clip bound) |
/// | `run_enterprises` | enterprise u32, province u32, good u8, employment f64, output f64 |
/// | `run_flows` | exporter_country u32, importer_country u32, good u8, value f64, units f64 |
/// | `run_home` | province u32, good u8, value f64, units f64 |
/// | `run.json` | `static_digest` of the inputs and parameters, `ticks_averaged`, `frictionless` |
///
/// Flows are the means over the last 12 ticks (fewer when the run is shorter, see
/// `ticks_averaged`) of traded goods only, one row per (exporter, importer, good) with a
/// positive flow, sorted. Value is the origin value of the shipped units, what the exporter
/// receives (trade SPEC, Payments). Units are origin units at dispatch, before losses.
/// Domestic pairs (exporter = importer) are the shipments between provinces of one country;
/// `run_home` has the home purchases (for every traded cell, valued at the cell's price).
pub fn write_run(dir: &Path, world: &EconWorld) -> Result<(), ExportError> {
    fs::create_dir_all(dir)?;
    let inputs = world.inputs();
    let (np, nc) = (inputs.n_provinces(), inputs.n_countries());
    let m = &world.markets;

    write_table(
        dir,
        "run_prices",
        vec![
            u32s("province", (0..m.price.len()).map(|c| (c / N_GOODS) as u32)),
            u8s("good", (0..m.price.len()).map(|c| (c % N_GOODS) as u8)),
            f64s("price", m.price.iter().copied()),
            bools("active", m.active.iter().copied()),
            f64s("price_index", world.price_index.iter().copied()),
            u32s("clip_ticks", m.clip_ticks.iter().copied()),
        ],
    )?;
    write_table(
        dir,
        "run_wages",
        vec![
            u32s("province", 0..np as u32),
            f64s("wage", world.wage.iter().copied()),
            u32s("clip_ticks", world.wage_clip_ticks.iter().copied()),
        ],
    )?;
    let e = &inputs.enterprises;
    write_table(
        dir,
        "run_enterprises",
        vec![
            u32s("enterprise", 0..inputs.n_enterprises() as u32),
            u32s("province", e.province.iter().copied()),
            u8s("good", e.good.iter().copied()),
            f64s("employment", world.employment.iter().copied()),
            f64s("output", world.output.iter().copied()),
        ],
    )?;

    let window = &world.scratch.flows;
    let mean = window.mean();
    let mut keys = Vec::new();
    for ex in 0..nc {
        for im in 0..nc {
            for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
                let at = (ex * nc + im) * N_GOODS + g;
                if mean.pair_value[at] > 0.0 || mean.pair_units[at] > 0.0 {
                    keys.push((ex as u32, im as u32, g as u8, at));
                }
            }
        }
    }
    write_table(
        dir,
        "run_flows",
        vec![
            u32s("exporter_country", keys.iter().map(|k| k.0)),
            u32s("importer_country", keys.iter().map(|k| k.1)),
            u8s("good", keys.iter().map(|k| k.2)),
            f64s("value", keys.iter().map(|k| mean.pair_value[k.3])),
            f64s("units", keys.iter().map(|k| mean.pair_units[k.3])),
        ],
    )?;
    let cells: Vec<usize> = (0..np * N_GOODS).filter(|&c| TRADED[c % N_GOODS]).collect();
    write_table(
        dir,
        "run_home",
        vec![
            u32s("province", cells.iter().map(|&c| (c / N_GOODS) as u32)),
            u8s("good", cells.iter().map(|&c| (c % N_GOODS) as u8)),
            f64s("value", cells.iter().map(|&c| mean.home_value[c])),
            f64s("units", cells.iter().map(|&c| mean.home_units[c])),
        ],
    )?;

    fs::write(
        dir.join("run.json"),
        format!(
            "{{\n  \"format\": \"{EXPORT_FORMAT}\",\n  \"static_digest\": \"{}\",\n  \"ticks_averaged\": {},\n  \"window\": {WINDOW},\n  \"frictionless\": {}\n}}\n",
            hex(&world.digest()),
            window.ticks(),
            world.params().frictionless,
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;

    use arrow_array::RecordBatch;
    use arrow_array::cast::AsArray;
    use arrow_array::types::{Float32Type, Int8Type};
    use ceteris_geo::pack::{f64s, read_table, strings, u8s, u32s};
    use ceteris_trade::N_GOODS;
    use ceteris_trade::goods::{NAME, TRADED};

    use crate::fixture::small_world;
    use crate::world::MINOR_PER_UNIT;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ceteris-export-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn f32s(b: &RecordBatch, name: &str) -> Vec<f32> {
        b.column_by_name(name)
            .unwrap()
            .as_primitive::<Float32Type>()
            .values()
            .to_vec()
    }

    fn bools(b: &RecordBatch, name: &str) -> Vec<bool> {
        b.column_by_name(name)
            .unwrap()
            .as_boolean()
            .iter()
            .map(|v| v.unwrap())
            .collect()
    }

    fn i8s(b: &RecordBatch, name: &str) -> Vec<i8> {
        b.column_by_name(name)
            .unwrap()
            .as_primitive::<Int8Type>()
            .values()
            .to_vec()
    }

    fn setup() -> (EconInputs, Kernel, EconParams, TradeParams) {
        let inputs = small_world();
        let (params, trade) = (EconParams::default(), TradeParams::default());
        let kernel = Kernel::build(&inputs.network, &inputs.weight, &inputs.wedges, &trade);
        (inputs, kernel, params, trade)
    }

    #[test]
    fn export_round_trips_fixture() {
        let (inputs, kernel, params, trade) = setup();
        let dir = scratch_dir("round-trip");
        write_inputs(&dir, &inputs, &kernel, &params, &trade).unwrap();
        let table = |name: &str| read_table(&dir.join(format!("{name}.arrow"))).unwrap();

        let goods = table("goods");
        assert_eq!(goods.num_rows(), N_GOODS);
        assert_eq!(strings(&goods, "name").unwrap()[4], NAME[4]);
        assert_eq!(bools(&goods, "traded"), TRADED);
        assert_eq!(
            i8s(&goods, "tariff_group"),
            [0, 0, 0, 0, 1, 1, 1, -1, -1, -1]
        );
        assert_eq!(f64s(&goods, "weight").unwrap(), inputs.weight);
        assert_eq!(f64s(&goods, "coverage").unwrap(), trade.coverage);
        assert_eq!(f64s(&goods, "time_cost").unwrap(), trade.time_cost);
        assert_eq!(f64s(&goods, "theta").unwrap(), trade.theta);
        assert_eq!(u8s(&goods, "theta_class").unwrap(), trade.theta_class);
        assert_eq!(u8s(&goods, "category").unwrap(), CATEGORY.map(|c| c as u8));

        let gateways = table("gateways");
        assert_eq!(u32s(&gateways, "country").unwrap().len(), 7);

        let countries = table("countries");
        assert_eq!(countries.num_rows(), 4);
        assert_eq!(f64s(&countries, "s_l").unwrap(), inputs.countries.s_l);
        assert_eq!(
            f64s(&countries, "unemployment").unwrap(),
            inputs.countries.unemployment
        );
        assert_eq!(f64s(&countries, "gini").unwrap(), inputs.countries.gini);
        for g in 0..N_GOODS {
            let col = f64s(&countries, &format!("omega_{g}")).unwrap();
            assert_eq!(col[2], inputs.countries.omega[2][g]);
        }
        let io = table("country_io");
        assert_eq!(io.num_rows(), 4 * N_GOODS * N_GOODS);
        let (c, k, g, a) = (
            u32s(&io, "country").unwrap(),
            u8s(&io, "input").unwrap(),
            u8s(&io, "sector").unwrap(),
            f64s(&io, "a").unwrap(),
        );
        for r in 0..io.num_rows() {
            assert_eq!(
                a[r],
                inputs.a(c[r] as usize, usize::from(k[r]), usize::from(g[r]))
            );
        }

        let tariffs = table("tariffs");
        assert_eq!(tariffs.num_rows(), inputs.wedges.tariff.len());
        let (imp, exp, grp, rate) = (
            u32s(&tariffs, "importer").unwrap(),
            u32s(&tariffs, "exporter").unwrap(),
            u8s(&tariffs, "group").unwrap(),
            f32s(&tariffs, "rate"),
        );
        for r in 0..tariffs.num_rows() {
            assert_eq!(
                rate[r],
                inputs
                    .wedges
                    .rate(imp[r] as usize, exp[r] as usize, usize::from(grp[r]))
            );
        }

        let provinces = table("provinces");
        assert_eq!(provinces.num_rows(), 25);
        assert_eq!(f64s(&provinces, "w0").unwrap(), inputs.provinces.w0);
        assert_eq!(
            f64s(&provinces, "population").unwrap(),
            inputs.provinces.population
        );
        let pops = table("pops");
        assert_eq!(pops.num_rows(), inputs.n_pops());
        assert_eq!(
            f64s(&pops, "labour_force").unwrap(),
            inputs.pops.labour_force
        );
        assert_eq!(u8s(&pops, "edu").unwrap(), inputs.pops.edu);
        let ents = table("enterprises");
        assert_eq!(ents.num_rows(), inputs.n_enterprises());
        for (col, want) in [
            ("a", &inputs.enterprises.a),
            ("va0", &inputs.enterprises.va0),
            ("l0", &inputs.enterprises.l0),
            ("va_target", &inputs.enterprises.va_target),
        ] {
            assert_eq!(&f64s(&ents, col).unwrap(), want, "{col}");
        }
        assert_eq!(u8s(&ents, "good").unwrap(), inputs.enterprises.good);

        let legs = table("legs");
        let n_legs = inputs.network.domestic.len()
            + inputs.network.access.len()
            + inputs.network.crossings.len();
        assert_eq!(legs.num_rows(), n_legs);

        let traded: Vec<usize> = (0..N_GOODS).filter(|&g| TRADED[g]).collect();
        let domestic = table("kernel_domestic");
        let n: usize = traded
            .iter()
            .map(|&g| kernel.good(g).domestic.entries.len())
            .sum();
        assert_eq!(domestic.num_rows(), n);
        let first = &kernel.good(0).domestic.entries[3];
        let (o, d) = (u32s(&domestic, "o").unwrap(), u32s(&domestic, "d").unwrap());
        assert_eq!(d[3], first.col);
        assert_eq!(o[3], 0);
        assert_eq!(f32s(&domestic, "k")[3], first.k);
        assert_eq!(f64s(&domestic, "tau").unwrap()[3], first.tau);

        let access = table("kernel_access");
        let n: usize = traded
            .iter()
            .map(|&g| kernel.good(g).access.entries.len())
            .sum();
        assert_eq!(access.num_rows(), n);

        let crossing = table("kernel_crossing");
        let n: usize = traded
            .iter()
            .map(|&g| kernel.good(g).crossings.entries.len())
            .sum();
        assert_eq!(crossing.num_rows(), n);
        let last = kernel.good(6).crossings.entries.last().unwrap();
        let at = crossing.num_rows() - 1;
        assert_eq!(u8s(&crossing, "good").unwrap()[at], 6);
        assert_eq!(u32s(&crossing, "b").unwrap()[at], last.col);
        assert_eq!(f32s(&crossing, "k")[at], last.k);
        assert_eq!(f64s(&crossing, "border_tau").unwrap()[at], last.border_tau);
        assert_eq!(f32s(&crossing, "tariff")[at], last.tariff);
        assert_eq!(u8s(&crossing, "lag").unwrap()[at], last.lag);

        let active = table("active");
        assert_eq!(active.num_rows(), 25 * N_GOODS);

        // the PIGL check draws reproduce through the world's demand
        let check = table("pigl_check");
        assert_eq!(check.num_rows(), PIGL_CHECK_DRAWS);
        let w = EconWorld::new(Arc::new(inputs.clone()), params.clone(), trade.clone());
        let (country, money, sigma) = (
            u32s(&check, "country").unwrap(),
            f64s(&check, "money").unwrap(),
            f64s(&check, "sigma").unwrap(),
        );
        let price: Vec<Vec<f64>> = (0..N_GOODS)
            .map(|g| f64s(&check, &format!("price_{g}")).unwrap())
            .collect();
        let got: Vec<Vec<f64>> = (0..N_GOODS)
            .map(|g| f64s(&check, &format!("x_{g}")).unwrap())
            .collect();
        assert!(price.iter().flatten().any(|p| p.is_infinite()));
        let spread = money.iter().fold(0.0, |a: f64, &m| a.max(m))
            / money.iter().fold(f64::INFINITY, |a: f64, &m| a.min(m));
        assert!(spread > 1e4, "money spread {spread}");
        let mut x = [0.0; N_GOODS];
        for r in [0, 37, PIGL_CHECK_DRAWS - 1] {
            let row: [f64; N_GOODS] = std::array::from_fn(|g| price[g][r]);
            let c = country[r] as usize;
            pigl(
                &w.layout().pigl[c],
                &row,
                money[r],
                sigma[r],
                &params,
                &mut x,
            );
            for g in 0..N_GOODS {
                assert_eq!(got[g][r], x[g], "draw {r} good {g}");
            }
        }

        let json = fs::read_to_string(dir.join("params.json")).unwrap();
        assert!(
            json.contains(
                "\"theta\": [12.55, 12.55, 12.55, 2.62, 5.88, 12.55, 5.88, 0.0, 0.0, 0.0]"
            ),
            "{json}"
        );
        assert!(
            json.contains("\"theta_class\": [2, 2, 2, 0, 1, 2, 1, 0, 0, 0]"),
            "{json}"
        );
        assert!(json.contains("\"sigma_demand\": 0.6"), "{json}");
        assert!(
            json.contains("\"pigl_theta\": [0.634, 0.082, 0.284]"),
            "{json}"
        );
        assert!(json.contains("\"pigl_eps\": 0.71"), "{json}");
        assert!(json.contains("\"rationing\": \"priority\""), "{json}");
        assert!(json.contains("\"technology\": \"putty_clay\""), "{json}");
        assert!(json.contains("\"weight_floor\": 1e-100"), "{json}");
        assert!(
            json.contains(&format!("\"minor_per_unit\": {MINOR_PER_UNIT:?}")),
            "{json}"
        );
        for id in [
            "P01", "P02", "P03a", "P03b", "P03c", "P05", "P43", "P06", "P08", "P39", "P40", "P41",
            "P42", "P45", "P46", "P47",
        ] {
            assert!(
                json.contains(&format!("\"{id}\"")),
                "register {id} missing: {json}"
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    fn files(dir: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().into_string().unwrap(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn exports_are_byte_identical() {
        let (inputs, kernel, params, trade) = setup();
        let (a, b) = (scratch_dir("bytes-a"), scratch_dir("bytes-b"));
        write_inputs(&a, &inputs, &kernel, &params, &trade).unwrap();
        write_inputs(&b, &inputs, &kernel, &params, &trade).unwrap();
        let (fa, fb) = (files(&a), files(&b));
        assert!(fa.len() >= 14);
        assert_eq!(fa, fb);

        let run = |dir: &Path| {
            let mut sim = EconWorld::sim(Arc::new(small_world()), params.clone(), trade.clone());
            sim.run_until(30).unwrap();
            write_run(dir, sim.world()).unwrap();
        };
        let (ra, rb) = (scratch_dir("run-a"), scratch_dir("run-b"));
        run(&ra);
        run(&rb);
        assert_eq!(files(&ra), files(&rb));
        for d in [a, b, ra, rb] {
            fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn run_export_carries_the_world() {
        let (inputs, _, params, trade) = setup();
        let mut sim = EconWorld::sim(Arc::new(inputs), params, trade);
        sim.run_until(30).unwrap();
        let w = sim.world();
        let dir = scratch_dir("run");
        write_run(&dir, w).unwrap();
        let table = |name: &str| read_table(&dir.join(format!("{name}.arrow"))).unwrap();

        let prices = table("run_prices");
        assert_eq!(prices.num_rows(), 25 * N_GOODS);
        assert_eq!(f64s(&prices, "price").unwrap(), w.markets.price);
        assert_eq!(bools(&prices, "active"), w.markets.active);
        assert_eq!(f64s(&prices, "price_index").unwrap(), w.price_index);
        assert_eq!(u32s(&prices, "clip_ticks").unwrap(), w.markets.clip_ticks);
        let wages = table("run_wages");
        assert_eq!(f64s(&wages, "wage").unwrap(), w.wage);
        assert_eq!(u32s(&wages, "clip_ticks").unwrap(), w.wage_clip_ticks);
        let ents = table("run_enterprises");
        assert_eq!(f64s(&ents, "employment").unwrap(), w.employment);
        assert_eq!(f64s(&ents, "output").unwrap(), w.output);

        let flows = table("run_flows");
        let (ex, im, g) = (
            u32s(&flows, "exporter_country").unwrap(),
            u32s(&flows, "importer_country").unwrap(),
            u8s(&flows, "good").unwrap(),
        );
        let (value, units) = (
            f64s(&flows, "value").unwrap(),
            f64s(&flows, "units").unwrap(),
        );
        assert!(flows.num_rows() > 0);
        let key = |r: usize| (ex[r], im[r], g[r]);
        for r in 0..flows.num_rows() {
            assert!(ex[r] < 4 && im[r] < 4 && TRADED[usize::from(g[r])]);
            assert!(value[r] > 0.0 && units[r] > 0.0 && value[r].is_finite());
            assert!(r == 0 || key(r - 1) < key(r), "rows sorted and unique");
        }
        assert!(
            (0..flows.num_rows()).any(|r| ex[r] == im[r]),
            "domestic pairs present"
        );
        assert!(
            (0..flows.num_rows()).any(|r| ex[r] != im[r]),
            "international pairs present"
        );
        // the island has no land border: its only partners are the mainland ports
        assert!((0..flows.num_rows()).any(|r| ex[r] == 3 || im[r] == 3));

        let home = table("run_home");
        assert_eq!(home.num_rows(), 25 * 7);
        let meta = fs::read_to_string(dir.join("run.json")).unwrap();
        assert!(meta.contains("\"ticks_averaged\": 12"), "{meta}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
