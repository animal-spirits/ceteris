//! Per-system cost against the goods count (ADR 0235, task G2): every system of the tick at 10,
//! 20 and 45 synthetic goods (`ceteris_econ::synthetic`), from the data state after one tick,
//! on the calibrated pack (`CETERIS_PACK`) or the fixture. The medians fit a fixed plus a
//! per-good cost per system (spec 19.2); the synthetic worlds are never a calibration input.
//! The stages outside the nine systems (begin_tick, the kernel advance and drift, the invariant
//! system, end_tick) and the whole tick are timed beside them, so the parts can be summed
//! against the tick. `GOODS_SPANS=<ticks>` also runs that many ticks of the sim loop with the
//! timing spans on (`ceteris_core::timing`, bit-identical) and prints each span's per-tick
//! min, median and max in criterion's `time: [..]` form, so `scripts/bench_quiet.sh` records
//! them; spans of one name in one tick add up, except `t2b.good`, the tick's slowest good.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_core::sim::State;
use ceteris_core::timing;
use ceteris_econ::fixture::small_world;
use ceteris_econ::synthetic::with_goods;
use ceteris_econ::systems;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::Pack;
use ceteris_trade::params::TradeParams;
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
        let synthetic = Arc::new(synthetic);
        let mut base = EconWorld::new(synthetic.clone(), EconParams::default(), trade.clone());
        step(&mut base, 0);
        let mut time = |name: &str, start: &EconWorld, run: &dyn Fn(&mut EconWorld)| {
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
        };
        time("begin_tick", &base, &|w| w.begin_tick());
        // the schedule's deliver system runs these before systems::deliver
        time("kernel", &before(&base, 0), &|w| {
            w.advance_kernel();
            systems::drift(w, 1);
        });
        for (i, (name, run)) in SYSTEMS.iter().enumerate() {
            time(name, &before(&base, i), run);
        }
        let mut done = before(&base, SYSTEMS.len());
        time("assert", &done, &|w| systems::assert_invariants(w, 1));
        systems::assert_invariants(&mut done, 1);
        time("end_tick", &done, &|w| {
            w.end_tick(1).expect("invariants hold")
        });
        time("tick", &base, &|w| step(w, 1));
        drop(base);
        if let Some(ticks) = std::env::var("GOODS_SPANS")
            .ok()
            .and_then(|v| v.parse().ok())
        {
            spans(&format!("goods_{what}"), n, synthetic, trade, ticks);
        }
    }
    g.finish();
}

/// Per-tick span times of `ticks` sim-loop ticks after a first, untimed one.
fn spans(group: &str, n: usize, inputs: Arc<EconInputs>, trade: TradeParams, ticks: u32) {
    let mut sim = EconWorld::sim(inputs, EconParams::default(), trade);
    sim.step().expect("invariants hold");
    timing::enable();
    let mut per_tick: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    for _ in 0..ticks {
        timing::take();
        sim.step().expect("invariants hold");
        let mut tick: BTreeMap<&str, f64> = BTreeMap::new();
        for s in timing::take() {
            let ms = (s.end - s.start) as f64 / 1e6;
            let slot = tick.entry(s.name).or_default();
            *slot = if s.name == "t2b.good" {
                slot.max(ms)
            } else {
                *slot + ms
            };
        }
        for (name, ms) in tick {
            per_tick.entry(name).or_default().push(ms);
        }
    }
    for (name, mut ms) in per_tick {
        ms.sort_by(f64::total_cmp);
        println!(
            "{group}/span/{name}/{n}\n                        time:   [{:.4} ms {:.4} ms {:.4} ms]",
            ms[0],
            ms[ms.len() / 2],
            ms[ms.len() - 1]
        );
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
