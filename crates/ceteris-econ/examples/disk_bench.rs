//! Disk benchmark of the retention tiers (ADR 0212, P3 A3): bytes per tick of each tier on the
//! calibrated pack (`CETERIS_PACK`, default `data/packs/2000`; the fixture without one), one
//! live run to 2150 and an ensemble to 2050, against the budgets of 20 GB and 50 GB.
//!
//!   cargo run --release -p ceteris-econ --example disk_bench -- [members] [scratch dir]
//!
//! The history tier is projected from one dense market table row set: monthly for the window,
//! annual before (the dense table is what retention thins).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_econ::fixture::small_world;
use ceteris_econ::history::History;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_run::retention::{Footprint, Metric, Recorder, Retention, Tier};
use ceteris_run::store::ChunkStore;

const START_YEAR: u32 = 2000;
const GB: f64 = 1e9;

fn inputs() -> EconInputs {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if EconInputs::has_calibration(&dir) {
        let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
        return EconInputs::from_pack(&pack, &dir).expect("pack inputs load");
    }
    eprintln!("notice: no calibrated pack at {}; fixture", dir.display());
    small_world()
}

fn metrics() -> Vec<Metric<EconWorld>> {
    vec![Metric {
        name: "world_price_index",
        sample: EconWorld::world_price_index,
    }]
}

/// Runs to `end_year` and returns the recorder's footprint and the dense market bytes of one
/// tick (for the history projection).
fn run(
    inputs: &Arc<EconInputs>,
    store: &Path,
    tier: Tier,
    end_year: u32,
    member: u64,
) -> (Footprint, usize) {
    let end = (end_year - START_YEAR) * 12;
    let trade = inputs.trade_params();
    let mut sim = EconWorld::sim(inputs.clone(), EconParams::default(), trade);
    if member > 0 {
        sim.world_mut().perturb(member, 0.01);
    }
    let store = ChunkStore::open(store, "bench").expect("store opens");
    let mut rec = Recorder::new(Retention::default(), tier, store, metrics());
    let mut history = History::new();
    loop {
        let t = sim.tick();
        rec.record(t, sim.world()).expect("record");
        if t == 1 {
            history.record(t, sim.world());
        }
        if t == end {
            return (rec.footprint(), history.market_bytes());
        }
        sim.step().expect("tick");
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let members: u64 = args.next().map_or(32, |a| a.parse().expect("members"));
    let scratch = args.next().map_or_else(std::env::temp_dir, PathBuf::from);
    let inputs = Arc::new(inputs());
    let r = Retention::default();

    let live_dir = scratch.join("ceteris-disk-bench-live");
    let (live, tick_market) = run(&inputs, &live_dir, Tier::Live, 2150, 0);
    let live_ticks = f64::from((2150 - START_YEAR) * 12);
    let dense_ticks =
        f64::from(r.window) + (live_ticks - f64::from(r.window)) / f64::from(r.annual);
    let history = tick_market as f64 * dense_ticks;
    let live_total = live.series_bytes as f64 + live.snapshot_bytes as f64 + history;
    println!(
        "live to 2150: series {} B ({:.1} B/tick), snapshots {} ({:.1} MB, {:.0} B/tick), \
         history {:.2} GB ({} B per dense tick); total {:.2} GB of 20",
        live.series_bytes,
        live.series_bytes as f64 / live_ticks,
        live.snapshots,
        live.snapshot_bytes as f64 / 1e6,
        live.snapshot_bytes as f64 / live_ticks,
        history / GB,
        tick_market,
        live_total / GB,
    );

    let ens_dir = scratch.join("ceteris-disk-bench-ensemble");
    let ens_ticks = f64::from((2050 - START_YEAR) * 12);
    let mut series = 0;
    let mut snaps = 0;
    for m in 1..=members {
        let (f, _) = run(&inputs, &ens_dir, Tier::Member, 2050, m);
        series += f.series_bytes;
        snaps = f.snapshot_bytes; // the shared store's running total
    }
    let total = series as f64 + snaps as f64;
    println!(
        "ensemble of {members} to 2050: series {series} B, snapshots {:.2} GB \
         ({:.0} B/tick/member); total {:.2} GB of 50",
        snaps as f64 / GB,
        snaps as f64 / ens_ticks / members as f64,
        total / GB,
    );
    let _ = std::fs::remove_dir_all(live_dir);
    let _ = std::fs::remove_dir_all(ens_dir);
}
