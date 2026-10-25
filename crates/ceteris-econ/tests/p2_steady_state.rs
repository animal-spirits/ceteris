//! ADR 0089: the calibrated data state is the model's steady state. From the calibration
//! tables (`python -m ceteris.reference calibrate`), 120 ticks with every friction on move no
//! price, wage or flow by more than 1e-9 relative: on the fixture (`tests/golden/p2-fixture/
//! calibration`) and, when the pack holds its calibration, on the full 2000 pack. Run with
//! `--release`; the pack run takes minutes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_trade::TradeParams;

const TICKS: u32 = 120;
const TOLERANCE: f64 = 1e-9;

fn fixture_dir() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

/// The quantities the fixed point holds, by name: prices of active cells, buyer prices that
/// are finite, wages, and the flows (employment, output and sales per enterprise; Q, D, sales
/// and merchant stock per active cell; each pop's income).
fn quantities(w: &EconWorld) -> Vec<(&'static str, Vec<f64>)> {
    let m = &w.markets;
    let active = |x: &[f64]| -> Vec<f64> {
        x.iter()
            .zip(&m.active)
            .filter(|&(_, &on)| on)
            .map(|(&v, _)| v)
            .collect()
    };
    let finite = w
        .price_index
        .iter()
        .map(|&p| if p.is_finite() { p } else { 0.0 })
        .collect();
    vec![
        ("price", active(&m.price)),
        ("buyer price", finite),
        ("wage", w.wage.clone()),
        ("employment", w.employment.clone()),
        ("output", w.output.clone()),
        ("enterprise sales", w.sales.clone()),
        ("supply Q", active(&m.supply)),
        ("demand D", active(&m.demand)),
        ("market sales", active(&m.sales)),
        ("merchant stock", active(&m.inventory)),
        ("pop income", w.income.clone()),
    ]
}

/// Largest relative drift of `now` from `base`, entries at 0 measured against the largest
/// entry, and the entry where it is.
fn drift(base: &[f64], now: &[f64]) -> (f64, usize) {
    let scale = base.iter().fold(0.0f64, |a, &b| a.max(b.abs()));
    base.iter()
        .zip(now)
        .map(|(&b, &n)| {
            let d = (n - b).abs();
            if b != 0.0 { d / b.abs() } else { d / scale }
        })
        .enumerate()
        .fold((0.0, 0), |a, (i, d)| if d > a.0 { (d, i) } else { a })
}

/// Runs `TICKS` ticks from the steady state of `inputs` and returns, per quantity, the
/// largest drift from the calibrated state, its tick and entry, and the world price index
/// path; prints them and the time per tick.
fn run(inputs: EconInputs, what: &str) -> Vec<(&'static str, f64, u32, usize)> {
    assert!(inputs.steady.is_some(), "{what}: inputs are not calibrated");
    let world = EconWorld::new(
        Arc::new(inputs),
        EconParams::default(),
        TradeParams::default(),
    );
    let steady = world.inputs().steady.clone().unwrap();
    let mut base = quantities(&world);
    // the calibrated state's last tick, which the first tick must reproduce
    for (name, values) in &mut base {
        match *name {
            "output" => values.clone_from(&steady.output),
            "enterprise sales" => values.clone_from(&steady.ent_sales),
            "pop income" => values.clone_from(&steady.income),
            _ => {}
        }
    }
    let mut worst: Vec<(&'static str, f64, u32, usize)> =
        base.iter().map(|(n, _)| (*n, 0.0, 0, 0)).collect();
    let mut sim = Sim::new(world, schedule(), SEED);
    let mut index = Vec::new();
    let started = std::time::Instant::now();
    for tick in 1..=TICKS {
        sim.step().unwrap_or_else(|v| panic!("{what}: {v}"));
        let w = sim.world();
        index.push(w.world_price_index());
        for (slot, ((_, b), (_, now))) in worst.iter_mut().zip(base.iter().zip(quantities(w))) {
            let (d, at) = drift(b, &now);
            if d > slot.1 {
                (slot.1, slot.2, slot.3) = (d, tick, at);
            }
        }
    }
    let per_tick = started.elapsed() / TICKS;
    println!("{what}: {TICKS} ticks from the calibrated state, {per_tick:?} per tick");
    for (name, d, tick, at) in &worst {
        println!("  {name:>16}: max relative drift {d:.3e} (tick {tick}, entry {at})");
    }
    let (lo, hi) = index
        .iter()
        .fold((f64::INFINITY, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
    println!(
        "  world price index: tick 1 {:.15}, tick {TICKS} {:.15}, range [{lo:.15}, {hi:.15}]",
        index[0],
        index[TICKS as usize - 1]
    );
    worst
}

fn assert_fixed(worst: &[(&'static str, f64, u32, usize)], what: &str) {
    for &(name, d, tick, at) in worst {
        assert!(
            d <= TOLERANCE,
            "{what}: {name} drifts by {d:.3e} relative at tick {tick} (entry {at})"
        );
    }
}

#[test]
fn fixture_calibrated_state_is_a_fixed_point() {
    let mut inputs = small_world();
    inputs
        .calibrate(&fixture_dir().join("calibration"))
        .unwrap();
    let worst = run(inputs, "fixture");
    assert_fixed(&worst, "fixture");
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
    let worst = run(inputs, "full pack");
    assert_fixed(&worst, "full pack");
}
