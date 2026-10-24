//! Destination sourcing on the two-level kernel (ADRs 0015 and 0016): destination spending
//! spread over origins, and the forward dispatch of what the origin markets fill.
//!
//! Destination d spends E_d (numeraire, tariff included) on good g over routes by
//! `X_od = E_d K_od p_o^-theta / Phi_d`, `Phi_d = sum_o K_od p_o^-theta`, so its delivered
//! price index is `P_d = Phi_d^(-1/theta)`. The kernel on a crossing carries the tariff, so X
//! is what d pays: the origin ships `X / (p_o (1 + t))` units and receives `p_o` per unit, the
//! importer's rebate account `t p_o` per unit, and d receives the shipped units over the
//! physical loss factor of the route. Domestic and home routes have t = 0.

use crate::kernel::{Access, Kernel};

/// Origins whose weight `(p_o / p_min)^-theta` is below this are not sourcing targets, and a
/// destination whose access `Phi_d` (in the same relative units) is below it buys nothing:
/// its price index is infinite. At theta = 5, markets 1e20x above the best price are left out.
/// It keeps every product of a weight with up to three f32 kernel factors a normal f64, so
/// the Markov shares stay accurate when prices span orders of magnitude.
pub const WEIGHT_FLOOR: f64 = 1e-100;

/// Buyer classes, the index into every `[f64; 2]` of this module and of `Markets::clear`.
pub const INTERMEDIATE: usize = 0;
pub const HOUSEHOLD: usize = 1;

/// Arrival slots of [`Sourced::arrive`]: slot s is delivered s ticks after dispatch. Slot 0 is
/// the home purchase (now), slot 1 domestic shipments and crossings with lag 0 (t + 1),
/// slots 2 to 4 crossings with lag 1 to 3.
pub const ARRIVAL_SLOTS: usize = 5;

/// Outputs of [`source`] and then [`dispatch`], for one good. Every field is overwritten by
/// each call and sized on the first, so a `Sourced` reused across goods and ticks does not
/// allocate. `dispatch` reads the sourcing state left by the `source` call before it.
///
/// Units: `orders`, `shipped` are origin units, `arrive` destination units after the
/// iceberg and border losses. Values are numeraire at origin prices: d pays `p_o (1 + t)` per
/// unit shipped to it, at dispatch. The tariff `t p_o` per unit goes to the importer's rebate
/// account; the rest moves d to b to a to o, so the leg value columns are what moves across
/// each leg in money terms and every gateway pays out exactly what it takes in. The home
/// purchase is in `paid`, `paid_by_class` and `received` but moves no money across legs: it is
/// `p_d` times `arrive[0][d]`.
#[derive(Clone, Debug, Default)]
pub struct Sourced {
    /// Units ordered at each origin by class, at its price `p_o`. Per province.
    pub orders: Vec<[f64; 2]>,
    /// Delivered price index `P_d = Phi_d^(-1/theta)` in price units, tariff included;
    /// `f64::INFINITY` where no active origin is reachable (`Phi_d` below [`WEIGHT_FLOOR`]).
    /// Per province.
    pub price_index: Vec<f64>,
    /// Units leaving each origin market, home purchases included: the filled orders. Per
    /// province.
    pub shipped: Vec<f64>,
    /// Units arriving at each destination by class, by [`ARRIVAL_SLOTS`] slot.
    pub arrive: [Vec<[f64; 2]>; ARRIVAL_SLOTS],
    /// What each destination pays for its filled orders, tariff and home purchases included.
    /// Per province.
    pub paid: Vec<f64>,
    /// `paid` by buyer class: with full fills this is the spending passed to `source`.
    pub paid_by_class: Vec<[f64; 2]>,
    /// What each origin market receives, `p_o` times `shipped`, net of tariffs. Per province.
    pub received: Vec<f64>,
    /// Tariff each destination pays its country's rebate account, t / (1 + t) of what it pays
    /// for imports, t of the crossing column. Per province.
    pub tariff: Vec<f64>,
    /// Money destination d pays origin o directly, both in one country. Aligned with
    /// `Kernel::good(g).domestic.entries`.
    pub domestic_value: Vec<f64>,
    /// Money destination d pays gateway b, net of tariff. Aligned with
    /// `Kernel::good(g).access.entries` read as destination province to gateway.
    pub access_in_value: Vec<f64>,
    /// Money gateway b pays gateway a, net of tariff. Aligned with
    /// `Kernel::good(g).crossings.entries`.
    pub crossing_value: Vec<f64>,
    /// Money gateway a pays origin o for everything o sent through a. Aligned with
    /// `Kernel::good(g).access.entries` read as origin province to gateway.
    pub access_out_value: Vec<f64>,
    /// Quantity destroyed by iceberg and border losses on every leg, all provinces.
    pub lost: f64,
    scratch: Scratch,
}

#[derive(Clone, Debug, Default)]
struct Scratch {
    /// Good of the last `source` call.
    good: Option<usize>,
    weight: Vec<f64>,
    access: Access,
    /// Spending per unit of access by class, `E_d / Phi_d`; 0 where unreachable.
    reach: Vec<[f64; 2]>,
    /// Backward pass, per gateway: value sent to destination gateway b, and value net of
    /// tariff arriving back at origin gateway a.
    to_dest_gateway: Vec<f64>,
    at_origin_gateway: Vec<f64>,
    /// Forward pass, per gateway: destination access `y_b = sum_d K_bd reach_d`, onward
    /// access `z_a = sum_b K_ab y_b` and the share of z_a left after tariffs.
    y: Vec<f64>,
    z: Vec<f64>,
    net_ratio: Vec<f64>,
    /// Value entering origin gateway a, and the same over `p_o tau_oa` (units at a before
    /// the tariff division).
    gross_a: Vec<f64>,
    units_a: Vec<f64>,
    /// Value entering destination gateway b, the same net of tariff, and units by lag.
    gross_b: Vec<f64>,
    net_b: Vec<f64>,
    units_b: [Vec<f64>; 4],
}

impl Sourced {
    pub fn new() -> Self {
        Self::default()
    }
}

fn zeroed(v: &mut Vec<f64>, n: usize) {
    v.clear();
    v.resize(n, 0.0);
}

fn zeroed2(v: &mut Vec<[f64; 2]>, n: usize) {
    v.clear();
    v.resize(n, [0.0; 2]);
}

/// Sourcing weights `(p_o / p_min)^-theta` of the active origins into `out` (0 for inactive
/// origins and below [`WEIGHT_FLOOR`]), and returns `p_min`, the lowest active price
/// (`f64::INFINITY` when no origin is active). Prices of inactive origins are not read.
pub fn origin_weights(k: &Kernel, price: &[f64], active: &[bool], out: &mut Vec<f64>) -> f64 {
    assert!(price.len() == k.n_provinces && active.len() == k.n_provinces);
    let mut p_min = f64::INFINITY;
    for (o, &p) in price.iter().enumerate() {
        if active[o] {
            assert!(p.is_finite() && p > 0.0, "active market {o} has price {p}");
            p_min = p_min.min(p);
        }
    }
    out.clear();
    out.extend((0..k.n_provinces).map(|o| {
        if !active[o] {
            return 0.0;
        }
        let w = libm::pow(price[o] / p_min, -k.theta);
        if w >= WEIGHT_FLOOR { w } else { 0.0 }
    }));
    p_min
}

/// Delivered price index from the relative access `phi` of [`Kernel::origin_access_into`]
/// on [`origin_weights`]: `p_min phi^(-1/theta)`, infinite below [`WEIGHT_FLOOR`].
pub fn price_index(p_min: f64, phi: f64, theta: f64) -> f64 {
    if phi >= WEIGHT_FLOOR {
        p_min * libm::pow(phi, -1.0 / theta)
    } else {
        f64::INFINITY
    }
}

/// Spreads the spending `spend[d] = [intermediate, household]` of every destination on good
/// `g` over the active origins, filling `out.orders` and `out.price_index`. `price` and
/// `active` are per province. The split is a backward Markov chain over the gateways: d sends
/// `E_d K_bd u'_b / Phi_d` to destination gateway b, b splits by `K_ab v'_a / u'_b` over
/// origin gateways (the tariff stays at the crossing), a by `K_oa w_o / v'_a` over origins.
/// A destination that reaches no active origin places no orders.
pub fn source(
    k: &Kernel,
    g: usize,
    price: &[f64],
    active: &[bool],
    spend: &[[f64; 2]],
    out: &mut Sourced,
) {
    let (np, ng) = (k.n_provinces, k.n_gateways);
    assert_eq!(spend.len(), np, "spend must have one entry per province");
    let gk = k.good(g);
    let s = &mut out.scratch;
    s.good = Some(g);
    let p_min = origin_weights(k, price, active, &mut s.weight);
    k.origin_access_into(g, &s.weight, &mut s.access);
    let (w, v, u, phi) = (&s.weight, &s.access.v, &s.access.u, &s.access.pi);

    out.price_index.clear();
    out.price_index
        .extend(phi.iter().map(|&f| price_index(p_min, f, k.theta)));
    s.reach.clear();
    s.reach.extend((0..np).map(|d| {
        if phi[d] >= WEIGHT_FLOOR {
            [spend[d][0] / phi[d], spend[d][1] / phi[d]]
        } else {
            [0.0; 2]
        }
    }));
    let reach = &s.reach;

    // Orders are accumulated as value net of tariff and turned into units at the end.
    let orders = &mut out.orders;
    zeroed2(orders, np);
    for c in 0..2 {
        // home and domestic routes
        for o in (0..np).filter(|&o| w[o] > 0.0) {
            let mut x = w[o] * reach[o][c];
            for e in gk.domestic.row(o) {
                x += f64::from(e.k) * w[o] * reach[e.col as usize][c];
            }
            orders[o][c] += x;
        }
        // destinations to their gateways
        let to_b = &mut s.to_dest_gateway;
        zeroed(to_b, ng);
        for d in (0..np).filter(|&d| reach[d][c] > 0.0) {
            for e in gk.access.row(d) {
                let b = e.col as usize;
                to_b[b] += reach[d][c] * f64::from(e.k) * u[b];
            }
        }
        // across the border, net of tariff
        let at_a = &mut s.at_origin_gateway;
        zeroed(at_a, ng);
        for a in (0..ng).filter(|&a| v[a] > 0.0) {
            for e in gk.crossings.row(a) {
                let b = e.col as usize;
                if to_b[b] == 0.0 {
                    continue;
                }
                let share = f64::from(e.k) * v[a] / u[b];
                at_a[a] += to_b[b] * share / (1.0 + f64::from(e.tariff));
            }
        }
        // origin gateways to origins
        for o in (0..np).filter(|&o| w[o] > 0.0) {
            for e in gk.access.row(o) {
                let a = e.col as usize;
                if at_a[a] == 0.0 {
                    continue;
                }
                orders[o][c] += at_a[a] * (f64::from(e.k) * w[o] / v[a]);
            }
        }
    }
    for o in (0..np).filter(|&o| w[o] > 0.0) {
        for x in &mut orders[o] {
            *x /= price[o];
        }
    }
}

/// Dispatches the filled orders of the preceding [`source`] call for the same good: origin o
/// ships `fill[o][c]` of its class-c orders along the routes they were placed on. `price` is
/// the one passed to `source`. The forward pass is the backward split read the other way:
/// origin o sends value `fill w_o K_oa z_a` to gateway a, a splits by `K_ab y_b / z_a`, b by
/// `K_bd reach_d / y_b`, with `reach_d = E_d / Phi_d`. Each leg divides the units by its
/// physical factor (iceberg, and the border loss on crossings), and a crossing divides the
/// value by its `1 + t`, so arrivals are the sum over routes of `X / (p_o (1 + t) tau_route)`.
pub fn dispatch(k: &Kernel, g: usize, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced) {
    let (np, ng) = (k.n_provinces, k.n_gateways);
    assert_eq!(
        out.scratch.good,
        Some(g),
        "dispatch must follow source for good {g}"
    );
    assert!(
        price.len() == np && fill.len() == np,
        "dispatch inputs must have one entry per province"
    );
    let gk = k.good(g);
    let Sourced {
        shipped,
        arrive,
        paid,
        paid_by_class,
        received,
        tariff,
        domestic_value,
        access_in_value,
        crossing_value,
        access_out_value,
        lost,
        scratch: s,
        ..
    } = out;
    zeroed(shipped, np);
    arrive.iter_mut().for_each(|a| zeroed2(a, np));
    zeroed(paid, np);
    zeroed2(paid_by_class, np);
    zeroed(received, np);
    zeroed(tariff, np);
    zeroed(domestic_value, gk.domestic.entries.len());
    zeroed(access_in_value, gk.access.entries.len());
    zeroed(crossing_value, gk.crossings.entries.len());
    zeroed(access_out_value, gk.access.entries.len());
    *lost = 0.0;

    let (w, reach) = (&s.weight, &s.reach);
    for c in 0..2 {
        // destination-side access sums on the spending weights of this class
        zeroed(&mut s.y, ng);
        for d in (0..np).filter(|&d| reach[d][c] > 0.0) {
            for e in gk.access.row(d) {
                s.y[e.col as usize] += f64::from(e.k) * reach[d][c];
            }
        }
        zeroed(&mut s.z, ng);
        zeroed(&mut s.net_ratio, ng);
        for a in 0..ng {
            let (mut gross, mut net) = (0.0, 0.0);
            for e in gk.crossings.row(a) {
                let x = f64::from(e.k) * s.y[e.col as usize];
                gross += x;
                net += x / (1.0 + f64::from(e.tariff));
            }
            s.z[a] = gross;
            if gross > 0.0 {
                s.net_ratio[a] = net / gross;
            }
        }

        // Origins: home, domestic and into the first gateway.
        zeroed(&mut s.gross_a, ng);
        zeroed(&mut s.units_a, ng);
        for o in 0..np {
            let f = fill[o][c];
            if w[o] == 0.0 || f == 0.0 {
                continue;
            }
            let (m, p) = (f * w[o], price[o]);
            let x = m * reach[o][c];
            if x > 0.0 {
                let units = x / p;
                arrive[0][o][c] += units;
                shipped[o] += units;
                paid[o] += x;
                paid_by_class[o][c] += x;
                received[o] += x;
            }
            let lo = gk.domestic.offsets[o] as usize;
            for (i, e) in gk.domestic.row(o).iter().enumerate() {
                let d = e.col as usize;
                let x = m * f64::from(e.k) * reach[d][c];
                if x == 0.0 {
                    continue;
                }
                let units = x / p;
                let got = units / e.tau;
                arrive[1][d][c] += got;
                *lost += units - got;
                shipped[o] += units;
                domestic_value[lo + i] += x;
                paid[d] += x;
                paid_by_class[d][c] += x;
                received[o] += x;
            }
            let lo = gk.access.offsets[o] as usize;
            for (i, e) in gk.access.row(o).iter().enumerate() {
                let a = e.col as usize;
                let x = m * f64::from(e.k) * s.z[a];
                if x == 0.0 {
                    continue;
                }
                let net = x * s.net_ratio[a];
                let units = net / p;
                access_out_value[lo + i] += net;
                received[o] += net;
                shipped[o] += units;
                *lost += units - units / e.tau;
                s.gross_a[a] += x;
                s.units_a[a] += x / (p * e.tau);
            }
        }

        // Origin gateways onto crossings; units are kept by transit lag from here on.
        zeroed(&mut s.gross_b, ng);
        zeroed(&mut s.net_b, ng);
        s.units_b.iter_mut().for_each(|v| zeroed(v, ng));
        for a in (0..ng).filter(|&a| s.gross_a[a] > 0.0) {
            let lo = gk.crossings.offsets[a] as usize;
            for (i, e) in gk.crossings.row(a).iter().enumerate() {
                let b = e.col as usize;
                let weight = f64::from(e.k) * s.y[b];
                if weight == 0.0 {
                    continue;
                }
                let share = weight / s.z[a];
                let duty = 1.0 + f64::from(e.tariff);
                let gross = s.gross_a[a] * share;
                let net = gross / duty;
                let units = s.units_a[a] * share / duty;
                let got = units / (e.tau * e.border_tau);
                crossing_value[lo + i] += net;
                *lost += units - got;
                s.units_b[usize::from(e.lag)][b] += got;
                s.gross_b[b] += gross;
                s.net_b[b] += net;
            }
        }

        // Destination gateways to destinations.
        for d in (0..np).filter(|&d| reach[d][c] > 0.0) {
            let lo = gk.access.offsets[d] as usize;
            for (i, e) in gk.access.row(d).iter().enumerate() {
                let b = e.col as usize;
                let weight = f64::from(e.k) * reach[d][c];
                if weight == 0.0 {
                    continue;
                }
                let share = weight / s.y[b];
                for (lag, at_b) in s.units_b.iter().enumerate() {
                    let units = at_b[b] * share;
                    let got = units / e.tau;
                    arrive[1 + lag][d][c] += got;
                    *lost += units - got;
                }
                let gross = s.gross_b[b] * share;
                let net = s.net_b[b] * share;
                access_in_value[lo + i] += net;
                paid[d] += gross;
                paid_by_class[d][c] += gross;
                tariff[d] += gross - net;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goods::{N_GOODS, tariff_group};
    use crate::network::{Leg, Network, Wedges};
    use crate::params::TradeParams;
    use proptest::prelude::*;

    const W: [f64; N_GOODS] = [0.5, 0.8, 0.9, 0.3, 0.002, 0.004, 0.001, 0.0, 0.0, 0.0];
    const THETA: f64 = 5.0;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn below(&mut self, n: usize) -> usize {
            ((self.next() * n as f64) as usize).min(n - 1)
        }
        fn leg(&mut self, from: u32, to: u32) -> Leg {
            Leg {
                from,
                to,
                cost_per_t: 5.0 + 400.0 * self.next(),
                hours: 1.0 + 2500.0 * self.next(),
            }
        }
    }

    /// A generator whose first draws are already spread over [0, 1) for small seeds.
    fn rng(seed: u64) -> Rng {
        let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        for _ in 0..4 {
            r.next();
        }
        r
    }

    /// Complete leg sets for the given province and gateway countries, random costs (lags 0
    /// to 3) and random tariffs.
    fn complete_world(rng: &mut Rng, provinces: Vec<u16>, gateways: Vec<u16>) -> (Network, Wedges) {
        let n_countries = usize::from(*provinces.iter().chain(&gateways).max().unwrap()) + 1;
        let mut net = Network {
            province_country: provinces,
            gateway_country: gateways,
            ..Network::default()
        };
        let (np, ng) = (net.n_provinces() as u32, net.n_gateways() as u32);
        for o in 0..np {
            for d in 0..np {
                if o != d && net.province_country[o as usize] == net.province_country[d as usize] {
                    net.domestic.push(rng.leg(o, d));
                }
            }
            for a in 0..ng {
                if net.province_country[o as usize] == net.gateway_country[a as usize] {
                    net.access.push(rng.leg(o, a));
                }
            }
        }
        for a in 0..ng {
            for b in 0..ng {
                if net.gateway_country[a as usize] != net.gateway_country[b as usize] {
                    net.crossings.push(rng.leg(a, b));
                }
            }
        }
        let mut wedges = Wedges::none(n_countries);
        for t in &mut wedges.tariff {
            *t = (0.3 * rng.next()) as f32;
        }
        (net, wedges)
    }

    /// 2 or 3 countries of 1 to 5 provinces and 1 or 2 gateways each.
    fn random_world(seed: u64) -> (Network, Wedges) {
        let mut rng = rng(seed);
        let n_countries = 2 + rng.below(2);
        let (mut provinces, mut gateways) = (vec![], vec![]);
        for c in 0..n_countries as u16 {
            provinces.extend(std::iter::repeat_n(c, 1 + rng.below(5)));
            gateways.extend(std::iter::repeat_n(c, 1 + rng.below(2)));
        }
        complete_world(&mut rng, provinces, gateways)
    }

    /// Two countries of 5 provinces; gateways 0 and 1 in country 0, gateway 2 in country 1.
    fn ten_province_world(seed: u64) -> (Network, Wedges) {
        let provinces = (0..10).map(|i| u16::from(i >= 5)).collect();
        complete_world(&mut rng(seed), provinces, vec![0, 0, 1])
    }

    fn kernel(net: &Network, wedges: &Wedges) -> Kernel {
        Kernel::build(net, &W, wedges, &TradeParams::default())
    }

    fn tau_leg(g: usize, l: &Leg) -> f64 {
        1.0 + l.cost_per_t * W[g] + TradeParams::default().time_cost[g] * l.hours / 24.0
    }

    /// K of a leg by the spec formula, stored as f32 like the kernel.
    fn k_leg(g: usize, l: &Leg, extra: f64) -> f64 {
        f64::powf(tau_leg(g, l) * extra, -THETA) as f32 as f64
    }

    fn find(legs: &[Leg], from: usize, to: usize) -> &Leg {
        legs.iter()
            .find(|l| (l.from as usize, l.to as usize) == (from, to))
            .unwrap()
    }

    /// One route o to d: kernel factor, physical loss factor, arrival slot and tariff.
    struct Hop {
        k: f64,
        tau: f64,
        slot: usize,
        tariff: f64,
    }

    /// Every route o to d, by explicit enumeration of gateway pairs.
    fn hops(net: &Network, wedges: &Wedges, g: usize, o: usize, d: usize) -> Vec<Hop> {
        let nu = 1.0 + TradeParams::default().border_nu;
        let (co, cd) = (net.province_country[o], net.province_country[d]);
        if o == d {
            return vec![Hop {
                k: 1.0,
                tau: 1.0,
                slot: 0,
                tariff: 0.0,
            }];
        }
        if co == cd {
            let l = find(&net.domestic, o, d);
            return vec![Hop {
                k: k_leg(g, l, 1.0),
                tau: tau_leg(g, l),
                slot: 1,
                tariff: 0.0,
            }];
        }
        let mut hs = vec![];
        for a in 0..net.n_gateways() {
            for b in 0..net.n_gateways() {
                if net.gateway_country[a] != co || net.gateway_country[b] != cd {
                    continue;
                }
                let (oa, ab, bd) = (
                    find(&net.access, o, a),
                    find(&net.crossings, a, b),
                    find(&net.access, d, b),
                );
                let t = f64::from(wedges.rate(cd as usize, co as usize, tariff_group(g)));
                hs.push(Hop {
                    k: k_leg(g, oa, 1.0) * k_leg(g, ab, nu * (1.0 + t)) * k_leg(g, bd, 1.0),
                    tau: tau_leg(g, oa) * tau_leg(g, ab) * nu * tau_leg(g, bd),
                    slot: 1 + ((ab.hours / 730.0).round().min(3.0)) as usize,
                    tariff: t,
                });
            }
        }
        hs
    }

    /// Applied tariff from o's country into d's, 0 within a country.
    fn tariff_od(net: &Network, wedges: &Wedges, g: usize, o: usize, d: usize) -> f64 {
        let (co, cd) = (net.province_country[o], net.province_country[d]);
        if co == cd {
            0.0
        } else {
            f64::from(wedges.rate(cd as usize, co as usize, tariff_group(g)))
        }
    }

    #[derive(Default)]
    struct Dense {
        orders: Vec<[f64; 2]>,
        price_index: Vec<f64>,
        shipped: Vec<f64>,
        arrive: [Vec<[f64; 2]>; 5],
        paid: Vec<f64>,
        received: Vec<f64>,
        tariff: Vec<f64>,
        lost: f64,
    }

    /// ADR 0015 on a dense route table, independent of the Markov split: d spends E_d over
    /// routes by K_route p_o^-theta / Phi_d; the origin ships X / (p_o (1 + t)) units, d
    /// receives that over the physical tau, pays X and the tariff takes t / (1 + t).
    fn dense(
        net: &Network,
        wedges: &Wedges,
        g: usize,
        price: &[f64],
        active: &[bool],
        spend: &[[f64; 2]],
        fill: &[[f64; 2]],
    ) -> Dense {
        let n = net.n_provinces();
        let p_min = (0..n)
            .filter(|&o| active[o])
            .map(|o| price[o])
            .fold(f64::INFINITY, f64::min);
        let w: Vec<f64> = (0..n)
            .map(|o| {
                let w = f64::powf(price[o] / p_min, -THETA);
                if active[o] && w >= WEIGHT_FLOOR {
                    w
                } else {
                    0.0
                }
            })
            .collect();
        let mut r = Dense {
            orders: vec![[0.0; 2]; n],
            price_index: vec![f64::INFINITY; n],
            shipped: vec![0.0; n],
            arrive: std::array::from_fn(|_| vec![[0.0; 2]; n]),
            paid: vec![0.0; n],
            received: vec![0.0; n],
            tariff: vec![0.0; n],
            lost: 0.0,
        };
        for (d, sd) in spend.iter().enumerate() {
            let all: Vec<Vec<Hop>> = (0..n).map(|o| hops(net, wedges, g, o, d)).collect();
            let phi: f64 = (0..n)
                .map(|o| all[o].iter().map(|h| h.k).sum::<f64>() * w[o])
                .sum();
            if phi < WEIGHT_FLOOR {
                continue;
            }
            r.price_index[d] = p_min * f64::powf(phi, -1.0 / THETA);
            for o in (0..n).filter(|&o| w[o] > 0.0) {
                for h in &all[o] {
                    for c in 0..2 {
                        let x = sd[c] * h.k * w[o] / phi;
                        let units = x / (1.0 + h.tariff) / price[o];
                        r.orders[o][c] += units;
                        let (xf, uf) = (x * fill[o][c], units * fill[o][c]);
                        let got = uf / h.tau;
                        r.shipped[o] += uf;
                        r.arrive[h.slot][d][c] += got;
                        r.lost += uf - got;
                        r.paid[d] += xf;
                        r.received[o] += xf / (1.0 + h.tariff);
                        r.tariff[d] += xf - xf / (1.0 + h.tariff);
                    }
                }
            }
        }
        r
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * a.abs().max(b.abs()) + 1e-300
    }

    fn assert_close(what: &str, got: &[f64], want: &[f64], tol: f64) {
        assert_eq!(got.len(), want.len(), "{what} length");
        for (i, (&a, &b)) in got.iter().zip(want).enumerate() {
            assert!(close(a, b, tol), "{what}[{i}]: {a} vs {b}");
        }
    }

    fn class(v: &[[f64; 2]], c: usize) -> Vec<f64> {
        v.iter().map(|x| x[c]).collect()
    }

    fn draw(rng: &mut Rng, n: usize, lo: f64, hi: f64) -> Vec<f64> {
        (0..n).map(|_| lo + (hi - lo) * rng.next()).collect()
    }

    fn draw2(rng: &mut Rng, n: usize, lo: f64, hi: f64) -> Vec<[f64; 2]> {
        (0..n)
            .map(|_| [lo + (hi - lo) * rng.next(), lo + (hi - lo) * rng.next()])
            .collect()
    }

    fn run(
        k: &Kernel,
        g: usize,
        price: &[f64],
        active: &[bool],
        spend: &[[f64; 2]],
        fill: &[[f64; 2]],
    ) -> Sourced {
        let mut out = Sourced::new();
        source(k, g, price, active, spend, &mut out);
        dispatch(k, g, price, fill, &mut out);
        out
    }

    #[test]
    fn sourcing_spends_destination_expenditure() {
        for seed in 1..=20u64 {
            let (net, wedges) = random_world(seed);
            let n = net.n_provinces();
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed + 100);
            let price = draw(&mut rng, n, 0.3, 3.0);
            let active = vec![true; n];
            let mut out = Sourced::new();
            // one spending destination at a time: the orders it places, valued at origin
            // prices plus the tariff, are its expenditure
            for (g, d) in (0..7).flat_map(|g| (0..n).map(move |d| (g, d))) {
                let mut spend = vec![[0.0; 2]; n];
                spend[d] = [10.0 + 90.0 * rng.next(), 5.0 + 50.0 * rng.next()];
                source(&k, g, &price, &active, &spend, &mut out);
                for (c, &want) in spend[d].iter().enumerate() {
                    let spent: f64 = (0..n)
                        .map(|o| {
                            out.orders[o][c] * price[o] * (1.0 + tariff_od(&net, &wedges, g, o, d))
                        })
                        .sum();
                    assert!(
                        close(spent, want, 1e-12),
                        "seed {seed} d {d} class {c}: {spent} vs {want}"
                    );
                }
            }
            // without tariffs, orders at origin prices add up to all spending at once
            let free = Wedges::none(wedges.n_countries());
            let k = kernel(&net, &free);
            let spend = draw2(&mut rng, n, 0.0, 100.0);
            for g in [0, 4, 6] {
                source(&k, g, &price, &active, &spend, &mut out);
                for c in 0..2 {
                    let spent: f64 = (0..n).map(|o| out.orders[o][c] * price[o]).sum();
                    let want: f64 = spend.iter().map(|s| s[c]).sum();
                    assert!(close(spent, want, 1e-12), "seed {seed}: {spent} vs {want}");
                }
            }
        }
    }

    #[test]
    fn three_province_toy_matches_eaton_kortum() {
        let g = 4;
        let pair = |o: u32, d: u32, cost_per_t: f64, hours: f64| {
            [
                Leg {
                    from: o,
                    to: d,
                    cost_per_t,
                    hours,
                },
                Leg {
                    from: d,
                    to: o,
                    cost_per_t,
                    hours,
                },
            ]
        };
        let legs = [
            pair(0, 1, 60.0, 30.0),
            pair(0, 2, 150.0, 70.0),
            pair(1, 2, 90.0, 20.0),
        ];
        let net = Network {
            province_country: vec![0, 0, 0],
            domestic: legs.iter().flatten().copied().collect(),
            ..Network::default()
        };
        let k = kernel(&net, &Wedges::none(1));
        let price = [1.0, 1.2, 0.9];
        let spend = [[0.0, 10.0], [0.0, 20.0], [0.0, 30.0]];
        let mut out = Sourced::new();
        source(&k, g, &price, &[true; 3], &spend, &mut out);

        let kk = |o: usize, d: usize| {
            if o == d {
                1.0
            } else {
                k_leg(g, find(&net.domestic, o, d), 1.0)
            }
        };
        for o in 0..3 {
            let want: f64 = (0..3)
                .map(|d| {
                    let share = kk(o, d) * price[o].powf(-THETA)
                        / (0..3)
                            .map(|j| kk(j, d) * price[j].powf(-THETA))
                            .sum::<f64>();
                    share * spend[d][1] / price[o]
                })
                .sum();
            assert!(
                close(out.orders[o][1], want, 1e-12),
                "origin {o}: {} vs {want}",
                out.orders[o][1]
            );
            assert_eq!(out.orders[o][0], 0.0);
        }
    }

    #[test]
    fn sourcing_matches_dense_kernel() {
        for seed in 1..=6u64 {
            let (net, wedges) = ten_province_world(seed * 0x9e37_79b9);
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed);
            for g in [0usize, 3, 4, 6] {
                let price = draw(&mut rng, 10, 0.5, 2.0);
                let mut active = vec![true; 10];
                active[rng.below(10)] = false;
                active[rng.below(10)] = false;
                let spend = draw2(&mut rng, 10, 0.0, 100.0);
                let fill = vec![[1.0; 2]; 10];
                let mut out = Sourced::new();
                source(&k, g, &price, &active, &spend, &mut out);
                let want = dense(&net, &wedges, g, &price, &active, &spend, &fill);
                for c in 0..2 {
                    assert_close(
                        &format!("seed {seed} g {g} orders class {c}"),
                        &class(&out.orders, c),
                        &class(&want.orders, c),
                        1e-10,
                    );
                }
            }
        }
    }

    #[test]
    fn dispatch_matches_dense_kernel() {
        for seed in 1..=6u64 {
            let (net, wedges) = ten_province_world(seed * 0x51_7cc1);
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed + 7);
            for g in [0usize, 2, 5] {
                let price = draw(&mut rng, 10, 0.5, 2.0);
                let mut active = vec![true; 10];
                active[rng.below(10)] = false;
                let spend = draw2(&mut rng, 10, 0.0, 100.0);
                let fill = draw2(&mut rng, 10, 0.0, 1.0);
                let out = run(&k, g, &price, &active, &spend, &fill);
                let want = dense(&net, &wedges, g, &price, &active, &spend, &fill);
                let tag = |what: &str| format!("seed {seed} g {g} {what}");
                for s in 0..5 {
                    for c in 0..2 {
                        assert_close(
                            &tag(&format!("arrive[{s}] class {c}")),
                            &class(&out.arrive[s], c),
                            &class(&want.arrive[s], c),
                            1e-10,
                        );
                    }
                }
                assert_close(&tag("shipped"), &out.shipped, &want.shipped, 1e-10);
                assert_close(&tag("paid"), &out.paid, &want.paid, 1e-10);
                assert_close(&tag("received"), &out.received, &want.received, 1e-10);
                assert_close(&tag("tariff"), &out.tariff, &want.tariff, 1e-10);
                assert_close(&tag("lost"), &[out.lost], &[want.lost], 1e-10);
            }
        }
    }

    #[test]
    fn price_index_is_ces_of_delivered_prices() {
        for seed in 1..=6u64 {
            let (net, wedges) = ten_province_world(seed * 0x2545_f491);
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed + 31);
            for g in [1usize, 4] {
                let price = draw(&mut rng, 10, 0.5, 2.0);
                let mut active = vec![true; 10];
                active[rng.below(10)] = false;
                let mut out = Sourced::new();
                source(&k, g, &price, &active, &[[1.0; 2]; 10], &mut out);
                for d in 0..10 {
                    let want: f64 = (0..10)
                        .filter(|&o| active[o])
                        .map(|o| {
                            let k_od: f64 = hops(&net, &wedges, g, o, d).iter().map(|h| h.k).sum();
                            k_od * price[o].powf(-THETA)
                        })
                        .sum();
                    let got = out.price_index[d].powf(-THETA);
                    assert!(
                        close(got, want, 1e-12),
                        "seed {seed} g {g} d {d}: {got} vs {want}"
                    );
                }
            }
        }
    }

    #[test]
    fn shares_homogeneous_in_prices() {
        for seed in [3u64, 17, 99] {
            let (net, wedges) = random_world(seed);
            let n = net.n_provinces();
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed);
            let price = draw(&mut rng, n, 0.5, 1.5);
            let spend = draw2(&mut rng, n, 1.0, 100.0);
            let active = vec![true; n];
            let shares = |out: &Sourced, p: &[f64]| -> Vec<f64> {
                (0..2)
                    .flat_map(|c| {
                        let value: Vec<f64> = (0..n).map(|o| out.orders[o][c] * p[o]).collect();
                        let total: f64 = value.iter().sum();
                        value.into_iter().map(move |v| v / total)
                    })
                    .collect()
            };
            let mut base = Sourced::new();
            source(&k, 4, &price, &active, &spend, &mut base);
            for lambda in [1e-6, 1e3] {
                let scaled: Vec<f64> = price.iter().map(|p| p * lambda).collect();
                let mut out = Sourced::new();
                source(&k, 4, &scaled, &active, &spend, &mut out);
                assert_close(
                    &format!("shares at lambda {lambda}"),
                    &shares(&out, &scaled),
                    &shares(&base, &price),
                    1e-12,
                );
                let index: Vec<f64> = base.price_index.iter().map(|p| p * lambda).collect();
                assert_close("price index", &out.price_index, &index, 1e-12);
                // prices and spending scaled together: the same units are ordered
                let spend_l: Vec<[f64; 2]> = spend
                    .iter()
                    .map(|s| [s[0] * lambda, s[1] * lambda])
                    .collect();
                source(&k, 4, &scaled, &active, &spend_l, &mut out);
                for c in 0..2 {
                    assert_close(
                        "units",
                        &class(&out.orders, c),
                        &class(&base.orders, c),
                        1e-12,
                    );
                }
            }
        }
    }

    #[test]
    fn fills_scale_shipments_by_class() {
        let (net, wedges) = random_world(42);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = rng(42);
        let price = draw(&mut rng, n, 0.5, 1.5);
        let spend = draw2(&mut rng, n, 1.0, 100.0);
        let active = vec![true; n];
        let full = run(&k, 6, &price, &active, &spend, &vec![[1.0; 2]; n]);

        // origin by origin: shipments are the filled orders of both classes
        let fill = draw2(&mut rng, n, 0.0, 1.0);
        let out = run(&k, 6, &price, &active, &spend, &fill);
        for o in 0..n {
            let want = fill[o][0] * out.orders[o][0] + fill[o][1] * out.orders[o][1];
            assert!(close(out.shipped[o], want, 1e-12), "origin {o}");
            assert!(close(out.received[o], price[o] * out.shipped[o], 1e-12));
        }

        // one fill per class everywhere: every arrival and payment of that class scales by it
        let (fi, fh) = (0.3, 0.7);
        let out = run(&k, 6, &price, &active, &spend, &vec![[fi, fh]; n]);
        for s in 0..5 {
            for d in 0..n {
                assert!(close(out.arrive[s][d][0], fi * full.arrive[s][d][0], 1e-12));
                assert!(close(out.arrive[s][d][1], fh * full.arrive[s][d][1], 1e-12));
            }
        }
        for d in 0..n {
            let want = [fi * full.paid_by_class[d][0], fh * full.paid_by_class[d][1]];
            assert!(close(out.paid_by_class[d][0], want[0], 1e-12));
            assert!(close(out.paid_by_class[d][1], want[1], 1e-12));
            assert!(close(out.paid[d], want[0] + want[1], 1e-12));
        }

        // households unfilled: nothing of theirs moves
        let out = run(&k, 6, &price, &active, &spend, &vec![[1.0, 0.0]; n]);
        assert!(out.arrive.iter().flatten().all(|a| a[1] == 0.0));
        assert!(out.paid_by_class.iter().all(|p| p[1] == 0.0));
        // with full fill every destination pays exactly what it spends
        for (paid, want) in full
            .paid_by_class
            .iter()
            .flatten()
            .zip(spend.iter().flatten())
        {
            assert!(close(*paid, *want, 1e-12), "{paid} vs {want}");
        }
    }

    #[test]
    fn payments_net_at_gateways() {
        for seed in 1..=10u64 {
            let (net, wedges) = random_world(seed * 7919);
            let n = net.n_provinces();
            let k = kernel(&net, &wedges);
            let mut rng = rng(seed);
            let g = [0, 4, 5, 6][seed as usize % 4];
            let price = draw(&mut rng, n, 0.3, 3.0);
            let spend = draw2(&mut rng, n, 0.0, 100.0);
            let fill = draw2(&mut rng, n, 0.0, 1.0);
            let out = run(&k, g, &price, &vec![true; n], &spend, &fill);

            let gk = k.good(g);
            let (mut m_in, mut m_out) = (vec![0.0; k.n_gateways], vec![0.0; k.n_gateways]);
            let mut paid: Vec<f64> = out.tariff.clone();
            let mut received = vec![0.0; n];
            for p in 0..n {
                // the home purchase moves no money across legs
                let home = price[p] * (out.arrive[0][p][0] + out.arrive[0][p][1]);
                paid[p] += home;
                received[p] += home;
                let lo = gk.domestic.offsets[p] as usize;
                for (i, e) in gk.domestic.row(p).iter().enumerate() {
                    received[p] += out.domestic_value[lo + i];
                    paid[e.col as usize] += out.domestic_value[lo + i];
                }
                let lo = gk.access.offsets[p] as usize;
                for (i, e) in gk.access.row(p).iter().enumerate() {
                    m_in[e.col as usize] += out.access_in_value[lo + i];
                    m_out[e.col as usize] += out.access_out_value[lo + i];
                    received[p] += out.access_out_value[lo + i];
                    paid[p] += out.access_in_value[lo + i];
                }
            }
            let mut due = 0.0;
            for (a, m_in_a) in m_in.iter_mut().enumerate() {
                let lo = gk.crossings.offsets[a] as usize;
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    *m_in_a += out.crossing_value[lo + i];
                    m_out[e.col as usize] += out.crossing_value[lo + i];
                    due += f64::from(e.tariff) * out.crossing_value[lo + i];
                }
            }
            for a in 0..k.n_gateways {
                assert!(close(m_in[a], m_out[a], 1e-12), "gateway {a}");
            }
            assert_close("paid", &paid, &out.paid, 1e-12);
            assert_close("received", &received, &out.received, 1e-12);
            let tariff: f64 = out.tariff.iter().sum();
            assert!(close(due, tariff, 1e-12), "{due} vs {tariff}");
            let total = |v: &[f64]| v.iter().sum::<f64>();
            assert!(close(
                total(&out.paid),
                total(&out.received) + tariff,
                1e-12
            ));
        }
    }

    #[test]
    fn unreachable_destination_buys_nothing() {
        // country 0: province 0 produces at 1; country 1: province 1 produces at 1e18 and
        // province 2 has no market and a costly road to 1; country 2: province 3 is an island
        // with no gateway and no producer. No gateways anywhere, so countries are disjoint.
        let road = |from, to| Leg {
            from,
            to,
            cost_per_t: 800.0,
            hours: 10.0,
        };
        let net = Network {
            province_country: vec![0, 1, 1, 2],
            domestic: vec![road(1, 2), road(2, 1)],
            ..Network::default()
        };
        let k = kernel(&net, &Wedges::none(3));
        let g = 0;
        let price = [1.0, 1e18, f64::NAN, f64::NAN];
        let active = [true, true, false, false];
        let spend = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0], [7.0, 8.0]];
        let out = run(&k, g, &price, &active, &spend, &[[1.0; 2]; 4]);

        assert_eq!(out.price_index[0], 1.0);
        assert!(close(out.price_index[1], 1e18, 1e-12));
        // province 2 reaches only province 1, whose weight 1e-90 times K ~ 1e-13 is below the
        // floor; province 3 reaches nobody
        assert_eq!(out.price_index[2], f64::INFINITY);
        assert_eq!(out.price_index[3], f64::INFINITY);
        assert!(close(out.orders[0][0] * 1.0, 1.0, 1e-12));
        assert!(close(out.orders[1][1] * 1e18, 4.0, 1e-12));
        assert_eq!(out.orders[2], [0.0; 2]);
        assert_eq!(out.orders[3], [0.0; 2]);
        for d in [2, 3] {
            assert_eq!(out.paid[d], 0.0);
            assert_eq!(out.paid_by_class[d], [0.0; 2]);
            assert!(out.arrive.iter().all(|a| a[d] == [0.0; 2]));
        }
        let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
        assert!(finite(&out.shipped) && finite(&out.paid) && finite(&out.received));
        assert!(out.orders.iter().flatten().all(|x| x.is_finite()));
        assert!(out.lost.is_finite());

        // no active origin at all: every index is infinite and nothing moves
        let out = run(&k, g, &price, &[false; 4], &spend, &[[1.0; 2]; 4]);
        assert!(out.price_index.iter().all(|&p| p == f64::INFINITY));
        assert!(out.orders.iter().flatten().all(|&x| x == 0.0));
        assert!(out.shipped.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn inactive_prices_are_never_read() {
        let (net, wedges) = random_world(5);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = rng(5);
        let mut price = draw(&mut rng, n, 0.5, 1.5);
        let spend = draw2(&mut rng, n, 1.0, 100.0);
        let fill = draw2(&mut rng, n, 0.0, 1.0);
        let mut active = vec![true; n];
        active[0] = false;
        let a = run(&k, 2, &price, &active, &spend, &fill);
        price[0] = f64::NAN;
        let b = run(&k, 2, &price, &active, &spend, &fill);
        assert_eq!(a.orders, b.orders);
        assert_eq!(a.arrive, b.arrive);
        assert_eq!(a.paid, b.paid);
        assert_eq!(a.orders[0], [0.0; 2]);
        assert_eq!(a.shipped[0], 0.0);
    }

    #[test]
    fn reuse_overwrites_and_does_not_reallocate() {
        let (net, wedges) = random_world(11);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = rng(8);
        let active = vec![true; n];
        let mut reused = Sourced::new();
        let p = draw(&mut rng, n, 0.5, 1.5);
        let s = draw2(&mut rng, n, 0.0, 100.0);
        source(&k, 5, &p, &active, &s, &mut reused);
        dispatch(&k, 5, &p, &vec![[1.0; 2]; n], &mut reused);
        let ptrs = (reused.orders.as_ptr(), reused.arrive[3].as_ptr());
        for g in [0, 3, 6] {
            let p = draw(&mut rng, n, 0.5, 1.5);
            let s = draw2(&mut rng, n, 0.0, 100.0);
            let f = draw2(&mut rng, n, 0.0, 1.0);
            source(&k, g, &p, &active, &s, &mut reused);
            dispatch(&k, g, &p, &f, &mut reused);
            let fresh = run(&k, g, &p, &active, &s, &f);
            assert_eq!(reused.orders, fresh.orders);
            assert_eq!(reused.arrive, fresh.arrive);
            assert_eq!(reused.crossing_value, fresh.crossing_value);
            assert_eq!(reused.lost.to_bits(), fresh.lost.to_bits());
        }
        assert_eq!(ptrs, (reused.orders.as_ptr(), reused.arrive[3].as_ptr()));
    }

    /// seed, good, log10 price, active, spend, fill; vectors cover up to 15 provinces.
    type Case = (
        u64,
        usize,
        Vec<f64>,
        Vec<bool>,
        Vec<[f64; 2]>,
        Vec<[f64; 2]>,
    );

    fn case(log_price: std::ops::Range<f64>) -> impl Strategy<Value = Case> {
        (
            any::<u64>(),
            0..7usize,
            prop::collection::vec(log_price, 15),
            prop::collection::vec(prop::bool::weighted(0.8), 15),
            prop::collection::vec([0.0..1000.0f64, 0.0..1000.0f64], 15),
            prop::collection::vec([0.0..=1.0f64, 0.0..=1.0f64], 15),
        )
    }

    fn setup(
        (seed, g, log_price, active, spend, fill): Case,
    ) -> (Kernel, usize, Vec<f64>, Sourced) {
        let (net, wedges) = random_world(seed);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let price: Vec<f64> = log_price[..n].iter().map(|&e| 10f64.powf(e)).collect();
        let out = run(&k, g, &price, &active[..n], &spend[..n], &fill[..n]);
        (k, n, price, out)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn forward_pass_conserves_goods(c in case(-2.0..2.0)) {
            let (_, _, price, out) = setup(c);
            let shipped: f64 = out.shipped.iter().sum();
            let arrived: f64 = out.arrive.iter().flatten().flatten().sum();
            prop_assert!(close(shipped - out.lost, arrived, 1e-12),
                "{shipped} - {} vs {arrived}", out.lost);
            for ((r, p), q) in out.received.iter().zip(&price).zip(&out.shipped) {
                prop_assert!(close(*r, p * q, 1e-12));
            }
        }

        #[test]
        fn extreme_prices_stay_finite(c in case(-80.0..80.0)) {
            let (_, n, price, out) = setup(c.clone());
            let finite = |v: &[f64]| v.iter().all(|x| x.is_finite() && *x >= 0.0);
            let finite2 = |v: &[[f64; 2]]| v.iter().flatten().all(|x| x.is_finite() && *x >= 0.0);
            prop_assert!(finite2(&out.orders));
            prop_assert!(out.price_index.iter().all(|p| *p > 0.0 && !p.is_nan()));
            prop_assert!(finite(&out.shipped) && finite(&out.paid) && finite(&out.received));
            prop_assert!(finite(&out.tariff) && finite2(&out.paid_by_class));
            prop_assert!(finite(&out.domestic_value) && finite(&out.access_in_value));
            prop_assert!(finite(&out.crossing_value) && finite(&out.access_out_value));
            prop_assert!(out.arrive.iter().all(|a| finite2(a)));
            prop_assert!(out.lost.is_finite() && out.lost >= 0.0);

            let spend = c.4;
            // no destination pays more than it spends
            for (paid, s) in out.paid_by_class.iter().zip(&spend[..n]) {
                prop_assert!(paid[0] + paid[1] <= (s[0] + s[1]) * (1.0 + 1e-12));
            }
            let shipped: f64 = out.shipped.iter().sum();
            let arrived: f64 = out.arrive.iter().flatten().flatten().sum();
            prop_assert!(close(shipped - out.lost, arrived, 1e-12));
            for ((r, p), q) in out.received.iter().zip(&price).zip(&out.shipped) {
                prop_assert!(close(*r, p * q, 1e-12));
            }
        }
    }

    #[test]
    fn extreme_prices_spend_everything_when_filled() {
        let mut rng = rng(77);
        for seed in 1..=30u64 {
            let (net, wedges) = random_world(seed * 31);
            let n = net.n_provinces();
            let k = kernel(&net, &wedges);
            let price: Vec<f64> = (0..n)
                .map(|_| 10f64.powf(-80.0 + 160.0 * rng.next()))
                .collect();
            let spend = draw2(&mut rng, n, 0.0, 1000.0);
            let out = run(
                &k,
                rng.below(7),
                &price,
                &vec![true; n],
                &spend,
                &vec![[1.0; 2]; n],
            );
            // shares add up: each reachable destination pays exactly its spending
            for d in (0..n).filter(|&d| out.price_index[d].is_finite()) {
                for (&paid, &want) in out.paid_by_class[d].iter().zip(&spend[d]) {
                    assert!(
                        close(paid, want, 1e-12),
                        "seed {seed} d {d}: {paid} vs {want}"
                    );
                }
            }
        }
    }
}
