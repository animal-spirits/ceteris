//! The scaling's sweep over goods runs on any number of threads and gives bit-identical results
//! (ADR 0097): every good's operations and their order do not depend on the pool, and goods are
//! combined in index order. Compared by bits at 1, 2, 4 and every available thread, on the
//! fixture and the small CI pack. `CETERIS_SWEEP_TIMING=1 cargo test --release -p
//! ceteris-calibrate --test parallel_sweeps -- --nocapture` prints the wall clock per pool.
#![allow(clippy::disallowed_methods)] // tests are not hashed

use std::path::Path;
use std::time::Instant;

use ceteris_calibrate::steady::{self, Limits, Scaling, Steady};
use ceteris_reference::io::{self, Inputs, Mat};
use ceteris_reference::model::Model;

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
}

fn thread_counts() -> Vec<usize> {
    let all = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut t = vec![1, 2, 4, all];
    t.sort_unstable();
    t.dedup();
    t
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture() -> Inputs {
    io::load(root().join("tests/golden/p2-fixture")).unwrap()
}

fn pack_ci() -> Inputs {
    let out = tempfile::tempdir().unwrap();
    ceteris_calibrate::export_pack(&root().join("tests/fixtures/pack-ci"), out.path(), false)
        .unwrap();
    io::load(out.path()).unwrap()
}

/// A deterministic start: ln w of the active origins spread over [-3, 3), so the scaling takes
/// many sweeps.
fn start(inp: &Inputs) -> Mat {
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    Mat::from_fn(n_p, n_g, |p, g| {
        let mut x = ((p * n_g + g) as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 32;
        if inp.traded[g] && inp.active[p][g] {
            (x >> 11) as f64 / (1u64 << 53) as f64 * 6.0 - 3.0
        } else {
            0.0
        }
    })
}

/// The stage 2 inputs of a calibrated steady state: spending and sales of the traded goods.
fn stage2(inp: &Inputs, s: &Steady) -> (Mat, Mat) {
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let mut v = Mat::zeros(n_p, n_g);
    for (e, (&p, &g)) in inp.ent_province.iter().zip(&inp.ent_good).enumerate() {
        if inp.traded[g] {
            v[(p, g)] += s.value[e] / (1.0 + s.spoilage[g]);
        }
    }
    let spend = Mat::from_fn(n_p, n_g, |p, g| s.spend_hh[(p, g)] + s.spend_int[(p, g)]);
    (spend, v)
}

fn scaling_bits(s: &Scaling) -> (Vec<u64>, Vec<u64>, usize, u64) {
    (
        bits(&s.log_weight.data),
        bits(&s.r),
        s.sweeps,
        s.residual.to_bits(),
    )
}

fn check(name: &str, inp: &Inputs, full: bool) {
    let timing = std::env::var_os("CETERIS_SWEEP_TIMING").is_some();
    let quiet = &mut |_: &str| {};
    let steady =
        steady::calibrate_with(inp, &inp.u_star.clone(), Limits::default(), quiet).unwrap();
    let (spend, v) = stage2(inp, &steady);
    let m = Model::new(inp).unwrap();
    let from = start(inp);
    let mut reference = None;
    for t in thread_counts() {
        let p = pool(t);
        let (mut secs, mut cold, mut warm) = (f64::INFINITY, None, None);
        for _ in 0..if timing { 3 } else { 1 } {
            let t0 = Instant::now();
            cold = Some(p.install(|| steady::sinkhorn(&m, &spend, &v, None).unwrap()));
            warm = Some(p.install(|| steady::sinkhorn(&m, &spend, &v, Some(&from)).unwrap()));
            secs = secs.min(t0.elapsed().as_secs_f64());
        }
        let (cold, warm) = (cold.unwrap(), warm.unwrap());
        // the whole fixed point on top of the scalings, where it is cheap enough to repeat
        let whole = full.then(|| {
            let s = p.install(|| {
                steady::calibrate_with(inp, &inp.u_star.clone(), Limits::default(), quiet).unwrap()
            });
            (
                bits(&s.price.data),
                bits(&s.rebate),
                bits(&s.consumer.data),
                s.info.pass_history,
            )
        });
        let got = (scaling_bits(&cold), scaling_bits(&warm), whole);
        if timing {
            let digest = format!("{:?}", (scaling_bits(&cold), scaling_bits(&warm)));
            let h = digest.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
            let sorted = |g: &[usize]| g.windows(2).all(|w| w[0] <= w[1]);
            eprintln!(
                "{name}: digest {h:016x}, legs sorted by good: {} {} {}",
                sorted(&inp.domestic.good),
                sorted(&inp.access.good),
                sorted(&inp.crossing.good)
            );
            eprintln!(
                "{name}: {t} threads {secs:.3} s (best of 3), sweeps cold {} warm {}",
                cold.sweeps, warm.sweeps
            );
        }
        match &reference {
            None => reference = Some(got),
            Some(r) => assert!(*r == got, "{name}: {t} threads differ from 1 thread"),
        }
    }
}

#[test]
fn fixture_scaling_is_bit_identical_at_any_thread_count() {
    check("fixture", &fixture(), true);
}

/// Minutes: one calibration of the pack, so run with `--include-ignored` (release).
#[test]
#[ignore = "calibrates the small pack once, about 3 minutes in release"]
fn small_pack_scaling_is_bit_identical_at_any_thread_count() {
    check("pack-ci", &pack_ci(), false);
}

/// The per-good sums are the all-goods sums' slices, bit for bit.
#[test]
fn per_good_sums_are_the_slices_of_the_all_goods_sums() {
    use ceteris_reference::model::Factors;
    let inp = fixture();
    let m = Model::new(&inp).unwrap();
    let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
    let f = Factors {
        dom: &inp.domestic.k,
        acc: &inp.access.k,
        cro: &inp.crossing.k,
    };
    let w: Vec<f64> = (0..n_p * n_g)
        .map(|i| 1.0 + (i % 7) as f64 * 0.37)
        .collect();
    let (all_o, all_d) = (m.origin_sums(&w, f), m.destination_sums(&w, f));
    for g in 0..n_g {
        let wg = &w[g * n_p..(g + 1) * n_p];
        assert_eq!(
            bits(&m.origin_sums_good(g, wg, f)),
            bits(&all_o[g * n_p..(g + 1) * n_p])
        );
        assert_eq!(
            bits(&m.destination_sums_good(g, wg, f)),
            bits(&all_d[g * n_p..(g + 1) * n_p])
        );
    }
}
