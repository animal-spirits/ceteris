//! Trade benchmarks (`specs/trade/SPEC.md`, Test plan) on the full pack from its calibrated
//! steady state: sourcing and dispatch of every traded good on the spending and fills of the
//! first tick, and the clearing and repricing of every market. Without a calibrated pack
//! (`CETERIS_PACK`, default `data/packs/2000`) they run on the econ fixture, so `cargo bench
//! --no-run` and CI work.
//!
//! Budgets per tick (M4 Pro): `source_dispatch` 5 ms, `clear_reprice` 2 ms; `kernel_build` is
//! reported only (2 s, not per tick).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::goods::TRADED;
use ceteris_trade::{Kernel, N_GOODS, Sourced, TradeParams, dispatch, source};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

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

/// Per traded good, the origin columns `source` and `dispatch` read in the tick.
struct Columns {
    price: Vec<Vec<f64>>,
    active: Vec<Vec<bool>>,
    spend: Vec<Vec<[f64; 2]>>,
    fill: Vec<Vec<[f64; 2]>>,
}

fn columns(w: &EconWorld) -> Columns {
    let np = w.inputs().n_provinces();
    let traded = (0..N_GOODS).filter(|&g| TRADED[g]);
    let column = |g: usize| (0..np).map(move |o| o * N_GOODS + g);
    let (m, sc) = (&w.markets, &w.scratch);
    Columns {
        price: traded
            .clone()
            .map(|g| column(g).map(|c| m.price[c]).collect())
            .collect(),
        active: traded
            .clone()
            .map(|g| column(g).map(|c| m.active[c]).collect())
            .collect(),
        spend: traded
            .clone()
            .map(|g| column(g).map(|c| sc.class_spend[c]).collect())
            .collect(),
        fill: traded
            .map(|g| column(g).map(|c| sc.fill[c]).collect())
            .collect(),
    }
}

fn source_all(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    for (g, r) in out.iter_mut().enumerate() {
        source(k, g, &c.price[g], &c.active[g], &c.spend[g], r);
    }
}

fn dispatch_all(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    for (g, r) in out.iter_mut().enumerate() {
        dispatch(k, g, &c.price[g], &c.fill[g], r);
    }
}

fn bench(c: &mut Criterion) {
    let (inputs, what) = inputs();
    let (params, trade) = (EconParams::default(), TradeParams::default());
    let mut w = EconWorld::new(Arc::new(inputs), params, trade.clone());
    w.begin_tick();
    for run in [
        systems::deliver,
        systems::labour,
        systems::produce,
        systems::demand,
    ] {
        run(&mut w);
    }
    // the markets as T3 finds them, then the fills it leaves for T2b
    systems::source(&mut w);
    let markets = w.markets.clone();
    let mut fill = w.scratch.fill.clone();
    let mut spoiled = w.scratch.spoiled.clone();
    systems::clear(&mut w);
    let cols = columns(&w);
    let kernel = w.kernel();
    let n_traded = TRADED.iter().filter(|&&t| t).count();
    let mut sourced: Vec<Sourced> = (0..n_traded).map(|_| Sourced::new()).collect();
    source_all(kernel, &cols, &mut sourced);
    dispatch_all(kernel, &cols, &mut sourced);

    let mut g = c.benchmark_group(format!("trade_{what}"));
    g.sample_size(20);
    g.bench_function("source", |b| {
        b.iter(|| source_all(kernel, &cols, &mut sourced))
    });
    g.bench_function("dispatch", |b| {
        b.iter(|| dispatch_all(kernel, &cols, &mut sourced))
    });
    g.bench_function("source_dispatch", |b| {
        b.iter(|| {
            source_all(kernel, &cols, &mut sourced);
            dispatch_all(kernel, &cols, &mut sourced);
        })
    });
    let (rationing, pi) = (w.params().rationing, w.expected_inflation);
    g.bench_function("clear_reprice", |b| {
        b.iter_batched(
            || markets.clone(),
            |mut m| {
                m.clear(rationing, &mut fill);
                m.clear_local(&mut spoiled);
                m.reprice(&trade, pi);
            },
            BatchSize::LargeInput,
        )
    });
    let inp = w.inputs();
    g.sample_size(10);
    g.bench_function("kernel_build", |b| {
        b.iter(|| Kernel::build(&inp.network, &inp.weight, &inp.wedges, &trade))
    });
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
