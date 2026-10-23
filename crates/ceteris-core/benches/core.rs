use std::hint::black_box;

use ceteris_core::entity::{EntityId, Kind};
use ceteris_core::reduce::fixed_sum;
use ceteris_core::rng::Seed;
use ceteris_core::schedule::SystemId;
use ceteris_core::toy::Toy;
use criterion::{Criterion, criterion_group, criterion_main};

fn rng(c: &mut Criterion) {
    c.bench_function("philox 1M u32", |b| {
        b.iter(|| {
            let mut s = Seed(black_box(1)).stream(0, SystemId(0), Kind(0), EntityId(0));
            (0..1_000_000).fold(0u32, |acc, _| acc ^ s.next_u32())
        })
    });
}

fn reduce(c: &mut Criterion) {
    let xs: Vec<f64> = (0..1_000_000).map(|i| f64::from(i).sqrt()).collect();
    c.bench_function("fixed_sum 1M", |b| b.iter(|| fixed_sum(black_box(&xs))));
}

fn toy(c: &mut Criterion) {
    c.bench_function("toy 400 provinces, 12 ticks", |b| {
        b.iter_batched(
            || Toy::sim(400, Seed(7)),
            |mut sim| sim.run_until(12).unwrap(),
            criterion::BatchSize::LargeInput,
        )
    });
}

criterion_group!(benches, rng, reduce, toy);
criterion_main!(benches);
