//! P2 gate on the full pack (`specs/econ/SPEC.md`, P2 gate): V1 after the uniqueness check, H2 per
//! theta class (ADRs 0018, 0023, 0092 and 0264) and the H2 distance elasticity, a reported
//! diagnostic since ADR 0264.
//!
//! Needs `CETERIS_PACK_EXPORT`, a `ceteris export --pack` directory of the calibrated pack in
//! `CETERIS_PACK` (default `data/packs/2000`), and the pack's calibration tables; H2 also reads
//! the CEPII gravity file of the data cache. Without them every test skips. Each run is 1,200
//! ticks of the release binary on the full pack: run with `--release`.
//!
//! The reference of V1 is the GE reference model at the calibrated point (ADR 0089): its inputs
//! and root come from the calibration tables (`write::read_reference`), and the test checks that
//! the point solves the model. V1 runs from two starts, so that a run that never left the steady
//! state cannot pass trivially: (a) the steady state with every price, wage and stock moved by
//! its own draw in [-10%, 10%] (`EconWorld::perturb`, each of the seeds [`PERTURB_SEEDS`]) and
//! (b) the 2000 data state of the calibrated inputs (prices 1, base wages, one month of base use in stock,
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
use ceteris_calibrate::{border, write};
use ceteris_econ::{EconInputs, EconParams};
use ceteris_pack::fetch;
use ceteris_pack::read::natural_earth::read_codes;
use ceteris_pack::read::tariffs::{GRAVITY, read_distances};
use ceteris_reference::io::{self, Inputs, Mat, pairwise_sum};
use ceteris_reference::model::{self, Model, max_abs};
use ceteris_reference::ppml::{self, GravityTable, RunFlows};
use ceteris_reference::solve::{self, Equilibrium, Info};
use ceteris_reference::uniqueness::{self, Gains, V1Mode};

const TICKS: u32 = 1200;
const HISTORY_EVERY: u32 = 12;
/// The perturbation seeds V1 runs from: 20, and 21 and 22 so that one draw cannot pass alone.
const PERTURB_SEEDS: [u64; 3] = [20, 21, 22];
/// V1's 1% (Equilibrium nesting).
const V1_TOL: f64 = 0.01;
/// Flows compared: above this share of world trade.
const FLOW_FLOOR: f64 = 1e-6;
/// The reference solver's tolerance on scaled residuals.
const REFERENCE_TOL: f64 = 1e-10;
/// The data's band for the distance elasticity, reported, not gated (ADR 0264).
const H2_DISTANCE: (f64, f64) = (-1.1, -0.7);
/// Theta classes; each must recover its own theta_c within max(2 pair-clustered standard errors,
/// [`H2_THETA_TOL`] of theta_c) of its PPML estimate with the model's country-pair kernel as the
/// cost control (ADRs 0092, 0264).
const N_CLASSES: usize = 4;
/// With the model's own kernel as the control the regression is nearly an identity and the
/// clustered s.e. no longer measures sampling error: V1's 1% (ADR 0264).
const H2_THETA_TOL: f64 = V1_TOL;
const N_TRADED: usize = 7;

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

/// The export of the calibrated pack, when it and the calibration tables are there.
fn export() -> Option<&'static Inputs> {
    static EXPORT: OnceLock<Option<Inputs>> = OnceLock::new();
    EXPORT
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
            Some(io::load(&export).unwrap())
        })
        .as_ref()
}

fn reference() -> Option<&'static Reference> {
    static REFERENCE: OnceLock<Option<Reference>> = OnceLock::new();
    REFERENCE
        .get_or_init(|| {
            let inp = export()?;
            let export = std::env::var_os("CETERIS_PACK_EXPORT")
                .map(PathBuf::from)
                .unwrap();
            let pack = pack();
            let pack_province = write::read_pack_provinces(&export).unwrap();
            let (inputs, x) = write::read_reference(&pack, inp, &pack_province).unwrap();
            let m = Model::new(&inputs).unwrap();
            // the calibration pins the traded goods' index to 1, local goods at p_c (ADR 0098);
            // the reference pins the world index over every good: the same point, in its units
            let shift = model::evaluate(&x, &m, &m.inp.ent_a).index.ln();
            let x: Vec<f64> = x.iter().map(|v| v - shift).collect();
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
    // the gate measures the steady state, which the initial drift leaves (ADR 0103)
    let mut args = vec!["run", "--pack", pack.to_str().unwrap(), "--ticks", &ticks];
    args.push("--no-drift");
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
    excluded: CellGaps,
    wage: (f64, String),
    flow_value: (f64, String),
    flow_units: (f64, String),
    flows_compared: usize,
    flows_missing: usize,
    path: Vec<(u64, f64)>,
    negative_money_pops: f64,
}

/// The frictionless run's one-market loops (`uniqueness::Gains`): each cell's price gain and J
/// at eta 1, from the calibration's market elasticities (ADRs 0263, 0267).
fn frictionless_gains(inputs: &EconInputs) -> Gains {
    let m = inputs
        .market_elasticity
        .as_ref()
        .expect("a calibration with market elasticities");
    let (params, trade) = (EconParams::frictionless(), inputs.trade_params());
    let n_g = ceteris_trade::goods::p2::N;
    let at =
        |f: &dyn Fn(usize) -> f64| Mat::from_fn(inputs.n_provinces(), n_g, |p, g| f(p * n_g + g));
    Gains {
        kappa: at(&|i| m.price_gain(i, &params, &trade)),
        j: at(&|i| m.j(i, params.eta, &trade)),
    }
}

/// The calibrated pack's frictionless gains.
fn pack_gains() -> &'static Gains {
    static GAINS: OnceLock<Gains> = OnceLock::new();
    GAINS.get_or_init(|| {
        let pack = pack();
        let graph =
            ceteris_geo::pack::Pack::load(&pack, &ceteris_geo::graph::TransportParams::default())
                .unwrap();
        frictionless_gains(&EconInputs::from_pack(&graph, &pack).unwrap())
    })
}

/// The price gap of each listed cell (province, good).
type CellGaps = Vec<((usize, usize), f64)>;

/// The largest price gap over the active cells `cells` (`p * G + g`) not in `excluded`, with its
/// cell, and the gap of every excluded cell: `price` over `index` against `want`.
fn price_gaps(
    price: &[f64],
    index: f64,
    want: &[f64],
    cells: &[usize],
    n_g: usize,
    excluded: &[(usize, usize)],
) -> ((f64, String), CellGaps) {
    let gap = |c: usize| (price[c] / index / want[c] - 1.0).abs();
    let out = |c: usize| excluded.contains(&(c / n_g, c % n_g));
    let kept: Vec<usize> = cells.iter().copied().filter(|&c| !out(c)).collect();
    let dev: Vec<f64> = kept.iter().map(|&c| gap(c)).collect();
    let worst_gap = worst(&dev, |i| {
        format!("province {} good {}", kept[i] / n_g, kept[i] % n_g)
    });
    let gaps_out = excluded
        .iter()
        .map(|&(p, g)| ((p, g), gap(p * n_g + g)))
        .collect();
    (worst_gap, gaps_out)
}

/// V1's largest gaps at the final tick, prices and wages over the world price index, flows of
/// the last 12 ticks with flow values over the index; and the path of the largest price gap per
/// history row. Prices skip the cells in `excluded`, outside one-market J2 at the run's gains,
/// whose gaps are returned instead.
fn gaps(out: &Path, r: &Reference, excluded: &[(usize, usize)]) -> Gaps {
    let (inp, eq) = (&r.inputs, &r.eq);
    let world = table(&out.join("run_history_world.arrow"));
    let levels = column(&world, "world_price_index");
    let index = *levels.last().unwrap();
    let n_g = inp.n_goods();
    let price = column(&table(&out.join("run_prices.arrow")), "price");
    let cells = inp.active_cells();
    let (price_gap, excluded_gaps) =
        price_gaps(&price, index, &eq.price.data, &cells, n_g, excluded);
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
                .filter(|&&c| !excluded.contains(&(c / n_g, c % n_g)))
                .map(|&c| (p[c] / level / eq.price.data[c] - 1.0).abs())
                .collect();
            (t as u64, max_abs(&dev))
        })
        .collect();
    Gaps {
        index,
        price: price_gap,
        excluded: excluded_gaps,
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
/// 1e-6 of world trade within 1%, in report mode the gaps are printed instead. Prices leave out
/// the cells outside one-market J2 at the run's gains (`uniqueness::cells_outside_region`),
/// printed with their gaps; more than 1% of the active cells excluded fails V1. Whether V1
/// holds, and the summary.
fn v1(r: &Reference, start: &str, flags: &[&str]) -> (bool, String) {
    let bound = uniqueness::spectral_bound(
        &r.inputs.theta,
        0.0,
        None,
        &uniqueness::value_added_share(&r.inputs),
    )
    .unwrap();
    let mode = uniqueness::v1_mode(bound);
    let excluded = uniqueness::cells_outside_region(&r.inputs, pack_gains());
    let active = r.inputs.active_cells().len();
    let tmp = tempfile::tempdir().unwrap();
    let stdout = run(tmp.path(), flags);
    let g = gaps(tmp.path(), r, &excluded);
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
         cells outside J2 excluded {} of {active} ({:?}); largest price gap every 120 ticks: {}",
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
        g.excluded.len(),
        g.excluded,
        path.join(" ")
    );
    println!("{summary}");
    let holds = (excluded.len() as f64) <= 0.01 * active as f64
        && (mode == V1Mode::Report
            || (g.flows_missing == 0
                && [g.price.0, g.wage.0, g.flow_units.0, g.flow_value.0]
                    .iter()
                    .all(|&d| d < V1_TOL)));
    (holds, summary)
}

/// V1 leaves out a cell outside one-market J2 at the run's gains: on the fixture's frictionless
/// gains (none outside) with one cell's gain pushed past 2 / J, that cell alone is excluded, its
/// gap reported, and the largest gap is the other cells'. Runs without a pack.
#[test]
fn v1_excludes_cells_outside_region() {
    let fixture = root().join("tests/golden/p2-fixture");
    let inp = io::load(&fixture).unwrap();
    let mut world = ceteris_econ::fixture::small_world();
    world.calibrate(&fixture.join("calibration")).unwrap();
    let mut gains = frictionless_gains(&world);
    assert_eq!(uniqueness::cells_outside_region(&inp, &gains), vec![]);
    let cells = inp.active_cells();
    let n_g = inp.n_goods();
    let c = cells[cells.len() / 2];
    let pushed = (c / n_g, c % n_g);
    gains.kappa[pushed] = 2.5 / gains.j[pushed];
    let excluded = uniqueness::cells_outside_region(&inp, &gains);
    assert_eq!(excluded, vec![pushed]);
    let want = vec![1.0; inp.n_provinces() * n_g];
    let mut price = want.clone();
    price[c] = 1.5;
    price[cells[0]] = 1.002;
    let (all, _) = price_gaps(&price, 1.0, &want, &cells, n_g, &[]);
    assert!((all.0 - 0.5).abs() < 1e-12, "{all:?}");
    let (kept, out) = price_gaps(&price, 1.0, &want, &cells, n_g, &excluded);
    assert!((kept.0 - 0.002).abs() < 1e-12, "{kept:?}");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, pushed);
    assert!((out[0].1 - 0.5).abs() < 1e-12);
}

#[test]
fn v1_full_pack_from_the_perturbed_steady_state() {
    let Some(r) = reference() else { return };
    let failed: Vec<String> = PERTURB_SEEDS
        .iter()
        .filter_map(|seed| {
            let seed = seed.to_string();
            let (holds, summary) = v1(r, "perturbed", &["--frictionless", "--perturb", &seed]);
            (!holds).then_some(summary)
        })
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
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

/// The default run of 1,200 ticks from the calibrated steady state.
fn default_run() -> Option<&'static Path> {
    static RUN: OnceLock<Option<tempfile::TempDir>> = OnceLock::new();
    RUN.get_or_init(|| {
        export()?;
        let tmp = tempfile::tempdir().unwrap();
        run(tmp.path(), &[]);
        Some(tmp)
    })
    .as_ref()
    .map(tempfile::TempDir::path)
}

/// CEPII gravity of 2000 (`distw` the population-weighted harmonic distance `distw_harmonic`,
/// `contig`) and the CEPII code of each pack country (its ISO code; pairs of countries CEPII
/// does not list are dropped and counted).
struct Gravity {
    rows: Vec<ceteris_pack::read::tariffs::Distance>,
    iso3: Vec<String>,
}

fn gravity() -> Option<&'static Gravity> {
    static GRAVITY_TABLE: OnceLock<Option<Gravity>> = OnceLock::new();
    GRAVITY_TABLE
        .get_or_init(|| {
            export()?;
            let path = fetch::path_of(GRAVITY);
            if !path.exists() {
                eprintln!(
                    "skipping: no {GRAVITY} in {}; fetch it with `ceteris pack fetch`",
                    path.parent().unwrap().display()
                );
                return None;
            }
            let rows = read_distances(&path, 2000).unwrap();
            let (iso, _) = read_codes(&fetch::path_of("ne_10m_admin_0_countries.zip")).unwrap();
            let codes = table(&pack().join("countries.arrow"));
            let codes = codes
                .column_by_name("code")
                .unwrap()
                .as_string::<i32>()
                .iter()
                .map(|c| {
                    let c = c.unwrap();
                    iso.get(c).cloned().unwrap_or_else(|| c.to_owned())
                })
                .collect();
            Some(Gravity { rows, iso3: codes })
        })
        .as_ref()
}

/// H2 distance (ADR 0264): PPML of simulated country-pair trade (traded goods, FOB value, the
/// last 12 ticks of the default run) on ln distance and contiguity with exporter and importer
/// effects, domestic pairs excluded. A reported diagnostic: the test prints the coefficient
/// against the data's band of -1.1 to -0.7 and fails only on a non-finite or non-negative one.
/// The model's cost-distance slope is about 0.09 per class with iceberg leg costs, so each
/// class's distance coefficient is about -0.09 theta_c and the pooled one near -0.46; the data
/// give about -0.8 per class whatever theta. The structural fix, a log-distance component in leg
/// costs, is scheduled by ADR 0264.
#[test]
fn h2_distance_elasticity() {
    let (Some(out), Some(g)) = (default_run(), gravity()) else {
        return;
    };
    let f = table(&out.join("run_flows.arrow"));
    let good = column(&f, "good");
    let traded: Vec<usize> = (0..good.len())
        .filter(|&i| good[i] < N_TRADED as f64)
        .collect();
    let pick = |name: &str| -> Vec<f64> {
        let v = column(&f, name);
        traded.iter().map(|&i| v[i]).collect()
    };
    let index = |v: Vec<f64>| -> Vec<usize> { v.into_iter().map(|x| x as usize).collect() };
    let (ex, im, value) = (
        index(pick("exporter_country")),
        index(pick("importer_country")),
        pick("value"),
    );
    let o: Vec<&str> = g.rows.iter().map(|r| r.iso3_o.as_str()).collect();
    let d: Vec<&str> = g.rows.iter().map(|r| r.iso3_d.as_str()).collect();
    let distw: Vec<f64> = g.rows.iter().map(|r| r.distw).collect();
    let contig: Vec<f64> = g.rows.iter().map(|r| r.contig).collect();
    let iso3: Vec<&str> = g.iso3.iter().map(String::as_str).collect();
    let e = ppml::distance_elasticity(
        &RunFlows {
            exporter_country: &ex,
            importer_country: &im,
            value: &value,
        },
        &GravityTable {
            iso3_o: &o,
            iso3_d: &d,
            distw: &distw,
            contig: &contig,
        },
        &iso3,
    )
    .unwrap();
    println!("H2 distance: {e:?}");
    let (lo, hi) = H2_DISTANCE;
    println!(
        "H2 distance (diagnostic, ADR 0264): {:.4} (s.e. {:.4}) against the data's {lo} to {hi}: \
         {}; iceberg leg costs give a cost-distance slope near 0.09 per class, a log-distance \
         leg-cost component is the scheduled fix",
        e.elasticity,
        e.se_elasticity,
        if lo <= e.elasticity && e.elasticity <= hi {
            "inside"
        } else {
            "outside"
        }
    );
    assert!(
        e.elasticity.is_finite() && e.elasticity < 0.0,
        "distance elasticity {} (s.e. {}): not finite and negative",
        e.elasticity,
        e.se_elasticity
    );
}

/// H2 per theta class (ADRs 0018, 0092 and 0264): per class, PPML of the simulated country-pair
/// flows of its goods on ln(1 + tariff) with the model's own country-pair trade-cost kernel net of
/// tariffs as the cost control (`model::ln_country_kernel` at the calibrated point, weighted by
/// the sourcing weights w_o and E_d / Phi_d), exporter and importer effects per good, foreign
/// pairs a crossing links, zero flows kept; theta is minus the tariff coefficient, and the class's
/// theta_c (the export's) must lie within max(2 pair-clustered standard errors, 1% of theta_c)
/// of it: with the model's own kernel as the control the regression is nearly an identity, so
/// the s.e. stops measuring sampling error and the tolerance is V1's 1% (ADR 0264). The flow is
/// what the importer pays, FOB value times (1 + t): the kernel carries (1 + t)^-theta on spending
/// (trade SPEC, Sourcing), so that is the flow whose tariff elasticity is theta (the FOB value's
/// is theta + 1).
///
/// Printed as a diagnostic, the earlier gate: ln distance (CEPII population-weighted) as the cost
/// control, which leaves the costs the model puts on near partners in the tariff coefficient
/// (ADR 0264). The class with one good is also fitted there by `ppml::trade_elasticity`.
#[test]
fn h2_theta_by_class() {
    let (Some(inp), Some(r), Some(out), Some(g)) =
        (export(), reference(), default_run(), gravity())
    else {
        return;
    };
    let n_c = inp.n_countries();
    let n_g = inp.n_goods();
    let f = table(&out.join("run_flows.arrow"));
    let mut value = vec![0.0; n_c * n_c * n_g];
    let (fe, fi, fg, fv) = (
        column(&f, "exporter_country"),
        column(&f, "importer_country"),
        column(&f, "good"),
        column(&f, "value"),
    );
    for i in 0..fv.len() {
        value[(fe[i] as usize * n_c + fi[i] as usize) * n_g + fg[i] as usize] = fv[i];
    }
    let t = table(&pack().join("econ_tariffs.arrow"));
    let mut rate = vec![0.0; n_c * n_c * 2];
    let (ti, te, tg, tr) = (
        column(&t, "importer"),
        column(&t, "exporter"),
        column(&t, "group"),
        column(&t, "rate"),
    );
    for i in 0..tr.len() {
        rate[(ti[i] as usize * n_c + te[i] as usize) * 2 + tg[i] as usize] = tr[i];
    }
    let export = std::env::var_os("CETERIS_PACK_EXPORT")
        .map(PathBuf::from)
        .unwrap();
    let params: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(export.join("params.json")).unwrap())
            .unwrap();
    let of: Vec<usize> = params["trade"]["theta_class"].as_array().unwrap()[..N_TRADED]
        .iter()
        .map(|v| v.as_u64().unwrap() as usize)
        .collect();
    // the kernel's theta, P150's scale included
    let theta_good = &r.inputs.theta[..N_TRADED];
    // Tariff groups: primary goods 0 to 3, manufactures 4 to 6.
    let group = [0, 0, 0, 0, 1, 1, 1];
    let ln_k = {
        let m = Model::new(&r.inputs).unwrap();
        let n_p = r.inputs.n_provinces();
        let src = m.sourcing(&r.eq.price);
        let phi = model::cells(&src.phi, n_p, n_g);
        let destination = Mat::from_fn(n_p, n_g, |p, g| {
            if phi[(p, g)] > 0.0 {
                r.eq.expenditure[(p, g)] / phi[(p, g)]
            } else {
                0.0
            }
        });
        model::ln_country_kernel(&r.inputs, &model::cells(&src.w, n_p, n_g), &destination)
    };
    let index: std::collections::BTreeMap<&str, usize> = g
        .iso3
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect();
    let mut dist = vec![f64::NAN; n_c * n_c];
    for row in &g.rows {
        if let (Some(&o), Some(&d)) = (
            index.get(row.iso3_o.as_str()),
            index.get(row.iso3_d.as_str()),
        ) {
            dist[o * n_c + d] = row.distw;
        }
    }
    let foreign: Vec<(usize, usize)> = (0..n_c * n_c)
        .filter(|&i| i / n_c != i % n_c)
        .map(|i| (i / n_c, i % n_c))
        .collect();
    let (mut lines, mut misses) = (vec![], vec![]);
    let n_classes = *of.iter().max().unwrap() + 1;
    assert_eq!(
        n_classes, N_CLASSES,
        "the export has {n_classes} theta classes"
    );
    // each class, gated; and each good of a class whose goods take their own border friction
    // (ADR 0276), reported
    let mut sets: Vec<(String, Vec<usize>, bool)> = (0..N_CLASSES)
        .map(|c| {
            let goods: Vec<usize> = (0..N_TRADED).filter(|&k| of[k] == c).collect();
            (format!("class {c} goods {goods:?}"), goods, true)
        })
        .collect();
    for k in (0..N_TRADED).filter(|&k| of[k] == border::PER_GOOD_CLASS) {
        sets.push((
            format!("class {} good {k} alone (diagnostic)", of[k]),
            vec![k],
            false,
        ));
    }
    for (label, goods, gated) in sets {
        let theta_c = theta_good[goods[0]];
        let rows_of = |keep: &dyn Fn(usize, usize, usize) -> bool| -> Vec<(usize, usize, usize)> {
            foreign
                .iter()
                .flat_map(|&(e, m)| goods.iter().map(move |&k| (e, m, k)))
                .filter(|&(e, m, k)| keep(e, m, k))
                .collect()
        };
        let tariff_of = |rows: &[(usize, usize, usize)]| -> Vec<f64> {
            rows.iter()
                .map(|&(e, m, k)| rate[(m * n_c + e) * 2 + group[k]])
                .collect()
        };
        let fob_of = |rows: &[(usize, usize, usize)]| -> Vec<f64> {
            rows.iter()
                .map(|&(e, m, k)| value[(e * n_c + m) * n_g + k])
                .collect()
        };
        let paid_of = |fob: &[f64], tariff: &[f64]| -> Vec<f64> {
            fob.iter().zip(tariff).map(|(v, t)| v * (1.0 + t)).collect()
        };

        let rows = rows_of(&|e, m, k| ln_k[(e * n_c + m) * n_g + k].is_finite());
        let tariff = tariff_of(&rows);
        let paid = paid_of(&fob_of(&rows), &tariff);
        let control: Vec<f64> = rows
            .iter()
            .map(|&(e, m, k)| ln_k[(e * n_c + m) * n_g + k])
            .collect();
        let fit = ppml::class_tariff_fit(&rows, &paid, &tariff, &[("ln_kernel", &control)], true)
            .unwrap();
        let theta = -fit.coef("ln_tariff");
        let se = fit.se("ln_tariff");
        let tol = (2.0 * se).max(H2_THETA_TOL * theta_c);
        let line = format!(
            "{label}: theta {theta:.7} (pair-clustered s.e. {se:.3e}), deviation \
             from theta_c {:.3e} ({:.3e} relative) against tolerance {tol:.3e} (max of 2 s.e. and \
             {H2_THETA_TOL} theta_c), ln kernel {:.4}, pseudo-R2 {:.4}, n {} ({} separated)",
            theta - theta_c,
            (theta - theta_c) / theta_c,
            fit.coef("ln_kernel"),
            fit.pseudo_r2,
            fit.n,
            fit.n_separated
        );
        println!("H2 per class: {line}");
        lines.push(line);
        if gated && ((theta - theta_c).abs() > tol || theta.is_nan()) {
            misses.push(label.clone());
        }

        let rows = rows_of(&|e, m, _| dist[e * n_c + m].is_finite() && dist[e * n_c + m] > 0.0);
        let tariff = tariff_of(&rows);
        let fob = fob_of(&rows);
        let paid = paid_of(&fob, &tariff);
        let distance: Vec<f64> = rows.iter().map(|&(e, m, _)| dist[e * n_c + m]).collect();
        let ln_dist: Vec<f64> = distance.iter().map(|d| d.ln()).collect();
        let controls = [("ln_dist", ln_dist.as_slice())];
        let fit = ppml::class_tariff_fit(&rows, &paid, &tariff, &controls, false).unwrap();
        let clustered = ppml::class_tariff_fit(&rows, &paid, &tariff, &controls, true).unwrap();
        let fob_fit = ppml::class_tariff_fit(&rows, &fob, &tariff, &controls, false).unwrap();
        let theta = -fit.coef("ln_tariff");
        let se = clustered.se("ln_tariff");
        let mut line = format!(
            "{label} with ln distance as the cost control (diagnostic): theta {theta:.4} (s.e. \
             {:.4}, pair-clustered {se:.4}), ln distance {:.4} (s.e. {:.4}), pseudo-R2 {:.4}, n \
             {} ({} separated); theta_c {theta_c} against {:.4} to {:.4}; FOB value: tariff \
             coefficient {:.4} (s.e. {:.4})",
            fit.se("ln_tariff"),
            fit.coef("ln_dist"),
            fit.se("ln_dist"),
            fit.pseudo_r2,
            fit.n,
            fit.n_separated,
            theta - 2.0 * se,
            theta + 2.0 * se,
            fob_fit.coef("ln_tariff"),
            fob_fit.se("ln_tariff")
        );
        if goods.len() == 1 {
            let (e, m): (Vec<usize>, Vec<usize>) = rows.iter().map(|&(e, m, _)| (e, m)).unzip();
            let dense = ppml::trade_elasticity(&paid, &e, &m, &tariff, &distance).unwrap();
            assert!(
                (dense.theta - theta).abs() <= 1e-6 * theta.abs(),
                "trade_elasticity {} vs hdfe {theta}",
                dense.theta
            );
            line += &format!(
                "; trade_elasticity {:.4} (s.e. {:.4})",
                dense.theta, dense.se_theta
            );
        }
        println!("H2 per class: {line}");
    }
    assert!(
        misses.is_empty(),
        "classes {misses:?}: theta_c outside the tolerance of the kernel-controlled estimate: \
         {}",
        lines.join("; ")
    );
}
