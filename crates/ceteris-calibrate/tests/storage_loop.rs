//! ADR 0091: the storage loop's linearized region (trade SPEC, Storage loop), the 4 x 4 map of
//! `ceteris_calibrate::stability::storage_matrix`, against the toy market of agriculture run
//! through `Markets::store`, `clear` and `reprice`.

use ceteris_calibrate::stability::{Storage, storage_matrix};
use ceteris_trade::{Markets, N_GOODS, Rationing, TradeParams};

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
    let mut m = Markets::new(1);
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
