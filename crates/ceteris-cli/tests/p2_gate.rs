//! P2 gate on the full pack (`specs/econ/SPEC.md`, P2 gate): V1 after the uniqueness check.
//!
//! Needs `CETERIS_PACK_EXPORT`, a `ceteris export --pack` directory of the calibrated pack in
//! `CETERIS_PACK` (default `data/packs/2000`), and the pack's calibration tables; without them
//! every test skips. Each run is 1,200 ticks of the release binary on the full pack: run with
//! `--release`.
//!
//! The reference of V1 is the GE reference model at the calibrated point (ADR 0089): its inputs
//! and root come from the calibration tables (`write::read_reference`), and the test checks that
//! the point solves the model. V1 runs from two starts, so that a run that never left the steady
//! state cannot pass trivially: (a) the steady state with every price, wage and stock moved by
//! its own draw in [-10%, 10%] (`EconWorld::perturb`, seed [`PERTURB_SEED`]) and (b) the 2000
//! data state of the calibrated inputs (prices 1, base wages, one month of base use in stock,
//! nothing in flight), the start before ADR 0089.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use arrow_array::RecordBatch;
use arrow_array::cast::AsArray;
use arrow_array::types::Float64Type;
use arrow_cast::cast;
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use ceteris_calibrate::write;
use ceteris_reference::io::{self, Inputs, pairwise_sum};
use ceteris_reference::model::{self, Model, max_abs};
use ceteris_reference::solve::{self, Equilibrium, Info};
use ceteris_reference::uniqueness::{self, V1Mode};

const TICKS: u32 = 1200;
const HISTORY_EVERY: u32 = 12;
const PERTURB_SEED: u64 = 20;
/// V1's 1% (Equilibrium nesting).
const V1_TOL: f64 = 0.01;
/// Flows compared: above this share of world trade.
const FLOW_FLOOR: f64 = 1e-6;
/// The reference solver's tolerance on scaled residuals.
const REFERENCE_TOL: f64 = 1e-10;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pack() -> PathBuf {
    let dir = std::env::var_os("CETERIS_PACK")
        .map_or_else(|| root().join("data/packs/2000"), PathBuf::from);
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// The reference inputs at the calibrated point and the equilibrium there.
struct Reference {
    inputs: Inputs,
    eq: Equilibrium,
}

fn reference() -> Option<&'static Reference> {
    static REFERENCE: OnceLock<Option<Reference>> = OnceLock::new();
    REFERENCE
        .get_or_init(|| {
            let Some(export) = std::env::var_os("CETERIS_PACK_EXPORT").map(PathBuf::from) else {
                eprintln!(
                    "skipping: CETERIS_PACK_EXPORT unset (a `ceteris export --pack` directory of \
                     the calibrated pack)"
                );
                return None;
            };
            let pack = pack();
            if !pack.join("econ_calib.arrow").exists() {
                eprintln!("skipping: no calibration tables in {}", pack.display());
                return None;
            }
            let inp = io::load(&export).unwrap();
            let pack_province = write::read_pack_provinces(&export).unwrap();
            let (inputs, x) = write::read_reference(&pack, &inp, &pack_province).unwrap();
            let m = Model::new(&inputs).unwrap();
            let residual = max_abs(&model::residual(&x, &m));
            assert!(
                residual <= REFERENCE_TOL,
                "the calibrated point does not solve the reference: {residual:e}"
            );
            let info = Info {
                method: "calibrated point".into(),
                iterations: 0,
                residual,
                dropped_province: m.dropped_province(),
                dropped_residual: 0.0,
                seconds: 0.0,
            };
            let eq = solve::unpack(&x, &m, info);
            println!(
                "reference: residual {residual:.3e} at the calibrated point, price index {}",
                eq.price_index
            );
            Some(Reference { inputs, eq })
        })
        .as_ref()
}

fn run(out: &Path, flags: &[&str]) -> String {
    let pack = pack();
    let (ticks, every) = (TICKS.to_string(), HISTORY_EVERY.to_string());
    let mut args = vec!["run", "--pack", pack.to_str().unwrap(), "--ticks", &ticks];
    args.extend(flags);
    args.extend([
        "--history",
        "--history-every",
        &every,
        "--out",
        out.to_str().unwrap(),
    ]);
    let run = Command::new(env!("CARGO_BIN_EXE_ceteris"))
        .args(&args)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8(run.stdout).unwrap()
}

fn table(path: &Path) -> RecordBatch {
    let file = File::open(path).unwrap();
    let batches: Vec<RecordBatch> = FileReader::try_new(file, None)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    arrow_select::concat::concat_batches(&batches[0].schema(), &batches).unwrap()
}

/// A numeric column as f64.
fn column(t: &RecordBatch, name: &str) -> Vec<f64> {
    let a = cast(t.column_by_name(name).unwrap(), &DataType::Float64).unwrap();
    a.as_primitive::<Float64Type>().values().to_vec()
}

/// numpy's `argmax`: the first largest, or the first NaN.
fn worst(dev: &[f64], label: impl Fn(usize) -> String) -> (f64, String) {
    let mut best = 0;
    for (i, &d) in dev.iter().enumerate() {
        if d.is_nan() {
            best = i;
            break;
        }
        if d > dev[best] {
            best = i;
        }
    }
    (dev[best], label(best))
}

struct Gaps {
    index: f64,
    price: (f64, String),
    wage: (f64, String),
    flow_value: (f64, String),
    flow_units: (f64, String),
    flows_compared: usize,
    flows_missing: usize,
    path: Vec<(u64, f64)>,
    negative_money_pops: f64,
}

/// V1's largest gaps at the final tick, prices and wages over the world price index, flows of
/// the last 12 ticks with flow values over the index; and the path of the largest price gap per
/// history row.
fn gaps(out: &Path, r: &Reference) -> Gaps {
    let (inp, eq) = (&r.inputs, &r.eq);
    let world = table(&out.join("run_history_world.arrow"));
    let levels = column(&world, "world_price_index");
    let index = *levels.last().unwrap();
    let n_g = inp.n_goods();
    let price = column(&table(&out.join("run_prices.arrow")), "price");
    let cells = inp.active_cells();
    let dev: Vec<f64> = cells
        .iter()
        .map(|&c| (price[c] / index / eq.price.data[c] - 1.0).abs())
        .collect();
    let price_gap = worst(&dev, |i| {
        format!("province {} good {}", cells[i] / n_g, cells[i] % n_g)
    });
    let wage = column(&table(&out.join("run_wages.arrow")), "wage");
    let dev: Vec<f64> = wage
        .iter()
        .zip(&eq.wage)
        .map(|(w, want)| (w / index / want - 1.0).abs())
        .collect();
    let wage_gap = worst(&dev, |i| format!("province {i}"));

    let n_c = inp.n_countries();
    let flows = table(&out.join("run_flows.arrow"));
    let (mut value, mut units) = (vec![0.0; n_c * n_c * n_g], vec![0.0; n_c * n_c * n_g]);
    let (ex, im, g) = (
        column(&flows, "exporter_country"),
        column(&flows, "importer_country"),
        column(&flows, "good"),
    );
    let (v, u) = (column(&flows, "value"), column(&flows, "units"));
    for i in 0..v.len() {
        let at = (ex[i] as usize * n_c + im[i] as usize) * n_g + g[i] as usize;
        value[at] = v[i];
        units[at] = u[i];
    }
    let (want_v, want_u) = (
        &eq.flows_by_country_pair_good,
        &eq.units_by_country_pair_good,
    );
    let floor = FLOW_FLOOR * pairwise_sum(want_v);
    let big: Vec<usize> = (0..want_v.len()).filter(|&i| want_v[i] > floor).collect();
    let label = |i: usize| {
        let at = big[i];
        format!(
            "exporter {} importer {} good {}",
            at / (n_c * n_g),
            at / n_g % n_c,
            at % n_g
        )
    };
    let dev: Vec<f64> = big
        .iter()
        .map(|&i| (value[i] / index / want_v[i] - 1.0).abs())
        .collect();
    let flow_value = worst(&dev, label);
    let dev: Vec<f64> = big
        .iter()
        .map(|&i| (units[i] / want_u[i] - 1.0).abs())
        .collect();
    let flow_units = worst(&dev, label);

    let markets = table(&out.join("run_history_markets.arrow"));
    let (tick, market_price) = (column(&markets, "tick"), column(&markets, "price"));
    let path = column(&world, "tick")
        .iter()
        .zip(&levels)
        .map(|(&t, &level)| {
            let p: Vec<f64> = (0..tick.len())
                .filter(|&i| tick[i] == t)
                .map(|i| market_price[i])
                .collect();
            let dev: Vec<f64> = cells
                .iter()
                .map(|&c| (p[c] / level / eq.price.data[c] - 1.0).abs())
                .collect();
            (t as u64, max_abs(&dev))
        })
        .collect();
    Gaps {
        index,
        price: price_gap,
        wage: wage_gap,
        flow_value,
        flow_units,
        flows_compared: big.len(),
        flows_missing: big.iter().filter(|&&i| value[i] == 0.0).count(),
        path,
        negative_money_pops: column(&world, "pops_negative_money")
            .into_iter()
            .fold(0.0, f64::max),
    }
}

/// V1 (SPEC, P2 gate) from `start`: the uniqueness bound at the pack's thetas and value-added
/// shares picks the mode; the frictionless run of 1,200 ticks is then compared with the
/// reference: in tolerance mode every price and wage (index-normalized) and every flow above
/// 1e-6 of world trade within 1%, in report mode the gaps are printed instead. Whether V1 holds,
/// and the summary.
fn v1(r: &Reference, start: &str, flags: &[&str]) -> (bool, String) {
    let bound = uniqueness::spectral_bound(
        &r.inputs.theta,
        0.0,
        None,
        &uniqueness::value_added_share(&r.inputs),
    )
    .unwrap();
    let mode = uniqueness::v1_mode(bound);
    let tmp = tempfile::tempdir().unwrap();
    let stdout = run(tmp.path(), flags);
    let g = gaps(tmp.path(), r);
    let step = (120 / HISTORY_EVERY) as usize;
    let path: Vec<String> = g
        .path
        .iter()
        .step_by(step)
        .map(|(t, d)| format!("{t}:{d:.2e}"))
        .collect();
    let perturbation = if stdout.trim().is_empty() {
        "no perturbation"
    } else {
        stdout.trim()
    };
    let mode_name = match mode {
        V1Mode::Tolerance => "tolerance",
        V1Mode::Report => "report",
    };
    let summary = format!(
        "V1 from {start} ({perturbation}): bound {bound:.4}, {mode_name} mode; at tick {TICKS} \
         price {:.3e} ({}), wage {:.3e} ({}), flow units {:.3e} ({}), flow value {:.3e} ({}); \
         {} flows compared, {} missing; world price index {}; most pops with negative money {}; \
         largest price gap every 120 ticks: {}",
        g.price.0,
        g.price.1,
        g.wage.0,
        g.wage.1,
        g.flow_units.0,
        g.flow_units.1,
        g.flow_value.0,
        g.flow_value.1,
        g.flows_compared,
        g.flows_missing,
        g.index,
        g.negative_money_pops,
        path.join(" ")
    );
    println!("{summary}");
    let holds = mode == V1Mode::Report
        || (g.flows_missing == 0
            && [g.price.0, g.wage.0, g.flow_units.0, g.flow_value.0]
                .iter()
                .all(|&d| d < V1_TOL));
    (holds, summary)
}

#[test]
fn v1_full_pack_from_the_perturbed_steady_state() {
    let Some(r) = reference() else { return };
    let seed = PERTURB_SEED.to_string();
    let (holds, summary) = v1(r, "perturbed", &["--frictionless", "--perturb", &seed]);
    assert!(holds, "{summary}");
}

/// V1 from the shocked data state is a stress test, not the gate's V1 (owner, 2026-10-26):
/// after the coal-led shock output stays at full employment while demand collapses, merchant
/// losses become pop debts (Mali, Albania) and the price level returns slowly; see ADRs 0090
/// and 0112 and the econ SPEC open questions. It is expected to fail
/// V1, and this test fails when it passes, so that the question is reopened.
#[test]
fn v1_full_pack_from_the_data_state_stays_open() {
    let Some(r) = reference() else { return };
    let (holds, summary) = v1(r, "data_state", &["--frictionless", "--data-state"]);
    assert!(!holds, "V1 from the data state now holds: {summary}");
}
