//! Trade benchmarks (`specs/trade/SPEC.md`, Test plan) on the full pack from its calibrated
//! steady state: sourcing and dispatch of every traded good on the spending and fills of tick
//! 1, and the clearing and repricing of every market. They live in ceteris-econ, which builds
//! the world they run on. Without a calibrated pack (`CETERIS_PACK`, default
//! `data/packs/2000`) they run on the econ fixture, so `cargo bench --no-run` and CI work.
//!
//! Budgets per tick (M4 Pro): `source_dispatch` 5 ms, `clear_reprice` 2 ms; `kernel_build` is
//! reported only (2 s, not per tick). `source` keeps the origin access of its last prices: E3
//! sources each good cold at unit spending and dispatches it in full for q_d and the lead time
//! (the migration carry, `unit_source_dispatch`), then T2 sources at the same prices warm and
//! T2b dispatches the fills (`source_dispatch`). `source` alone is the cold sourcing. The tick
//! runs the goods in parallel, one task each: the `_parallel` benches time a tick's two trade
//! phases as it runs them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::goods::TRADED;
use ceteris_trade::{Kernel, N_GOODS, Sourced, dispatch, source};
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rayon::prelude::*;

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
    gamma: Vec<Vec<f64>>,
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
        gamma: w.layout().gamma.clone(),
    }
}

fn source_all(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    for (g, r) in out.iter_mut().enumerate() {
        source(k, g, &c.price[g], &c.active[g], &c.gamma[g], &c.spend[g], r);
    }
}

fn dispatch_all(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    for (g, r) in out.iter_mut().enumerate() {
        dispatch(k, g, &c.price[g], &c.fill[g], r);
    }
}

/// One task per good, sourcing and dispatch together, as E3 runs them.
fn unit_parallel(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    out.par_iter_mut().enumerate().for_each(|(g, r)| {
        source(k, g, &c.price[g], &c.active[g], &c.gamma[g], &c.spend[g], r);
        dispatch(k, g, &c.price[g], &c.fill[g], r);
    });
}

/// One task per good, sourcing for every good and then dispatch, as T2 and T2b run them.
fn tick_parallel(k: &Kernel, c: &Columns, out: &mut [Sourced]) {
    out.par_iter_mut().enumerate().for_each(|(g, r)| {
        source(k, g, &c.price[g], &c.active[g], &c.gamma[g], &c.spend[g], r);
    });
    out.par_iter_mut().enumerate().for_each(|(g, r)| {
        dispatch(k, g, &c.price[g], &c.fill[g], r);
    });
}

fn bench(c: &mut Criterion) {
    let (inputs, what) = inputs();
    let (params, trade) = (EconParams::default(), inputs.trade_params());
    let mut w = EconWorld::new(Arc::new(inputs), params, trade.clone());
    // tick 1, whose prices moved at tick 0's repricing: the sourcing kept from tick 0 is stale
    let posted = w.markets.price.clone();
    w.begin_tick();
    systems::schedule().run_tick(&mut w, systems::SEED, 0);
    w.end_tick(0).expect("invariants hold");
    if w.markets.price == posted {
        eprintln!("notice: prices did not move at tick 0; E3 reuses the sourcing of T2");
    }
    let stale = w.scratch.sourced.clone();
    w.begin_tick();
    for run in [
        systems::deliver,
        systems::labour,
        systems::produce,
        systems::demand,
    ] {
        run(&mut w);
    }
    // as E3 leaves it: sourced at this tick's prices, so T2 sources warm
    let refreshed = w.scratch.sourced.clone();
    // the markets as T3 finds them, then the fills it leaves for T2b
    systems::source(&mut w);
    let markets = w.markets.clone();
    let mut fill = w.scratch.fill.clone();
    let mut spoiled = w.scratch.spoiled.clone();
    systems::clear(&mut w);
    let cols = columns(&w);
    let unit = Columns {
        spend: cols
            .spend
            .iter()
            .map(|s| vec![[1.0, 0.0]; s.len()])
            .collect(),
        fill: cols.fill.iter().map(|f| vec![[1.0; 2]; f.len()]).collect(),
        price: cols.price.clone(),
        active: cols.active.clone(),
        gamma: cols.gamma.clone(),
    };
    let kernel = w.kernel();
    let mut sourced = stale.clone();
    source_all(kernel, &cols, &mut sourced);

    let mut g = c.benchmark_group(format!("trade_{what}"));
    g.sample_size(20);
    // `source` keeps the origin access of the last prices: cold is the first sourcing at new
    // prices (E3), warm a repeat at the same prices (T2)
    g.bench_function("source", |b| {
        b.iter_batched(
            || stale.clone(),
            |mut s| {
                source_all(kernel, &cols, &mut s);
                s
            },
            BatchSize::LargeInput,
        )
    });
    g.bench_function("source_warm", |b| {
        b.iter(|| source_all(kernel, &cols, &mut sourced))
    });
    g.bench_function("dispatch", |b| {
        b.iter(|| dispatch_all(kernel, &cols, &mut sourced))
    });
    // the critical path of the parallel stage, which runs one task per good
    g.bench_function("dispatch_one_good", |b| {
        b.iter(|| dispatch(kernel, 0, &cols.price[0], &cols.fill[0], &mut sourced[0]))
    });
    g.bench_function("source_dispatch", |b| {
        b.iter_batched(
            || refreshed.clone(),
            |mut s| {
                source_all(kernel, &cols, &mut s);
                dispatch_all(kernel, &cols, &mut s);
                s
            },
            BatchSize::LargeInput,
        )
    });
    g.bench_function("unit_source_dispatch", |b| {
        b.iter_batched(
            || stale.clone(),
            |mut s| {
                source_all(kernel, &unit, &mut s);
                dispatch_all(kernel, &unit, &mut s);
                s
            },
            BatchSize::LargeInput,
        )
    });
    // T2 and T2b from E3's sourcing, warm; E3 from tick 0's, cold
    type Run = fn(&Kernel, &Columns, &mut [Sourced]);
    let runs: [(&str, &Columns, &Vec<Sourced>, Run); 2] = [
        ("source_dispatch_parallel", &cols, &refreshed, tick_parallel),
        (
            "unit_source_dispatch_parallel",
            &unit,
            &stale,
            unit_parallel,
        ),
    ];
    for (name, columns, start, run) in runs {
        g.bench_function(name, |b| {
            b.iter_batched(
                || start.clone(),
                |mut s| {
                    run(kernel, columns, &mut s);
                    s
                },
                BatchSize::LargeInput,
            )
        });
    }
    let (rationing, pi) = (w.params().rationing, w.expected_inflation);
    let gain = w.layout().price_gain.clone();
    g.bench_function("clear_reprice", |b| {
        b.iter_batched(
            || markets.clone(),
            |mut m| {
                m.clear(rationing, &mut fill);
                m.clear_local(&mut spoiled);
                m.reprice(&trade, &gain, pi);
                m
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
