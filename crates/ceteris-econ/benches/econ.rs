//! P2 system benchmarks (`specs/econ/SPEC.md`, Gate): every system of the tick on the full
//! pack, on tick 1 from its calibrated steady state (the first tick whose E3 finds prices
//! moved), each measured on the state the systems before it leave. Without a calibrated pack
//! (`CETERIS_PACK`, default `data/packs/2000`) they run on the fixture, so `cargo bench
//! --no-run` and CI work.
//!
//! Budgets per tick (M4 Pro): `labour` + `produce` 12 ms, `consume` (E3 demand) 10 ms. The
//! migration carries are measured on their own: `price_refresh` (E3's unit `source` and
//! `dispatch` per traded good, for q_d and the lead time L) and `local_chain` (the serial E2
//! chain of local goods, every province). `kernel_build` is reported only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{self, cap_output};
use ceteris_econ::{EconInputs, EconParams, EconWorld, local};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::{Kernel, TradeParams};
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
    ("distribute", systems::distribute),
];

/// The calibrated full pack, or the fixture with a notice.
fn inputs() -> (EconInputs, &'static str) {
    let dir = std::env::var_os("CETERIS_PACK").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/packs/2000"),
        PathBuf::from,
    );
    if EconInputs::has_calibration(&dir) {
        let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
        let inputs = EconInputs::from_pack(&pack, &dir).expect("pack inputs load");
        return (inputs, "pack");
    }
    eprintln!(
        "notice: no calibrated pack at {}; benchmarking the fixture",
        dir.display()
    );
    (small_world(), "fixture")
}

/// One whole tick, as `Sim::step` runs it.
fn step(w: &mut EconWorld, tick: u32) {
    w.begin_tick();
    systems::schedule().run_tick(w, systems::SEED, tick);
    w.end_tick(tick).expect("invariants hold");
}

// Batched routines return their input, so that criterion drops it untimed.

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
    let (params, trade) = (EconParams::default(), TradeParams::default());
    // tick 1: its prices are those of tick 0's repricing, so the sourcing that `source`
    // keeps from tick 0 is stale at E3, as in any tick away from an exact fixed point
    let mut base = EconWorld::new(Arc::new(inputs), params, trade.clone());
    let posted = base.markets.price.clone();
    step(&mut base, 0);
    if base.markets.price == posted {
        eprintln!("notice: prices did not move at tick 0; E3 reuses the sourcing of T2");
    }
    let mut g = c.benchmark_group(format!("econ_{what}"));
    g.sample_size(20);

    for (n, (name, run)) in SYSTEMS.iter().enumerate() {
        let start = before(&base, n);
        g.bench_function(*name, |b| {
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

    let produce = SYSTEMS.iter().position(|s| s.0 == "produce").unwrap();
    let mut start = before(&base, produce);
    cap_output(&mut start);
    g.bench_function("local_chain", |b| {
        b.iter_batched(
            || start.clone(),
            |mut w| {
                local::chain_all(&mut w);
                w
            },
            BatchSize::LargeInput,
        )
    });

    let consume = SYSTEMS.iter().position(|s| s.0 == "consume").unwrap();
    let start = before(&base, consume);
    g.bench_function("price_refresh", |b| {
        b.iter_batched(
            || start.clone(),
            |mut w| {
                w.refresh_price_index();
                w
            },
            BatchSize::LargeInput,
        )
    });

    let start = base.clone();
    g.bench_function("end_tick", |b| {
        let w = before(&start, SYSTEMS.len());
        b.iter(|| w.end_tick(1).expect("invariants hold"))
    });
    g.bench_function("tick", |b| {
        b.iter_batched(
            || base.clone(),
            |mut w| {
                step(&mut w, 1);
                w
            },
            BatchSize::LargeInput,
        )
    });

    let inp = base.inputs();
    g.sample_size(10);
    g.bench_function("kernel_build", |b| {
        b.iter(|| Kernel::build(&inp.network, &inp.weight, &inp.wedges, &trade))
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
