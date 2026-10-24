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
    /// D = exp(-E x), clearing and the price rule as built. Returns the largest of |x|, |q|
    /// and |S / S*_eq - 1| over the last 100 ticks.
    fn simulate(&self, k1: f64, k2: f64, x0: f64, steps: usize) -> f64 {
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
        let (mut q, mut late) = (0.0, 0.0f64);
        for t in 0..steps {
            let x = libm::log(m.price[G]);
            q = (1.0 - self.eta) * q + self.eta * self.eps * x;
            m.supply[G] = libm::exp(q);
            m.demand_int[G] = 0.0;
            m.demand_hh[G] = libm::exp(-self.e * x);
            m.clear(Rationing::Proportional, &mut fill);
            m.reprice(&p, 0.0);
            if t + 100 >= steps {
                let s = m.inventory[G] / self.cover - 1.0;
                late = late.max(x.abs()).max(q.abs()).max(s.abs());
            }
        }
        late
    }
}

/// A small origin among many (E = theta + 1 at theta 12.55) and one facing total household
/// demand (E = sigma = 0.6) with an elastic supply, both at the default employment speed.
const SMALL_ORIGIN: Toy = Toy {
    e: 13.55,
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
    let pairs: [(&Toy, f64, f64, bool); 20] = [
        (&SMALL_ORIGIN, 0.01, 0.005, true),
        (&SMALL_ORIGIN, 0.02, 0.01, true),
        (&SMALL_ORIGIN, 0.05, 0.02, true),
        (&SMALL_ORIGIN, 0.1, 0.01, true),
        (&SMALL_ORIGIN, 0.02, 0.06, true),
        (&SMALL_ORIGIN, 0.02, -0.01, false),
        (&SMALL_ORIGIN, 0.16, 0.005, false),
        (&SMALL_ORIGIN, 0.2, 0.02, false),
        (&SMALL_ORIGIN, 0.1, 0.06, false),
        (&SMALL_ORIGIN, 0.02, 0.12, false),
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
        let late = toy.simulate(k1, k2, X0, STEPS);
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
