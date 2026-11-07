//! L4 benchmark (specs/p3-lanes/PLAN.md): `L` lanes stepped together against `L` lone runs
//! one after another, on the calibrated pack (`CETERIS_PACK`, default `data/packs/2000`) or the
//! fixture. Usage: `lanes_bench MODE L TICKS` with MODE `lanes` or `scalar`; prints one line
//! of run-ticks per second. Wrap in `/usr/bin/time -l` (macOS) or `-v` (Linux) for peak
//! memory; `scripts/lanes_bench.sh` does both.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use ceteris_econ::fixture::small_world;
use ceteris_econ::lanes::EconLanes;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::TradeParams;

fn inputs() -> (EconInputs, &'static str) {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if EconInputs::has_calibration(&dir) {
        let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
        return (
            EconInputs::from_pack(&pack, &dir).expect("pack inputs load"),
            "pack",
        );
    }
    eprintln!("notice: no calibrated pack at {}; fixture", dir.display());
    (small_world(), "fixture")
}

/// Lane `l` differs from lane 0 in the wage step only, so all lanes share one kernel.
fn lane_params(l: usize) -> (EconParams, TradeParams) {
    let mut p = EconParams::default();
    p.kappa_w *= 1.0 + 0.01 * l as f64;
    (p, TradeParams::default())
}

fn lanes<const L: usize>(inputs: &Arc<EconInputs>, ticks: u32) -> f64 {
    let mut lanes = EconLanes::<L>::build(inputs, std::array::from_fn(lane_params));
    let t = Instant::now();
    lanes.run_until(ticks).expect("invariants hold");
    t.elapsed().as_secs_f64()
}

fn scalar(inputs: &Arc<EconInputs>, n: usize, ticks: u32) -> f64 {
    let mut total = 0.0;
    for l in 0..n {
        let (p, t) = lane_params(l);
        let mut sim = EconWorld::sim(Arc::clone(inputs), p, t);
        let start = Instant::now();
        sim.run_until(ticks).expect("invariants hold");
        total += start.elapsed().as_secs_f64();
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (mode, n, ticks) = (
        args[1].as_str(),
        args[2].parse::<usize>().unwrap(),
        args[3].parse::<u32>().unwrap(),
    );
    let (inp, source) = inputs();
    let inputs = Arc::new(inp);
    let secs = match (mode, n) {
        ("scalar", n) => scalar(&inputs, n, ticks),
        ("lanes", 1) => lanes::<1>(&inputs, ticks),
        ("lanes", 8) => lanes::<8>(&inputs, ticks),
        ("lanes", 16) => lanes::<16>(&inputs, ticks),
        ("lanes", 32) => lanes::<32>(&inputs, ticks),
        _ => panic!("MODE is lanes or scalar; lanes L is 1, 8, 16 or 32"),
    };
    let run_ticks = (n as u32 * ticks) as f64;
    println!(
        "{source} {mode} L={n} ticks={ticks} secs={secs:.3} run_ticks_per_sec={:.2} ms_per_run_tick={:.2}",
        run_ticks / secs,
        1000.0 * secs / run_ticks
    );
}
