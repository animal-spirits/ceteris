#![allow(clippy::disallowed_methods)] // tests are not hashed
//! ADR 0267: the block map is the linearized tick. On the calibrated fixture with its gains
//! raised until a country's block is unstable, a small price perturbation of that country run
//! through `EconWorld` grows at the block's spectral radius, in the default mode and
//! frictionless. Run with `--release`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ceteris_calibrate::block::{self, Block};
use ceteris_calibrate::steady::{self, Steady};
use ceteris_calibrate::write;
use ceteris_core::sim::Sim;
use ceteris_econ::fixture::small_world;
use ceteris_econ::systems::{SEED, schedule};
use ceteris_econ::{EconInputs, EconParams, EconWorld};
use ceteris_reference::io::{self, Inputs};
use ceteris_reference::model::Model;
use ceteris_trade::TradeParams;
use ceteris_trade::goods::p2::N as N_GOODS;

fn fixture() -> PathBuf {
    std::env::var_os("CETERIS_FIXTURE_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/p2-fixture"),
        PathBuf::from,
    )
}

/// The calibrated fixture: its reference inputs, its steady state and the econ inputs loaded
/// from the tables it writes.
fn calibrated() -> (Inputs, Steady, EconInputs) {
    let inp = io::load(fixture()).unwrap();
    let s = steady::calibrate(&inp, &inp.u_star.clone(), &mut |_: &str| {}).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let provinces: Vec<usize> = (0..inp.n_provinces()).collect();
    write::write(tmp.path(), &inp, &s, &provinces).unwrap();
    let mut econ = small_world();
    econ.calibrate(tmp.path()).unwrap();
    (inp, s, econ)
}

/// The blocks under `params` at the default gains.
fn blocks(inp: &Inputs, s: &Steady, params: &EconParams) -> Vec<(Vec<usize>, Block)> {
    let m = Model::physical(inp).unwrap();
    let src = m.sourcing(&s.price);
    block::blocks(&m, &src, s, &inp.ent_s_l(), params, &TradeParams::default())
}

/// J2's gain limit 2 / J of each cell (good-major) at employment speed `eta`.
fn j2_limit(s: &Steady, n_p: usize, eta: f64) -> impl Fn(usize) -> f64 + '_ {
    move |j: usize| {
        let (p, g) = (j % n_p, j / n_p);
        2.0 / (s.demand_elasticity[(p, g)] + eta * s.supply_elasticity[(p, g)] / (2.0 - eta))
    }
}

/// Growth per tick of the largest log price deviation of `cells` (good-major) after a
/// perturbation of `size` of their prices, read from the window maxima of `windows`, in a run
/// whose price gain is `gain` per cell (good-major): the per-market cap at kappa1 10 and P03d
/// 1 is 2 / J, and the block limit, set to `gain`, takes it below.
fn tick_growth(
    econ: &EconInputs,
    params: &EconParams,
    gain: &dyn Fn(usize) -> f64,
    cells: &[usize],
    size: f64,
    windows: [(u32, u32); 2],
) -> f64 {
    let n_p = econ.n_provinces();
    let mut econ = econ.clone();
    let limit: Vec<f64> = (0..n_p * N_GOODS)
        .map(|c| gain((c % N_GOODS) * n_p + c / N_GOODS))
        .collect();
    econ.market_elasticity.as_mut().unwrap().limit = [limit.clone(), limit];
    let trade = TradeParams {
        kappa1: 10.0,
        gain_share: 1.0,
        expectation_gain: 0.0,
        ..econ.trade_params()
    };
    let mut w = EconWorld::new(Arc::new(econ), params.clone(), trade);
    let at: Vec<usize> = cells
        .iter()
        .map(|&j| (j % n_p) * N_GOODS + j / n_p)
        .collect();
    let p0 = w.markets.price.clone();
    for (k, &c) in at.iter().enumerate() {
        let f = ((k as f64 + 1.0) * 0.618_033_988_75).fract() * 2.0 - 1.0;
        w.markets.price[c] *= (size * f).exp();
    }
    let mut sim = Sim::new(w, schedule(), SEED);
    let mut max = [0.0f64; 2];
    let end = windows[1].1;
    for t in 1..=end {
        sim.step().unwrap();
        let w = sim.world();
        let d = at
            .iter()
            .map(|&c| (w.markets.price[c] / p0[c]).ln().abs())
            .fold(0.0, f64::max);
        for (m, &(a, b)) in max.iter_mut().zip(&windows) {
            if t >= a && t < b {
                *m = m.max(d);
            }
        }
    }
    (max[1] / max[0]).powf(1.0 / f64::from(windows[1].0 - windows[0].0))
}

/// The two couplings the test raises: substitution between origins (every cell of the block at
/// one gain x) and input orders (the block's mining cells, good 2, at x times their J2 limit).
#[derive(Clone, Copy, Debug)]
enum Case {
    Substitution,
    Orders,
}

/// ADR 0267, the map against the tick. On the calibrated fixture the gains of one country's
/// cells are raised (every other cell at 0.02, or its J2 limit below that) to 1% beyond the
/// point where its block crosses radius 1, with every cell alone still inside its J2 limit
/// (2 / J): a coupled instability, of substitution between origins and of input orders through
/// the order-up-to position. Prices of the block's cells are perturbed by 1e-6, and their
/// largest log deviation grows over ticks 100 to 300 at the block's spectral radius, to within
/// the measurement tolerance the calibration decides stability with (`block::TOLERANCE`): the
/// window estimates agree to about 1e-5, the radius is read to about 1e-3, and the deviations
/// stay below 1e-3, so the price step's clip and the local stock orders' kink (orders only where
/// output passes its past peak) stay out. The default mode runs without downward wage rigidity
/// (delta_w 0), so that its wage rule is one linear map; expected inflation is held at 0 (gain
/// 0), as the block map holds it.
#[test]
fn block_map_growth_matches_the_tick() {
    let (inp, s, econ) = calibrated();
    let n_p = inp.n_provinces();
    for params in [
        EconParams {
            delta_w: 0.0,
            ..EconParams::default()
        },
        EconParams::frictionless(),
    ] {
        let j2 = j2_limit(&s, n_p, params.eta);
        let blocks = blocks(&inp, &s, &params);
        for case in [Case::Substitution, Case::Orders] {
            let raised = |j: usize, x: f64| match case {
                Case::Substitution => x,
                Case::Orders if j / n_p == 2 => x * j2(j),
                Case::Orders => 0.02f64.min(j2(j)),
            };
            let radius = |b: &Block, cells: &[usize], x: f64| {
                let mut b = b.clone();
                for (k, &j) in cells.iter().enumerate() {
                    b.gain[k] = raised(j, x);
                }
                b.spectral_radius(1.0, params.eta)
            };
            let hi0 = match case {
                Case::Substitution => 0.2,
                Case::Orders => 1.0,
            };
            // the block that crosses first, and where
            let (crossing, cells, b) = blocks
                .iter()
                .filter(|(cells, b)| radius(b, cells, hi0) > 1.0)
                .map(|(cells, b)| {
                    let (mut lo, mut hi) = (0.0, hi0);
                    for _ in 0..20 {
                        let mid = 0.5 * (lo + hi);
                        if radius(b, cells, mid) > 1.0 {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                    }
                    (hi, cells, b)
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("a block crosses");
            let x = 1.01 * crossing;
            let map = radius(b, cells, x);
            assert!(
                cells.iter().all(|&j| raised(j, x) < j2(j)),
                "{case:?}: a cell alone is beyond J2"
            );
            let gain = |j: usize| {
                if cells.contains(&j) {
                    raised(j, x)
                } else {
                    0.02f64.min(j2(j))
                }
            };
            let tick = tick_growth(&econ, &params, &gain, cells, 1e-6, [(100, 150), (250, 300)]);
            eprintln!(
                "eta {} {case:?}: crossing at {crossing:.5}, radius {map:.5}, tick growth {tick:.5}",
                params.eta
            );
            assert!(map > 1.0 + 2.0 * block::TOLERANCE, "radius {map}");
            assert!(
                (tick - map).abs() <= block::TOLERANCE,
                "eta {} {case:?}: tick {tick} vs map {map}",
                params.eta
            );
        }
    }
}
