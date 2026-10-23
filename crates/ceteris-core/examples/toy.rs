//! Runs the P0 toy world: `cargo run --release --example toy -- [ticks] [provinces] [seed] [threads]`.

use std::time::Instant;

use ceteris_core::rng::Seed;
use ceteris_core::snapshot::hex;
use ceteris_core::toy::{FIRM_ENTRY, FIRM_EXIT, OUTPUT_SHOCK, Toy};

fn main() {
    let arg = |i: usize, default: u64| {
        std::env::args()
            .nth(i)
            .map_or(default, |a| a.parse().expect("numeric argument"))
    };
    let (ticks, provinces, seed, threads) = (
        arg(1, 1200) as u32,
        arg(2, 10) as usize,
        arg(3, 42),
        arg(4, 0) as usize,
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    let mut sim = Toy::sim(provinces, Seed(seed));
    let start = Instant::now();
    pool.install(|| sim.run_until(ticks))
        .unwrap_or_else(|v| panic!("{v}"));
    let elapsed = start.elapsed();
    let world = sim.world();
    let count = |k| world.events.events().iter().filter(|e| e.kind == k).count();
    println!(
        "ticks {ticks}, provinces {provinces}, seed {seed}, threads {}",
        pool.current_num_threads()
    );
    println!(
        "firms {}, entries {}, exits {}, output shocks {}",
        world.firm_id.len(),
        count(FIRM_ENTRY),
        count(FIRM_EXIT),
        count(OUTPUT_SHOCK)
    );
    let (lo, hi) = world
        .price
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &p| (lo.min(p), hi.max(p)));
    println!("price range {lo:.2} .. {hi:.2}");
    println!(
        "{:.3} ms per tick",
        elapsed.as_secs_f64() * 1e3 / f64::from(ticks)
    );
    println!("state hash {}", hex(&sim.state_hash()));
}
