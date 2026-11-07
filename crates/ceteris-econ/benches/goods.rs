//! Per-system cost against the goods count (ADR 0235, task G2): every system of the tick at 10,
//! 20 and 45 synthetic goods (`ceteris_econ::synthetic`), from the data state after one tick,
//! on the calibrated pack (`CETERIS_PACK`) or the fixture. The medians fit a fixed plus a
//! per-good cost per system (spec 19.2); the synthetic worlds are never a calibration input.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_econ::fixture::small_world;
use ceteris_econ::synthetic::with_goods;
use ceteris_econ::systems;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

type System = fn(&mut EconWorld);

const SYSTEMS: [(&str, System); 9] = [
    ("deliver", systems::deliver),
    ("labour", systems::labour),
    ("produce", systems::produce),
    ("consume", systems::demand),
    ("source", systems::source),
    ("clear", systems::clear),
    ("dispatch", systems::dispatch),
    ("reprice", systems::reprice),
    ("distribute", |w| systems::distribute(w, 0)),
];

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
    eprintln!(
        "notice: no calibrated pack at {}; benchmarking the fixture",
        dir.display()
    );
    (small_world(), "fixture")
}

fn step(w: &mut EconWorld, tick: u32) {
    w.begin_tick();
    systems::schedule().run_tick(w, systems::SEED, tick);
    w.end_tick(tick).expect("invariants hold");
}

/// A world at the start of a tick, after `begin_tick` and the first `n` systems.
fn before(base: &EconWorld, n: usize) -> EconWorld {
    let mut w = base.clone();
    w.begin_tick();
    for (_, run) in &SYSTEMS[..n] {
        run(&mut w);
    }
    w
}

fn bench(c: &mut Criterion) {
    let (inputs, what) = inputs();
    let trade = inputs.trade_params();
    let mut g = c.benchmark_group(format!("goods_{what}"));
    g.sample_size(10);
    for n in [10, 20, 45] {
        if n < inputs.n_goods() {
            continue;
        }
        let (synthetic, trade) = with_goods(&inputs, &trade, n);
        let mut base = EconWorld::new(Arc::new(synthetic), EconParams::default(), trade);
        step(&mut base, 0);
        for (i, (name, run)) in SYSTEMS.iter().enumerate() {
            let start = before(&base, i);
            g.bench_function(format!("{name}/{n}"), |b| {
                b.iter_batched(
                    || start.clone(),
                    |mut w| {
                        run(&mut w);
                        w
                    },
                    BatchSize::LargeInput,
                )
            });
        }
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
