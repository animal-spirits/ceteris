//! The ADR 0294 identity gate: runs the calibrated pack (`CETERIS_PACK`) for N ticks (default
//! 24) at the given rayon thread count and prints, per tick, the state hash and the hash of
//! that tick's ledger journal. A speed change that claims no output change prints the same
//! lines as `main` at 1 thread and at all threads; `--timing` must not change them either.
//!
//! cargo run --release -p ceteris-econ --example identity -- [--ticks N] [--threads T] [--timing]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use ceteris_core::snapshot::{Encoder, hex, state_hash};
use ceteris_core::timing;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};

fn main() {
    let (mut ticks, mut threads, mut timed) = (24u32, 0usize, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut value = || args.next().and_then(|v| v.parse().ok()).expect("a number");
        match a.as_str() {
            "--ticks" => ticks = value() as u32,
            "--threads" => threads = value(),
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
        let mut sim = EconWorld::sim(Arc::clone(&inputs), EconParams::default(), trade);
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
}
