//! The ADR 0294 identity gate: runs the calibrated pack (`CETERIS_PACK`) for N ticks (default
//! 24) at the given rayon thread count and prints, per tick, the state hash and the hash of
//! that tick's ledger journal. A speed change that claims no output change prints the same
//! lines as `main` at 1 thread and at all threads; `--timing` must not change them either.
//!
//! cargo run --release -p ceteris-econ --example identity -- [--ticks N] [--threads T] [--goods N] [--timing]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use ceteris_core::snapshot::{Encoder, hex, state_hash};
use ceteris_core::timing;
use ceteris_econ::synthetic::with_goods;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};

fn main() {
    let (mut ticks, mut threads, mut goods, mut timed) = (24u32, 0usize, 0usize, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().and_then(|v| v.parse().ok()).expect("a number");
        match a.as_str() {
            "--ticks" => ticks = value() as u32,
            "--threads" => threads = value(),
            "--goods" => goods = value(),
            "--timing" => timed = true,
            other => panic!("unknown argument '{other}'"),
        }
    }
    let dir = default_dir();
    let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    let inputs = Arc::new(EconInputs::from_pack(&pack, &dir).expect("calibrated pack inputs"));
    if timed {
        timing::enable();
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("thread pool");
    pool.install(|| {
        let trade = inputs.trade_params();
        let (inputs, trade) = if goods > 0 {
            let (i, t) = with_goods(&inputs, &trade, goods);
            (Arc::new(i), t)
        } else {
            (Arc::clone(&inputs), trade)
        };
        if timed {
            // goods sharing theta and kappa share a kernel (DESIGN 8.3)
            let legs = inputs.network.good_legs.as_ref();
            let classes: std::collections::BTreeSet<(u64, Option<u64>)> = trade
                .goods
                .traded_goods()
                .map(|g| (trade.theta[g].to_bits(), legs.map(|l| l.kappa[g].to_bits())))
                .collect();
            eprintln!(
                "kernel classes     {} of {} traded goods",
                classes.len(),
                trade.goods.traded
            );
        }
        let mut sim = EconWorld::sim(inputs, EconParams::default(), trade);
        sim.world_mut().scratch.detail = true;
        for _ in 0..ticks {
            sim.step().unwrap_or_else(|v| panic!("{v}"));
            let mut e = Encoder::default();
            for x in sim.world().ledger.journal() {
                e.u16(x.flow.0);
                e.u32(x.from.0);
                e.u32(x.to.0);
                e.i64(x.amount);
            }
            let journal = state_hash(&e.into_bytes());
            println!(
                "{} {} {}",
                sim.tick(),
                hex(&sim.state_hash()),
                hex(&journal)
            );
        }
    });
    if timed {
        // per name and tick the summed span time; per-good spans add up to CPU, not wall, time
        let mut sums = std::collections::BTreeMap::<(&str, u32), u64>::new();
        for sp in timing::take() {
            *sums.entry((sp.name, sp.tick)).or_default() += sp.end - sp.start;
        }
        let mut names = std::collections::BTreeMap::<&str, Vec<u64>>::new();
        for ((name, _), ns) in sums {
            names.entry(name).or_default().push(ns);
        }
        for (name, mut v) in names {
            v.sort_unstable();
            eprintln!("{name:<18} median {:>8.3} ms", v[v.len() / 2] as f64 / 1e6);
        }
    }
}
