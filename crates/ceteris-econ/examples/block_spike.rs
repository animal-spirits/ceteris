//! RFC 0317 S3 spike: `dispatch_block::<B>` against `dispatch` per good on the pack world with
//! `--goods` goods. Steps `--ticks` ticks, then replays the last tick's dispatch inputs.
//! `--only scalar|block` runs one mode for `--reps` single-thread passes (wrap in
//! `/usr/bin/time -l` for instructions retired); without it prints the timing table.
//! cargo run --release -p ceteris-econ --example block_spike -- --goods 45 --threads 14 --b 8

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use ceteris_econ::synthetic::with_goods;
use ceteris_econ::world::Columns;
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_geo::graph::TransportParams;
use ceteris_geo::pack::{Pack, default_dir};
use ceteris_trade::{Block, Kernel, Sourced, dispatch};
use rayon::prelude::*;

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn ms(f: impl FnOnce()) -> f64 {
    let t = Instant::now();
    f();
    t.elapsed().as_secs_f64() * 1e3
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

struct Setup<'a> {
    kernel: &'a Kernel,
    col: &'a [Columns],
    sourced: &'a [Sourced],
    alone: &'a [usize],
    reps: usize,
    threads: usize,
    only: &'a str,
}

fn bench<const B: usize>(s: &Setup, blocks: &[Vec<usize>]) {
    let (kernel, col) = (s.kernel, s.col);
    let price =
        |gs: &[usize]| -> [&[f64]; B] { std::array::from_fn(|l| col[gs[l]].price.as_slice()) };
    let fill =
        |gs: &[usize]| -> [&[[f64; 2]]; B] { std::array::from_fn(|l| col[gs[l]].fill.as_slice()) };
    let mut outs: Vec<[Sourced; B]> = blocks
        .iter()
        .map(|gs| std::array::from_fn(|l| s.sourced[gs[l]].clone()))
        .collect();
    let mut bufs: Vec<Block<B>> = blocks.iter().map(|_| Block::default()).collect();
    let pass = |gs: &[usize], bk: &mut Block<B>, o: &mut [Sourced; B]| {
        bk.gather(
            kernel,
            std::array::from_fn(|l| gs[l]),
            price(gs),
            fill(gs),
            o,
        );
        bk.run(kernel, gs[0]);
        bk.scatter(o);
    };
    if s.only == "block" {
        for _ in 0..s.reps {
            for ((gs, o), bk) in blocks.iter().zip(&mut outs).zip(&mut bufs) {
                pass(gs, bk, o);
            }
        }
        return;
    }
    // one thread: one good's dispatch against a block's
    let mut scratch = s.sourced.to_vec();
    let one: Vec<f64> = blocks
        .iter()
        .flatten()
        .map(|&g| {
            let mut t: Vec<f64> = (0..s.reps)
                .map(|_| ms(|| dispatch(kernel, g, &col[g].price, &col[g].fill, &mut scratch[g])))
                .collect();
            median(&mut t)
        })
        .collect();
    let one_mean = mean(&one);
    let (mut tg, mut tr, mut ts) = (vec![], vec![], vec![]);
    for ((gs, o), bk) in blocks.iter().zip(&mut outs).zip(&mut bufs) {
        let (mut a, mut r, mut c) = (vec![], vec![], vec![]);
        for _ in 0..s.reps {
            a.push(ms(|| {
                bk.gather(
                    kernel,
                    std::array::from_fn(|l| gs[l]),
                    price(gs),
                    fill(gs),
                    o,
                )
            }));
            r.push(ms(|| bk.run(kernel, gs[0])));
            c.push(ms(|| bk.scatter(o)));
        }
        tg.push(median(&mut a));
        tr.push(median(&mut r));
        ts.push(median(&mut c));
    }
    let (g, r, c) = (mean(&tg), mean(&tr), mean(&ts));
    println!(
        "B={B}: one good {one_mean:.3} ms | block kernel {r:.3} ms = {:.2}x one | gather {g:.3} scatter {c:.3} | \
         with transposes {:.2}x one | kernel per good {:.2}x scalar",
        r / one_mean,
        (g + r + c) / one_mean,
        r / (B as f64 * one_mean)
    );
    // all threads: every good scalar against blocks plus the goods left alone
    let goods: Vec<usize> = blocks.iter().flatten().chain(s.alone).copied().collect();
    let (mut sc, mut bl) = (vec![], vec![]);
    for _ in 0..s.reps {
        let mut all = s.sourced.to_vec();
        sc.push(ms(|| {
            all.par_iter_mut().enumerate().for_each(|(g, o)| {
                if goods.contains(&g) {
                    dispatch(kernel, g, &col[g].price, &col[g].fill, o);
                }
            });
        }));
        let mut left: Vec<(usize, Sourced)> =
            s.alone.iter().map(|&g| (g, s.sourced[g].clone())).collect();
        bl.push(ms(|| {
            blocks
                .par_iter()
                .zip(outs.par_iter_mut())
                .zip(bufs.par_iter_mut())
                .for_each(|((gs, o), bk)| pass(gs, bk, o));
            left.par_iter_mut()
                .for_each(|(g, o)| dispatch(kernel, *g, &col[*g].price, &col[*g].fill, o));
        }));
    }
    println!(
        "all {} goods at {} threads: scalar {:.3} ms, B={B} blocks {:.3} ms",
        goods.len(),
        s.threads,
        median(&mut sc),
        median(&mut bl)
    );
}

fn main() {
    let (mut ticks, mut threads, mut goods, mut b, mut reps) =
        (3u32, 14usize, 45usize, 8usize, 30usize);
    let mut only = String::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut v = || args.next().expect("a value");
        match a.as_str() {
            "--ticks" => ticks = v().parse().unwrap(),
            "--threads" => threads = v().parse().unwrap(),
            "--goods" => goods = v().parse().unwrap(),
            "--b" => b = v().parse().unwrap(),
            "--reps" => reps = v().parse().unwrap(),
            "--only" => only = v(),
            other => panic!("unknown argument '{other}'"),
        }
    }
    let dir = default_dir();
    let pack = Pack::load(&dir, &TransportParams::default()).expect("pack loads");
    let inputs = Arc::new(EconInputs::from_pack(&pack, &dir).expect("calibrated pack inputs"));
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap();
    pool.install(|| {
        let trade = inputs.trade_params();
        let (inputs, trade) = if goods > 0 {
            let (i, t) = with_goods(&inputs, &trade, goods);
            (Arc::new(i), t)
        } else {
            (Arc::clone(&inputs), trade)
        };
        let legs = inputs.network.good_legs.as_ref();
        let mut classes = BTreeMap::<(u64, Option<u64>), Vec<usize>>::new();
        for g in trade.goods.traded_goods() {
            let key = (trade.theta[g].to_bits(), legs.map(|l| l.kappa[g].to_bits()));
            classes.entry(key).or_default().push(g);
        }
        let mut sim = EconWorld::sim(inputs, EconParams::default(), trade);
        for _ in 0..ticks {
            sim.step().unwrap_or_else(|v| panic!("{v}"));
        }
        let w = sim.world();
        let mut blocks: Vec<Vec<usize>> = vec![];
        let mut alone: Vec<usize> = vec![];
        for gs in classes.values() {
            let mut it = gs.chunks_exact(b);
            blocks.extend(it.by_ref().map(<[usize]>::to_vec));
            alone.extend(it.remainder());
        }
        eprintln!(
            "{} classes, {} blocks of {b}, {} goods alone",
            classes.len(),
            blocks.len(),
            alone.len()
        );
        let s = Setup {
            kernel: w.kernel(),
            col: &w.scratch.columns,
            sourced: &w.scratch.sourced,
            alone: &alone,
            reps,
            threads,
            only: &only,
        };
        if only == "scalar" {
            let mut scratch = s.sourced.to_vec();
            for _ in 0..reps {
                for &g in classes.values().flatten() {
                    dispatch(
                        s.kernel,
                        g,
                        &s.col[g].price,
                        &s.col[g].fill,
                        &mut scratch[g],
                    );
                }
            }
            return;
        }
        match b {
            4 => bench::<4>(&s, &blocks),
            8 => bench::<8>(&s, &blocks),
            _ => panic!("--b 4 or 8"),
        }
    });
}
