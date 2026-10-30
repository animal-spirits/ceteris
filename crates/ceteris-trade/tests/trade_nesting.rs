//! Two-level versus direct kernel (`specs/trade/SPEC.md`, Invariants): on a planar world,
//! destination spending sourced through gateways is compared with sourcing on direct
//! province-to-province leg costs. Reported, not asserted; both must spend E exactly.

use ceteris_trade::goods::N_GOODS;
use ceteris_trade::{Kernel, Leg, Network, Sourced, TradeParams, Wedges, dispatch, source};

const W: [f64; N_GOODS] = [0.5, 0.8, 0.9, 0.3, 0.002, 0.004, 0.001, 0.0, 0.0, 0.0];
const GOOD: usize = 4;
const PER_COUNTRY: usize = 10;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

type Point = (f64, f64);

fn dist(a: Point, b: Point) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Cost of a straight-line leg: freight and time both proportional to distance.
fn leg(from: usize, to: usize, d: f64) -> Leg {
    Leg {
        from: from as u32,
        to: to as u32,
        cost_per_t: 400.0 * d,
        hours: 1500.0 * d,
    }
}

struct World {
    net: Network,
    wedges: Wedges,
    provinces: Vec<Point>,
}

/// Country 0 fills x < 0.5, country 1 x > 0.5. Each country has two gateways at its side of
/// the border, at y = 0.25 and y = 0.75, each with half its country's mass.
fn planar_world(seed: u64) -> World {
    let mut rng = Rng(seed);
    let mut provinces = vec![];
    let mut net = Network::default();
    for c in 0..2u16 {
        for _ in 0..PER_COUNTRY {
            let x = 0.5 * rng.next();
            provinces.push((if c == 0 { x } else { 1.0 - x }, rng.next()));
            net.province_country.push(c);
        }
    }
    let gateways: Vec<Point> = vec![(0.48, 0.25), (0.48, 0.75), (0.52, 0.25), (0.52, 0.75)];
    net.gateway_country = vec![0, 0, 1, 1];
    net.gateway_mass = vec![0.5; 4];
    for (o, &po) in provinces.iter().enumerate() {
        for (d, &pd) in provinces.iter().enumerate() {
            if o != d && net.province_country[o] == net.province_country[d] {
                net.domestic.push(leg(o, d, dist(po, pd)));
            }
        }
        for (a, &pa) in gateways.iter().enumerate() {
            if net.province_country[o] == net.gateway_country[a] {
                net.access.push(leg(o, a, dist(po, pa)));
            }
        }
    }
    for (a, &pa) in gateways.iter().enumerate() {
        for (b, &pb) in gateways.iter().enumerate() {
            if net.gateway_country[a] != net.gateway_country[b] {
                net.crossings.push(leg(a, b, dist(pa, pb)));
            }
        }
    }
    let mut wedges = Wedges::none(2);
    // manufactured group: country 1 imports from 0 at 10%, country 0 from 1 at 10%
    wedges.tariff[2 * 2 + 1] = 0.1;
    wedges.tariff[2 + 1] = 0.1;
    World {
        net,
        wedges,
        provinces,
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

#[test]
fn two_level_vs_direct_report() {
    let p = TradeParams::default();
    let (nu, theta) = (1.0 + p.border_nu_of(GOOD), p.theta[GOOD]);
    let mut diffs_orders = vec![];
    let mut diffs_arrival = vec![];

    for seed in 1..=8u64 {
        let w = planar_world(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let n = w.provinces.len();
        let mut rng = Rng(seed);
        let spend: Vec<[f64; 2]> = (0..n)
            .map(|_| [10.0 + 90.0 * rng.next(), 10.0 + 90.0 * rng.next()])
            .collect();
        let price: Vec<f64> = (0..n).map(|_| 0.8 + 0.4 * rng.next()).collect();
        let active = vec![true; n];

        let kernel = Kernel::build(&w.net, &W, &w.wedges, &p);
        let mut gate = Sourced::new();
        source(
            &kernel,
            GOOD,
            &price,
            &active,
            &vec![1.0; n],
            &spend,
            &mut gate,
        );
        dispatch(&kernel, GOOD, &price, &vec![[1.0; 2]; n], &mut gate);

        // gateway kernel: every destination spends E, and goods survive every loss but for
        // selection (ADR 0200)
        for (d, e) in spend.iter().enumerate() {
            let rel = (gate.paid[d] - (e[0] + e[1])).abs() / (e[0] + e[1]);
            assert!(rel < 1e-12, "gateway kernel destination {d}: {rel}");
        }
        let shipped: f64 = gate.shipped.iter().sum();
        let arrived: f64 = gate.arrive.iter().flatten().flatten().sum();
        assert!((shipped - gate.lost + gate.selected - arrived).abs() < 1e-12 * shipped);

        // direct kernel: the cross-border pair gets its own straight-line leg
        let wt: Vec<f64> = price.iter().map(|&q| q.powf(-theta)).collect();
        let mut direct_orders = vec![0.0; n];
        let mut direct_arrival = vec![0.0; n];
        for d in 0..n {
            let route_tau = |o: usize| {
                let crossing = w.net.province_country[o] != w.net.province_country[d];
                let l = leg(o, d, dist(w.provinces[o], w.provinces[d]));
                let tau = 1.0 + l.cost_per_t * W[GOOD] + p.time_cost[GOOD] * l.hours / 24.0;
                if crossing { tau * nu } else { tau }
            };
            let tariff = |o: usize| {
                let (co, cd) = (w.net.province_country[o], w.net.province_country[d]);
                if co == cd {
                    0.0
                } else {
                    f64::from(w.wedges.rate(cd as usize, co as usize, 1))
                }
            };
            let weight = |o: usize| {
                if o == d {
                    wt[o]
                } else {
                    (route_tau(o) * (1.0 + tariff(o))).powf(-theta) * wt[o]
                }
            };
            let phi: f64 = (0..n).map(weight).sum();
            let e = spend[d][0] + spend[d][1];
            let mut spent = 0.0;
            for o in 0..n {
                let x = e * weight(o) / phi;
                let units = x / (price[o] * (1.0 + tariff(o)));
                direct_orders[o] += units;
                spent += x;
            }
            // buyers pay the CES index, Gamma 1 (ADR 0200)
            direct_arrival[d] = e * phi.powf(1.0 / theta);
            assert!(
                (spent - e).abs() < 1e-12 * e,
                "direct kernel destination {d}"
            );
        }

        for i in 0..n {
            let gate_orders = gate.orders[i][0] + gate.orders[i][1];
            let gate_arrival: f64 = gate.arrive.iter().map(|a| a[i][0] + a[i][1]).sum();
            diffs_orders.push((gate_orders.ln() - direct_orders[i].ln()).abs());
            diffs_arrival.push((gate_arrival.ln() - direct_arrival[i].ln()).abs());
        }
    }
    println!(
        "two-level vs direct: median |ln difference| in orders per origin {:.4}, \
         in arrivals per destination {:.4} ({} cells)",
        median(diffs_orders.clone()),
        median(diffs_arrival.clone()),
        diffs_orders.len()
    );
    assert!(
        diffs_orders
            .iter()
            .chain(&diffs_arrival)
            .all(|x| x.is_finite())
    );
}
