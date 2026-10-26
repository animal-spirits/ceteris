//! ADR 0089: the calibrated data state is the model's steady state. From the calibration
//! tables (`python -m ceteris.reference calibrate`), 120 ticks with every friction on move no
//! price, wage or flow by more than the money resolution allows: on the fixture
//! (`tests/golden/p2-fixture/calibration`) and, when the pack holds its calibration, on the
//! full 2000 pack. Run with `--release`; the pack run takes about 20 s.
//!
//! Money is integer minor units (1e-4 numeraire), so a province's prices and flows can hold
//! only to about one minor unit over its monthly income. The tolerance of an entry is
//! max(1e-9, C minor / income) with C = 100, income the calibrated monthly household income of
//! the province the entry belongs to (owner decision, 2026-10-25); world quantities hold to
//! 1e-9. The ratio drift / (minor / income) must stay below C / 2 everywhere, and the drift
//! must stay bounded: its largest value over ticks 61 to 120 at most twice that over ticks 5
//! to 60 (ticks 1 to 4, where prices and wages settle onto the rounding lattice, left out).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::world::MINOR_PER_UNIT;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_trade::{N_GOODS, TradeParams};

const TICKS: u32 = 120;
/// The floor of every tolerance, and the tolerance of world quantities.
const FLOOR: f64 = 1e-9;
/// Minor units of income per unit of relative drift a province may show.
const C: f64 = 100.0;
/// Ticks left out of the first boundedness window.
const SETTLE: u32 = 4;
const SPLIT: u32 = 60;
const MINOR: f64 = 1.0 / MINOR_PER_UNIT;

fn fixture_dir() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

/// The quantities the fixed point holds, by name, each entry with its province (`None` for a
/// world quantity): prices of active cells, buyer prices that are finite, wages, and the flows
/// (employment, output and sales per enterprise; Q, D, sales and merchant stock per active
/// cell; each pop's income); and the world price index.
fn quantities(w: &EconWorld) -> Vec<(&'static str, Vec<f64>, Vec<Option<usize>>)> {
    let (inp, m) = (w.inputs(), &w.markets);
    let cells: Vec<usize> = (0..m.price.len()).filter(|&c| m.active[c]).collect();
    let active = |x: &[f64]| -> Vec<f64> { cells.iter().map(|&c| x[c]).collect() };
    let cell_provinces: Vec<Option<usize>> = cells.iter().map(|&c| Some(c / N_GOODS)).collect();
    let all_cells: Vec<Option<usize>> = (0..m.price.len()).map(|c| Some(c / N_GOODS)).collect();
    let ent: Vec<Option<usize>> = inp
        .enterprises
        .province
        .iter()
        .map(|&i| Some(i as usize))
        .collect();
    let pops: Vec<Option<usize>> = inp
        .pops
        .province
        .iter()
        .map(|&i| Some(i as usize))
        .collect();
    let provinces: Vec<Option<usize>> = (0..inp.n_provinces()).map(Some).collect();
    let finite = w
        .price_index
        .iter()
        .map(|&p| if p.is_finite() { p } else { 0.0 })
        .collect();
    vec![
        ("price", active(&m.price), cell_provinces.clone()),
        ("buyer price", finite, all_cells),
        ("wage", w.wage.clone(), provinces),
        ("employment", w.employment.clone(), ent.clone()),
        ("output", w.output.clone(), ent.clone()),
        ("enterprise sales", w.sales.clone(), ent),
        ("supply Q", active(&m.supply), cell_provinces.clone()),
        ("demand D", active(&m.demand), cell_provinces.clone()),
        ("market sales", active(&m.sales), cell_provinces.clone()),
        ("merchant stock", active(&m.inventory), cell_provinces),
        ("pop income", w.income.clone(), pops),
        ("world price index", vec![w.world_price_index()], vec![None]),
    ]
}

/// Relative drift of every entry of `now` from `base`; entries at 0 are measured against the
/// largest entry.
fn drifts(base: &[f64], now: &[f64]) -> Vec<f64> {
    let scale = base.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
    base.iter()
        .zip(now)
        .map(|(&b, &n)| {
            let d = (n - b).abs();
            if b != 0.0 { d / b.abs() } else { d / scale }
        })
        .collect()
}

/// One quantity's record over the run.
#[derive(Debug, Default)]
struct Record {
    name: &'static str,
    /// Largest relative drift, its tick and entry.
    drift: f64,
    tick: u32,
    entry: usize,
    /// Largest drift / (minor / income), its tick and entry (province quantities only).
    ratio: f64,
    ratio_tick: u32,
    ratio_entry: usize,
    /// The largest ratio where the resolution sets the tolerance (C minor / income above the
    /// floor), its tick and entry.
    resolved: f64,
    resolved_entry: usize,
    /// Largest drift / tolerance, and its tick and entry.
    over: f64,
    over_tick: u32,
    over_entry: usize,
    /// Largest scaled drift (drift / tolerance) over ticks SETTLE + 1 to SPLIT and SPLIT + 1
    /// to TICKS, and over ticks 1 to SPLIT.
    early: f64,
    late: f64,
    first_window: f64,
}

/// Runs `TICKS` ticks from the steady state of `inputs` and returns the record of every
/// quantity; prints them, the world price index path and the time per tick.
fn run(inputs: EconInputs, what: &str) -> Vec<Record> {
    assert!(inputs.steady.is_some(), "{what}: inputs are not calibrated");
    let world = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    let inp = world.inputs().clone();
    let steady = inp.steady.clone().unwrap();
    let mut income = vec![0.0; inp.n_provinces()];
    for k in 0..inp.n_pops() {
        income[inp.pops.province[k] as usize] += steady.income[k];
    }
    // the resolution of a province: one minor unit over its monthly income
    let resolution: Vec<f64> = income.iter().map(|&x| MINOR / x).collect();
    let mut base = quantities(&world);
    // the calibrated state's last tick, which the first tick must reproduce
    for (name, values, _) in &mut base {
        match *name {
            "output" => values.clone_from(&steady.output),
            "enterprise sales" => values.clone_from(&steady.ent_sales),
            "pop income" => values.clone_from(&steady.income),
            _ => {}
        }
    }
    let mut records: Vec<Record> = base
        .iter()
        .map(|(name, _, _)| Record {
            name,
            ..Record::default()
        })
        .collect();
    let mut sim = Sim::new(world, schedule(), SEED);
    let mut index = Vec::new();
    let started = std::time::Instant::now();
    for tick in 1..=TICKS {
        sim.step().unwrap_or_else(|v| panic!("{what}: {v}"));
        let w = sim.world();
        index.push(w.world_price_index());
        for (r, ((_, b, provinces), (_, now, _))) in
            records.iter_mut().zip(base.iter().zip(quantities(w)))
        {
            for (entry, (d, province)) in drifts(b, &now).into_iter().zip(provinces).enumerate() {
                let tolerance = province.map_or(FLOOR, |i| FLOOR.max(C * resolution[i]));
                if d > r.drift {
                    (r.drift, r.tick, r.entry) = (d, tick, entry);
                }
                if let Some(i) = province {
                    let ratio = d / resolution[*i];
                    if ratio > r.ratio {
                        (r.ratio, r.ratio_tick, r.ratio_entry) = (ratio, tick, entry);
                    }
                    if tolerance > FLOOR && ratio > r.resolved {
                        (r.resolved, r.resolved_entry) = (ratio, entry);
                    }
                }
                let scaled = d / tolerance;
                if scaled > r.over {
                    (r.over, r.over_tick, r.over_entry) = (scaled, tick, entry);
                }
                if tick <= SPLIT {
                    r.first_window = r.first_window.max(scaled);
                    if tick > SETTLE {
                        r.early = r.early.max(scaled);
                    }
                } else {
                    r.late = r.late.max(scaled);
                }
            }
        }
    }
    let per_tick = started.elapsed() / TICKS;
    println!("{what}: {TICKS} ticks from the calibrated state, {per_tick:?} per tick");
    for r in &records {
        println!(
            "  {:>17}: max drift {:.3e} (tick {}, entry {}); max drift / (minor / income) {:.3e} \
             (tick {}, entry {}), where the resolution sets the tolerance {:.3e} (entry {}); \
             max drift / tolerance {:.3e} (tick {}, entry {}); windows (drift / tolerance): \
             ticks 1-{SPLIT} {:.3e}, {}-{SPLIT} {:.3e}, {}-{TICKS} {:.3e}",
            r.name,
            r.drift,
            r.tick,
            r.entry,
            r.ratio,
            r.ratio_tick,
            r.ratio_entry,
            r.resolved,
            r.resolved_entry,
            r.over,
            r.over_tick,
            r.over_entry,
            r.first_window,
            SETTLE + 1,
            r.early,
            SPLIT + 1,
            r.late
        );
    }
    let (lo, hi) = index
        .iter()
        .fold((f64::INFINITY, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
    println!(
        "  world price index: tick 1 {:.15}, tick {TICKS} {:.15}, range [{lo:.15}, {hi:.15}]",
        index[0],
        index[TICKS as usize - 1]
    );
    records
}

fn assert_fixed(records: &[Record], what: &str) {
    for r in records {
        let name = r.name;
        assert!(
            r.over <= 1.0,
            "{what}: {name} drifts to {:.3} times its tolerance (largest drift {:.3e} at tick {})",
            r.over,
            r.drift,
            r.tick
        );
        assert!(
            r.ratio <= C / 2.0,
            "{what}: {name} drift / (minor / income) {:.3e} at tick {} (entry {}) passes C / 2",
            r.ratio,
            r.ratio_tick,
            r.ratio_entry
        );
        assert!(
            r.late <= 2.0 * r.early,
            "{what}: {name} is not bounded: ticks {}-{TICKS} reach {:.3e} of the tolerance, \
             ticks {}-{SPLIT} {:.3e}",
            SPLIT + 1,
            r.late,
            SETTLE + 1,
            r.early
        );
    }
}

#[test]
fn fixture_calibrated_state_is_a_fixed_point() {
    let mut inputs = small_world();
    inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap();
    let records = run(inputs, "fixture");
    assert_fixed(&records, "fixture");
}

/// Calibration tables of other inputs do not load: the static digest guards them.
#[test]
fn calibration_of_other_inputs_does_not_load() {
    let mut inputs = small_world();
    inputs.pops.labour_force[0] *= 1.01;
    let err = inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap_err();
    assert!(err.0.contains("other inputs"), "{err}");
}

#[test]
fn pack_calibrated_state_is_a_fixed_point() {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if !EconInputs::has_calibration(&dir) {
        eprintln!(
            "skipping: no calibration tables in {}; install them from `python -m \
             ceteris.reference calibrate --pack {}`",
            dir.display(),
            dir.display()
        );
        return;
    }
    let pack =
        ceteris_geo::pack::Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    let inputs = EconInputs::from_pack(&pack, &dir).unwrap();
    let records = run(inputs, "full pack");
    assert_fixed(&records, "full pack");
}
