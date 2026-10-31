//! Stability region of the stock-flow price rule in (kappa1, kappa2) for the linearized origin
//! market (`specs/trade/SPEC.md`, Stability region; ADR 0019): the Jury conditions on the
//! characteristic polynomial of the 3 x 3 transition matrix against a toy market run through
//! `Markets::clear` and `Markets::reprice`.

use ceteris_trade::{Markets, N_GOODS, Rationing, TradeParams};

/// A traded good.
const G: usize = 3;

/// One origin market near its steady state D = Q = 1, S = S* = cover: demand elasticity E,
/// employment speed eta, supply elasticity eps, inventory coverage `cover`.
struct Toy {
    e: f64,
    eta: f64,
    eps: f64,
    cover: f64,
}

impl Toy {
    /// a2, a1, a0 of det(lambda I - M) = lambda^3 + a2 lambda^2 + a1 lambda + a0, M the
    /// transition matrix of (x_t, q_{t-1}, sigma_t) (spec, Stability region).
    fn poly(&self, k1: f64, k2: f64) -> [f64; 3] {
        let Toy {
            e,
            eta,
            eps,
            cover: c,
        } = *self;
        let a2 = e * (k1 + k2) + e * k2 / c + eps * eta * (k1 + k2 / c) + eta - 3.0;
        let a1 =
            (eta - 2.0) * e * (k1 + k2) + (eta - 1.0) * e * k2 / c - eps * eta * k1 - 2.0 * eta
                + 3.0;
        let a0 = (1.0 - eta) * (e * (k1 + k2) - 1.0);
        [a2, a1, a0]
    }

    /// The Jury conditions: P(1) > 0, P(-1) < 0, |a0| < 1, 1 - a0^2 > |a1 - a0 a2|.
    fn stable(&self, k1: f64, k2: f64) -> bool {
        let [a2, a1, a0] = self.poly(k1, k2);
        1.0 + a2 + a1 + a0 > 0.0
            && -1.0 + a2 - a1 + a0 < 0.0
            && a0.abs() < 1.0
            && 1.0 - a0 * a0 > (a1 - a0 * a2).abs()
    }

    /// Inside (`Some(true)`) or outside (`Some(false)`) with its four neighbours at 10% in
    /// each coordinate on the same side; `None` within 10% of the boundary.
    fn side(&self, k1: f64, k2: f64) -> Option<bool> {
        let at = [
            (k1, k2),
            (0.9 * k1, k2),
            (1.1 * k1, k2),
            (k1, 0.9 * k2),
            (k1, 1.1 * k2),
        ];
        let inside = at.map(|(a, b)| self.stable(a, b));
        if inside.iter().all(|&s| s) {
            Some(true)
        } else if inside.iter().all(|&s| !s) {
            Some(false)
        } else {
            None
        }
    }

    /// Runs the market from a price deviation x0 for `steps` ticks: output Q = exp(q) with
    /// q_t = (1 - eta) q_{t-1} + eta eps x_t from the price of the tick, household orders
    /// D = exp(-E x), clearing and the price rule as built. Returns the largest of |x| and |q|,
    /// and that of |S / S*_eq - 1|, over the last 100 ticks.
    fn simulate(&self, k1: f64, k2: f64, x0: f64, steps: usize) -> [f64; 2] {
        let mut coverage = [0.0; N_GOODS];
        coverage[G] = self.cover;
        let p = TradeParams {
            kappa1: k1,
            kappa2: k2,
            coverage,
            ..TradeParams::default()
        };
        let mut m = Markets::new(1);
        m.active = (0..N_GOODS).map(|g| g == G).collect();
        m.price[G] = libm::exp(x0);
        m.inventory[G] = self.cover;
        let mut fill = vec![[1.0; 2]; N_GOODS];
        let (mut q, mut late) = (0.0, [0.0f64; 2]);
        for t in 0..steps {
            let x = libm::log(m.price[G]);
            q = (1.0 - self.eta) * q + self.eta * self.eps * x;
            m.supply[G] = libm::exp(q);
            m.demand_int[G] = 0.0;
            m.demand_hh[G] = libm::exp(-self.e * x);
            m.clear(Rationing::Proportional, &mut fill);
            m.reprice(&p, &[k1; N_GOODS], 0.0);
            if t + 100 >= steps {
                let s = m.inventory[G] / self.cover - 1.0;
                late = [late[0].max(x.abs()).max(q.abs()), late[1].max(s.abs())];
            }
        }
        late
    }
}

/// A small origin among many (E = theta + 1 at theta 10.57) and one facing total household
/// demand (E = sigma = 0.6) with an elastic supply, both at the default employment speed.
const SMALL_ORIGIN: Toy = Toy {
    e: 11.57,
    eta: 0.3,
    eps: 1.42,
    cover: 2.0,
};
const TOTAL_DEMAND: Toy = Toy {
    e: 0.6,
    eta: 0.3,
    eps: 2.6,
    cover: 2.0,
};

#[test]
fn stability_region_matches_simulation() {
    const X0: f64 = 1e-9;
    const STEPS: usize = 2000;
    let pairs: [(&Toy, f64, f64, bool); 21] = [
        (&SMALL_ORIGIN, 0.01, 0.005, true),
        (&SMALL_ORIGIN, 0.02, 0.01, true),
        (&SMALL_ORIGIN, 0.05, 0.02, true),
        (&SMALL_ORIGIN, 0.1, 0.01, true),
        (&SMALL_ORIGIN, 0.14, 0.005, true),
        (&SMALL_ORIGIN, 0.02, 0.06, true),
        (&SMALL_ORIGIN, 0.02, -0.01, false),
        (&SMALL_ORIGIN, 0.19, 0.005, false),
        (&SMALL_ORIGIN, 0.2, 0.02, false),
        (&SMALL_ORIGIN, 0.1, 0.1, false),
        (&SMALL_ORIGIN, 0.02, 0.2, false),
        (&TOTAL_DEMAND, 0.04, 0.005, true),
        (&TOTAL_DEMAND, 0.08, 0.02, true),
        (&TOTAL_DEMAND, 0.16, 0.04, true),
        (&TOTAL_DEMAND, 0.32, 0.1, true),
        (&TOTAL_DEMAND, 0.16, 0.2, true),
        (&TOTAL_DEMAND, 0.005, 0.01, false),
        (&TOTAL_DEMAND, 0.01, 0.03, false),
        (&TOTAL_DEMAND, 0.02, 0.06, false),
        (&TOTAL_DEMAND, 0.04, 0.2, false),
        (&TOTAL_DEMAND, 0.005, 0.4, false),
    ];
    for (toy, k1, k2, inside) in pairs {
        assert_eq!(
            toy.side(k1, k2),
            Some(inside),
            "E {} ({k1}, {k2}): not {} the region by 10%",
            toy.e,
            if inside { "inside" } else { "outside" }
        );
        let [flow, stock] = toy.simulate(k1, k2, X0, STEPS);
        let late = flow.max(stock);
        if inside {
            assert!(late < X0, "E {} ({k1}, {k2}) inside: {late:e}", toy.e);
        } else {
            assert!(late > X0, "E {} ({k1}, {k2}) outside: {late:e}", toy.e);
        }
    }
}

/// The default (kappa1 0.05, kappa2 0) sits on the region's edge kappa2 = 0: P(1) = 0, the
/// merchant stock is neutral (it keeps what a transient leaves), and every other condition
/// holds, so prices and output return to the steady state.
#[test]
fn default_lies_on_the_untargeted_stock_edge() {
    let p = TradeParams::default();
    for toy in [SMALL_ORIGIN, TOTAL_DEMAND] {
        let [a2, a1, a0] = toy.poly(p.kappa1, p.kappa2);
        assert!((1.0 + a2 + a1 + a0).abs() < 1e-12);
        assert!(-1.0 + a2 - a1 + a0 < 0.0 && a0.abs() < 1.0);
        assert!(1.0 - a0 * a0 > (a1 - a0 * a2).abs());
        assert!(toy.stable(p.kappa1, 1e-6));
    }
}

/// ADR 0263: a frictionless market beyond J2 at the default kappa1 (E 11.57 at theta 10.57, eps
/// 30.4 as São Tomé's light manufacturing: J 41.97, kappa1 J 2.10) cycles away from its steady
/// state; at its capped gain (kappa1_cell J = 2 m = 1) it is inside the region and settles. The
/// supply response is lagged by the employment step, so the toy's eps carries the loop at eta 1.
#[test]
fn capped_gain_settles_a_market_beyond_j2() {
    const X0: f64 = 1e-9;
    const STEPS: usize = 2000;
    let toy = Toy {
        e: 11.57,
        eta: 1.0,
        eps: 30.4,
        cover: 2.0,
    };
    let p = TradeParams::default();
    let j = toy.e + toy.eta * toy.eps / (2.0 - toy.eta);
    assert!(p.kappa1 * j > 2.0, "J {j}");
    // kappa2 = 0 is the region's edge (`default_lies_on_the_untargeted_stock_edge`)
    assert!(!toy.stable(p.kappa1, 1e-6));
    let [late, _] = toy.simulate(p.kappa1, p.kappa2, X0, STEPS);
    assert!(late > 1e3 * X0, "uncapped: {late:e}");

    let gain = p.price_gain(j);
    assert!((gain * j - 2.0 * p.gain_share).abs() < 1e-12);
    assert!(toy.stable(gain, 1e-6));
    // the stock keeps what the transient left (kappa2 = 0): price and output settle
    let [late, _] = toy.simulate(gain, p.kappa2, X0, STEPS);
    assert!(late < 1e-3 * X0, "capped: {late:e}");
}

/// Agriculture, competitively stored (ADR 0091).
const AG: usize = 0;

/// The storage loop of agriculture at the approved storage values (trade SPEC, Storage loop): the spectral
/// radius of the 4 x 4 transition matrix of (x_t, q_{t-1}, sigma_t, pi_t), as the growth rate
/// of its iterates over the second half of 8,000 ticks.
fn storage_radius(e: f64, eta: f64, eps: f64, kappa: f64) -> f64 {
    let p = TradeParams::default().with_storage();
    let (s, d) = (p.coverage[AG], p.storage_loss[AG]);
    let (psi, g) = (p.speculation[AG], p.storage_gain);
    let c = p.convenience[AG] * (p.storage_cost[AG] + d);
    let rho = 1.0 / (1.0 + d * s);
    let b = 1.0 - eta;
    let a = rho * e * (1.0 + psi * c) + eta * eps;
    let cs = rho * (d * s - psi * c);
    let m = [
        [1.0 - kappa * a, -kappa * b, kappa * cs, kappa * rho * psi],
        [eta * eps, b, 0.0, 0.0],
        [(e + eta * eps / rho) / s, b / (rho * s), 1.0 - d, 0.0],
        [
            -g * kappa * a,
            -g * kappa * b,
            g * kappa * cs,
            1.0 - g + g * kappa * rho * psi,
        ],
    ];
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

/// Agriculture's market at the approved storage values through a harvest shock of `shock` times
/// output held `months` ticks (exogenous during the shock, a harvest), household demand at E 0.6,
/// 48 ticks: the largest |ln p| and the lowest stock over its target.
fn harvest(shock: f64, months: usize) -> (f64, f64) {
    let p = TradeParams::default().with_storage();
    let s_star = p.coverage[AG];
    let q_star = 1.0 + p.storage_loss[AG] * s_star;
    let mut m = Markets::new(1);
    m.active = (0..N_GOODS).map(|g| g == AG).collect();
    m.inventory[AG] = s_star;
    let mut fill = vec![[1.0; 2]; N_GOODS];
    let mut spoiled = vec![0.0; N_GOODS];
    let (mut peak, mut low_stock) = (0.0f64, f64::INFINITY);
    for t in 0..48 {
        let x = libm::log(m.price[AG]);
        m.supply[AG] = q_star * if t < months { shock } else { 1.0 };
        m.demand_int[AG] = 0.0;
        m.demand_hh[AG] = libm::exp(-0.6 * x);
        m.store(&p, &mut spoiled);
        m.clear(Rationing::Proportional, &mut fill);
        m.reprice(&p, &[p.kappa1; N_GOODS], 0.0);
        peak = peak.max(libm::log(m.price[AG]).abs());
        low_stock = low_stock.min(m.inventory[AG]);
    }
    (peak, low_stock / s_star)
}

/// ADR 0091 (Deaton and Laroque 1992): on agriculture a 30% harvest cut held for a year raises
/// the price more than a 30% surplus held as long lowers it.
#[test]
fn stockout_raises_price_more_than_surplus_lowers_it() {
    let (rise, _) = harvest(0.7, 12);
    let (fall, _) = harvest(1.3, 12);
    assert!(rise > fall, "rise {rise} fall {fall}");
}

/// ADR 0091 Tests, as amended: a cut shorter than the cover moves the price by less than half
/// the flow-clearing price without stock, ln(1 / 0.7) / 0.6, Deaton and Laroque's benchmark
/// (0.10 against 0.59 at the approved values).
#[test]
fn storage_smooths_a_short_cut_below_flow_clearing() {
    let (rise, _) = harvest(0.7, 3);
    let clearing = libm::log(1.0 / 0.7) / 0.6;
    assert!(rise < 0.5 * clearing, "rise {rise} clearing {clearing}");
}

/// ADR 0091 Tests, as amended: a 12-month cut draws the stock below half its target, and the
/// convenience yield z^-zeta, unbounded as cover falls, keeps a pipeline stock above zero (0.23
/// of target at the approved values).
#[test]
fn cut_draws_the_stock_to_a_pipeline_floor() {
    let (_, low) = harvest(0.7, 12);
    assert!(low < 0.5 && low > 0.0, "lowest stock {low} of target");
}
