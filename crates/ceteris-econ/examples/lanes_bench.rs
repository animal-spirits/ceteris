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
fn lane_params(inputs: &EconInputs, l: usize) -> (EconParams, TradeParams) {
    let mut p = EconParams::default();
    p.kappa_w *= 1.0 + 0.01 * l as f64;
    (p, inputs.trade_params())
}

/// Resident set now, MiB, from `ps` (macOS and Linux).
fn rss_mib() -> f64 {
    let pid = std::process::id().to_string();
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .expect("ps runs");
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .unwrap_or(f64::NAN)
        / 1024.0
}

fn lanes<const L: usize>(inputs: &Arc<EconInputs>, ticks: u32) -> f64 {
    let before = rss_mib();
    let mut lanes = EconLanes::<L>::build(inputs, std::array::from_fn(|l| lane_params(inputs, l)))
        .without_flows();
    let built = rss_mib();
    let t = Instant::now();
    lanes.run_until(1).expect("invariants hold");
    let mut secs = t.elapsed().as_secs_f64();
    let first = rss_mib();
    let t = Instant::now();
    lanes.run_until(ticks).expect("invariants hold");
    secs += t.elapsed().as_secs_f64();
    eprintln!(
        "rss MiB: before {before:.0}, built {built:.0}, tick 1 {first:.0}, end {:.0}; per lane built {:.0}, after tick 1 {:.0}",
        rss_mib(),
        (built - before) / L as f64,
        (first - built) / L as f64
    );
    let s = lanes.stage_secs();
    let per = |x: f64| 1000.0 * x / (L as f64 * f64::from(ticks));
    eprintln!(
        "stage ms per run-tick: begin {:.2} systems {:.2} source {:.2} clear {:.2} dispatch {:.2} reprice+end {:.2}",
        per(s[0]),
        per(s[1]),
        per(s[2]),
        per(s[3]),
        per(s[4]),
        per(s[5])
    );
    secs
}

fn scalar(inputs: &Arc<EconInputs>, n: usize, ticks: u32) -> f64 {
    let mut total = 0.0;
    for l in 0..n {
        let (p, t) = lane_params(inputs, l);
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
