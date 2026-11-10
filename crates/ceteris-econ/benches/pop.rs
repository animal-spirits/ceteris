//! Tick cost of demography (system 220, P4 Branch A) against 19.2: the system alone on a monthly
//! and on an ageing (yearly) tick, and the whole tick with population off and on, on the
//! calibrated pack (`CETERIS_PACK`) or the fixture. The cohorts are `pop::toy`, one full cell set
//! per province: a cost fixture, never a calibration input.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_econ::fixture::small_world;
use ceteris_econ::pop::toy;
use ceteris_econ::{EconInputs, EconParams, EconWorld, systems};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

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

fn bench(c: &mut Criterion) {
    let (inputs, what) = inputs();
    let inputs = Arc::new(inputs);
    let off = EconWorld::new(inputs.clone(), EconParams::default(), inputs.trade_params());
    let (d, cohorts) = toy(inputs.n_provinces());
    let on = off.clone().with_population(d, cohorts);
    let mut g = c.benchmark_group(format!("pop_{what}"));
    g.sample_size(10);
    // tick 1 is a monthly tick; tick 12 an ageing tick (ageing and education progression)
    for (name, tick) in [("system/month", 1), ("system/year", 12)] {
        g.bench_function(name, |b| {
            b.iter_batched(
                || on.clone(),
                |mut w| {
                    systems::demography(&mut w, tick);
                    w
                },
                BatchSize::LargeInput,
            )
        });
    }
    for (name, world) in [("tick/off", &off), ("tick/on", &on)] {
        let mut base = world.clone();
        step(&mut base, 0);
        g.bench_function(name, |b| {
            b.iter_batched(
                || base.clone(),
                |mut w| {
                    step(&mut w, 1);
                    w
                },
                BatchSize::LargeInput,
            )
        });
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
