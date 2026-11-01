//! ADR 0091: the storage loop's linearized region (trade SPEC, Storage loop), the 4 x 4 map of
//! `ceteris_calibrate::stability::storage_matrix`, against the toy market of agriculture run
//! through `Markets::store`, `clear` and `reprice`.

use ceteris_calibrate::stability::{Storage, storage_matrix};
use ceteris_trade::goods::p2::N as N_GOODS;
use ceteris_trade::{Goods, Markets, Rationing, TradeParams};

/// Agriculture, competitively stored (ADR 0091).
const AG: usize = 0;

/// The storage loop of agriculture at the approved storage values: the spectral radius of
/// [`storage_matrix`], as the growth rate of its iterates over the second half of 8,000 ticks.
fn storage_radius(e: f64, eta: f64, eps: f64, kappa: f64) -> f64 {
    let p = TradeParams::default().with_storage();
    let st = Storage {
        delta: p.storage_loss[AG],
        cost: p.storage_cost[AG],
        zeta: p.convenience[AG],
        psi: p.speculation[AG],
        gain: p.storage_gain,
    };
    let m = storage_matrix(e, eta, eps, p.coverage[AG], kappa, st);
    let mut v = [1.0, 0.3, -0.7, 0.2];
    let mut log = 0.0;
    const N: usize = 8000;
    for t in 0..N {
        let w: [f64; 4] = std::array::from_fn(|i| (0..4).map(|j| m[i][j] * v[j]).sum());
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        if t >= N / 2 {
            log += norm.ln();
        }
        v = w.map(|x| x / norm);
    }
    (log / (N / 2) as f64).exp()
}

/// Runs agriculture's origin market at the approved storage values from a price deviation x0: output Q = Q* exp(q)
/// with Q* = 1 + delta s, household orders D = exp(-E x), then `store`, `clear` and `reprice` as
/// built. Returns the largest of |x|, |q|, |S / S* - 1| and |pi^e| over the last 100 ticks.
fn simulate_storage(e: f64, eta: f64, eps: f64, kappa: f64, x0: f64, steps: usize) -> f64 {
    let p = TradeParams::default().with_storage();
    let s_star = p.coverage[AG];
    let q_star = 1.0 + p.storage_loss[AG] * s_star;
    let mut m = Markets::new(1, &Goods::p2());
    m.active = (0..N_GOODS).map(|g| g == AG).collect();
    m.price[AG] = libm::exp(x0);
    m.inventory[AG] = s_star;
    let mut fill = vec![[1.0; 2]; N_GOODS];
    let mut spoiled = vec![0.0; N_GOODS];
    let (mut q, mut late) = (0.0, 0.0f64);
    for t in 0..steps {
        let x = libm::log(m.price[AG]);
        q = (1.0 - eta) * q + eta * eps * x;
        m.supply[AG] = q_star * libm::exp(q);
        m.demand_int[AG] = 0.0;
        m.demand_hh[AG] = libm::exp(-e * x);
        m.store(&p, &mut spoiled);
        m.clear(Rationing::Proportional, &mut fill);
        m.reprice(&p, &[kappa; N_GOODS], 0.0);
        if t + 100 >= steps {
            let s = m.inventory[AG] / s_star - 1.0;
            late = late
                .max(x.abs())
                .max(q.abs())
                .max(s.abs())
                .max(m.expectation[AG].abs());
        }
    }
    late
}

/// ADR 0091: the storage loop's linearized region against the toy market of agriculture, points
/// at least 0.3% from the unit circle. At g_s 0.006 the market facing total demand with an
/// inelastic supply (E 0.6, eps 0.09, as the full pack's lowest) is inside at the default kappa1
/// (at g_s 0.08 the expectation's momentum put it just outside); the outside points are on the
/// -1 side.
#[test]
fn stability_with_storage_matches_simulation() {
    const X0: f64 = 1e-9;
    const STEPS: usize = 6000;
    let points: [(f64, f64, f64, f64, bool); 8] = [
        (0.6, 0.3, 2.61, 0.05, true),
        (8.68, 0.3, 1.5, 0.05, true),
        (4.0, 1.0, 1.5, 0.1, true),
        (2.0, 0.3, 1.0, 0.02, true),
        (0.6, 0.3, 0.09, 0.05, true),
        (8.68, 1.0, 2.0, 0.15, false),
        (8.68, 0.3, 1.5, 0.2, false),
        (0.6, 1.0, 0.2, 2.5, false),
    ];
    for (e, eta, eps, kappa, inside) in points {
        let r = storage_radius(e, eta, eps, kappa);
        assert_eq!(
            r < 1.0,
            inside,
            "E {e} eta {eta} eps {eps} kappa {kappa}: radius {r}"
        );
        assert!((r - 1.0).abs() > 3e-3, "radius {r} too near 1");
        let late = simulate_storage(e, eta, eps, kappa, X0, STEPS);
        if inside {
            assert!(late < X0, "E {e} eps {eps} kappa {kappa} inside: {late:e}");
        } else {
            assert!(
                late > 10.0 * X0,
                "E {e} eps {eps} kappa {kappa} outside: {late:e}"
            );
        }
    }
}

/// One storable good's origin market at the approved storage values, run `steps` ticks through
/// `store`, `clear` and `reprice` from the state (x, q_{t-1}, sigma, pi) = `v` around the steady
/// state, as [`simulate_storage`] does for agriculture; returns the state after the last tick.
fn toy_state(
    g: usize,
    e: f64,
    eta: f64,
    eps: f64,
    kappa: f64,
    v: [f64; 4],
    steps: usize,
) -> [f64; 4] {
    let p = TradeParams::default().with_storage();
    let s_star = p.coverage[g];
    let q_star = 1.0 + p.storage_loss[g] * s_star;
    let mut m = Markets::new(1, &Goods::p2());
    m.active = (0..N_GOODS).map(|k| k == g).collect();
    m.price[g] = libm::exp(v[0]);
    m.inventory[g] = s_star * (1.0 + v[2]);
    m.expectation[g] = v[3];
    let mut fill = vec![[1.0; 2]; N_GOODS];
    let mut spoiled = vec![0.0; N_GOODS];
    let mut q = v[1];
    for _ in 0..steps {
        let x = libm::log(m.price[g]);
        q = (1.0 - eta) * q + eta * eps * x;
        m.supply[g] = q_star * libm::exp(q);
        m.demand_int[g] = 0.0;
        m.demand_hh[g] = libm::exp(-e * x);
        m.store(&p, &mut spoiled);
        m.clear(Rationing::Proportional, &mut fill);
        m.reprice(&p, &[kappa; N_GOODS], 0.0);
    }
    [
        libm::log(m.price[g]),
        q,
        m.inventory[g] / s_star - 1.0,
        m.expectation[g],
    ]
}

/// ADR 0280: the partial-adjustment order has ADR 0091's linearisation at the steady state, so
/// the Storage loop's map (and with it J's storage factor, the psi limit and the block map) is
/// unchanged. For each stored good the toy market's tangent map over 1 and 5 ticks, by central
/// differences, equals [`storage_matrix`] and its fifth power to 1e-7 (differencing at 1e-6).
#[test]
fn partial_adjustment_keeps_the_storage_loop() {
    const H: f64 = 1e-6;
    let p = TradeParams::default().with_storage();
    for g in [0, 1, 2, 5] {
        let st = Storage {
            delta: p.storage_loss[g],
            cost: p.storage_cost[g],
            zeta: p.convenience[g],
            psi: p.speculation[g],
            gain: p.storage_gain,
        };
        for (e, eta, eps, kappa) in [(0.6, 0.3, 2.61, 0.05), (8.68, 1.0, 1.5, 0.03)] {
            let m = storage_matrix(e, eta, eps, p.coverage[g], kappa, st);
            let mut power = m;
            for steps in [1, 5] {
                if steps == 5 {
                    for _ in 1..5 {
                        power = std::array::from_fn(|i| {
                            std::array::from_fn(|j| (0..4).map(|k| power[i][k] * m[k][j]).sum())
                        });
                    }
                }
                for j in 0..4 {
                    let mut v = [0.0; 4];
                    v[j] = H;
                    let up = toy_state(g, e, eta, eps, kappa, v, steps);
                    v[j] = -H;
                    let down = toy_state(g, e, eta, eps, kappa, v, steps);
                    for i in 0..4 {
                        let tangent = (up[i] - down[i]) / (2.0 * H);
                        assert!(
                            (tangent - power[i][j]).abs() < 1e-7,
                            "good {g} E {e} eta {eta} ticks {steps} [{i}][{j}]: {tangent} map {}",
                            power[i][j]
                        );
                    }
                }
            }
        }
    }
}
