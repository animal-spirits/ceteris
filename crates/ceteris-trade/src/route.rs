//! Merchant routing on the two-level kernel (`specs/trade/SPEC.md`, Routing, Low-rank
//! evaluation, Payments).

use crate::kernel::{Access, Kernel};

/// Outputs of one [`route`] call, for one good. Every field is overwritten by each call and
/// sized on the first, so a `Routed` reused across goods and ticks does not allocate. The
/// caller moves `arrive` into the transit wheel (`TransitWheel::push(lag, g, &arrive[lag])`)
/// before the next call.
///
/// Units: `home`, `shipped` and the `*_units` columns are origin units, `arrive` destination
/// units after the iceberg and border losses. Values are numeraire at delivered prices: each
/// route is paid the destination price times the units that arrive, at dispatch. On a
/// cross-border route the tariff takes t / (1 + t) of that payment; the rest is the money
/// that moves d to b to a to o, so the leg value columns are what moves across each leg in
/// money terms and every gateway pays out exactly what it takes in.
#[derive(Clone, Debug, Default)]
pub struct Routed {
    /// Supply that stays at the origin, delivered immediately. Per province.
    pub home: Vec<f64>,
    /// Goods due at the destination by transit lag in months, delivered at t + 1 + lag.
    /// Domestic arrivals are in lag 0. Per province.
    pub arrive: [Vec<f64>; 4],
    /// What each destination market pays: its price times the units arriving from other
    /// provinces, tariff included. Per province.
    pub paid: Vec<f64>,
    /// What each origin market receives for its shipments: the delivered value of everything
    /// it shipped, net of tariffs. Per province.
    pub received: Vec<f64>,
    /// Money destination d pays origin o directly, both in one country: the destination
    /// price times the units arriving. Aligned with `Kernel::good(g).domestic.entries`.
    pub domestic_value: Vec<f64>,
    /// Origin units sent origin to destination in one country, aligned like
    /// `domestic_value`.
    pub domestic_units: Vec<f64>,
    /// Money gateway a pays origin o: the delivered value, net of tariff, of everything o
    /// sent through a. Aligned with `Kernel::good(g).access.entries` read as origin province
    /// to gateway.
    pub access_out_value: Vec<f64>,
    /// Money gateway b pays gateway a: the delivered value, net of tariff, of the units that
    /// crossed a to b. Aligned with `Kernel::good(g).crossings.entries`.
    pub crossing_value: Vec<f64>,
    /// Origin units, at dispatch from their origins, whose route crosses a to b. Aligned like
    /// `crossing_value`.
    pub crossing_units: Vec<f64>,
    /// Money destination d pays gateway b: its price times the units arriving through b, net
    /// of tariff. Aligned with `Kernel::good(g).access.entries` read as destination province
    /// to gateway.
    pub access_in_value: Vec<f64>,
    /// Tariff each destination market pays its country's rebate account: t / (1 + t) of the
    /// delivered value of its imports, t of the crossing column. Per province.
    pub tariff: Vec<f64>,
    /// Origin units sent to other provinces. Per province.
    pub shipped: Vec<f64>,
    /// Quantity destroyed by iceberg and border losses on every leg, all provinces.
    pub lost: f64,
    /// Average netback price per origin unit over all destinations including home: the
    /// destination price net of losses and tariff, sum_d share p_d / tau / (1 + t). Equal to
    /// the origin's own price on the home route plus `received`, per unit of supply. Defined
    /// per unit, so also for zero supply. An origin with no route at all reports its own
    /// price.
    pub netback: Vec<f64>,
    scratch: Scratch,
}

#[derive(Clone, Debug, Default)]
struct Scratch {
    price_theta: Vec<f64>,
    access: Access,
    /// Delivered value per unit arriving at a destination gateway, by gateway.
    dest_value: Vec<f64>,
    /// Delivered value net of tariff per unit arriving at an origin gateway, by gateway.
    onward_value: Vec<f64>,
    /// Units arriving at an origin gateway, and the same at dispatch from the origins.
    from_origins_units: Vec<f64>,
    from_origins_dispatched: Vec<f64>,
    at_dest_units: [Vec<f64>; 4],
    /// Units arriving at a destination gateway over all lags, and the same weighted by
    /// 1 / (1 + t) of the crossing they took.
    at_dest_total: Vec<f64>,
    at_dest_net: Vec<f64>,
}

impl Routed {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Destinations whose weight `(p / p_max)^theta` is below this are not routing targets. It
/// keeps every product of the weight with up to three f32 kernel factors a normal f64, so
/// the Markov shares stay accurate when prices span orders of magnitude.
pub const WEIGHT_FLOOR: f64 = 1e-100;

fn zeroed(v: &mut Vec<f64>, n: usize) {
    v.clear();
    v.resize(n, 0.0);
}

/// Routes the supply of traded good `g` and fills `out`.
///
/// `supply`, `price` and `active` are per province. Active destinations are weighted by
/// `(p / p_max_active)^theta`, which has the routing shares of `p^theta` without overflow.
/// An origin with nothing to route to (`Pi = 0`, the weight of every reachable market
/// below the floor) keeps its whole supply at home.
pub fn route(
    k: &Kernel,
    g: usize,
    supply: &[f64],
    price: &[f64],
    active: &[bool],
    out: &mut Routed,
) {
    let (np, ng) = (k.n_provinces, k.n_gateways);
    assert!(
        supply.len() == np && price.len() == np && active.len() == np,
        "route inputs must have one entry per province"
    );
    let gk = k.good(g);
    let Routed {
        home,
        arrive,
        paid,
        received,
        domestic_value,
        domestic_units,
        access_out_value,
        crossing_value,
        crossing_units,
        access_in_value,
        tariff,
        shipped,
        lost,
        netback,
        scratch: s,
    } = out;

    zeroed(home, np);
    arrive.iter_mut().for_each(|a| zeroed(a, np));
    zeroed(paid, np);
    zeroed(received, np);
    zeroed(domestic_value, gk.domestic.entries.len());
    zeroed(domestic_units, gk.domestic.entries.len());
    zeroed(access_out_value, gk.access.entries.len());
    zeroed(crossing_value, gk.crossings.entries.len());
    zeroed(crossing_units, gk.crossings.entries.len());
    zeroed(access_in_value, gk.access.entries.len());
    zeroed(tariff, np);
    zeroed(shipped, np);
    zeroed(netback, np);
    *lost = 0.0;

    let mut p_max = 0.0f64;
    for d in 0..np {
        if active[d] {
            assert!(
                price[d].is_finite() && price[d] > 0.0,
                "active market {d} has price {}",
                price[d]
            );
            p_max = p_max.max(price[d]);
        }
    }
    if p_max == 0.0 {
        home.copy_from_slice(supply);
        netback.copy_from_slice(price);
        return;
    }
    s.price_theta.clear();
    s.price_theta.extend((0..np).map(|d| {
        let w = if active[d] {
            libm::pow(price[d] / p_max, k.theta)
        } else {
            0.0
        };
        if w >= WEIGHT_FLOOR { w } else { 0.0 }
    }));
    let pt = &s.price_theta;
    k.access_into(g, pt, &mut s.access);
    let (v, u, pi) = (&s.access.v, &s.access.u, &s.access.pi);

    // Delivered value per unit entering a destination gateway b: the posted price at the
    // destination d it ends at, net of the last leg.
    zeroed(&mut s.dest_value, ng);
    for d in 0..np {
        for e in gk.access.row(d) {
            s.dest_value[e.col as usize] += f64::from(e.k) * pt[d] * price[d] / e.tau;
        }
    }
    for (dv, &vb) in s.dest_value.iter_mut().zip(v) {
        if vb > 0.0 {
            *dv /= vb;
        }
    }
    // The same per unit leaving an origin gateway a, net of the crossing, the border loss
    // and the tariff.
    zeroed(&mut s.onward_value, ng);
    for (a, &ua) in u.iter().enumerate() {
        if ua > 0.0 {
            let sum: f64 = gk
                .crossings
                .row(a)
                .iter()
                .map(|e| {
                    f64::from(e.k) * v[e.col as usize] * s.dest_value[e.col as usize]
                        / (e.tau * e.border_tau * (1.0 + f64::from(e.tariff)))
                })
                .sum();
            s.onward_value[a] = sum / ua;
        }
    }

    // Origins: home, domestic and into the first gateway.
    zeroed(&mut s.from_origins_units, ng);
    zeroed(&mut s.from_origins_dispatched, ng);
    for o in 0..np {
        if pi[o] <= 0.0 {
            home[o] = supply[o];
            netback[o] = price[o];
            continue;
        }
        let per_unit = 1.0 / pi[o];
        let scale = supply[o] * per_unit;
        home[o] = scale * pt[o];
        let mut back = pt[o] * price[o];
        let mut sent = 0.0;
        let lo = gk.domestic.offsets[o] as usize;
        for (i, e) in gk.domestic.row(o).iter().enumerate() {
            let d = e.col as usize;
            let w = f64::from(e.k) * pt[d];
            if w == 0.0 {
                continue;
            }
            back += w * price[d] / e.tau;
            let x = scale * w;
            let got = x / e.tau;
            let value = price[d] * got;
            arrive[0][d] += got;
            domestic_value[lo + i] = value;
            domestic_units[lo + i] = x;
            paid[d] += value;
            received[o] += value;
            *lost += x - got;
            sent += x;
        }
        let lo = gk.access.offsets[o] as usize;
        for (i, e) in gk.access.row(o).iter().enumerate() {
            let a = e.col as usize;
            let w = f64::from(e.k) * u[a];
            if w == 0.0 {
                continue;
            }
            back += w * s.onward_value[a] / e.tau;
            let x = scale * w;
            let got = x / e.tau;
            let value = got * s.onward_value[a];
            access_out_value[lo + i] = value;
            s.from_origins_units[a] += got;
            s.from_origins_dispatched[a] += x;
            received[o] += value;
            *lost += x - got;
            sent += x;
        }
        shipped[o] = sent;
        netback[o] = back * per_unit;
    }

    // Origin gateways onto crossings. Units are kept by transit lag from here on; the tariff
    // only as the 1 / (1 + t) weighted total at the destination gateway.
    s.at_dest_units.iter_mut().for_each(|a| zeroed(a, ng));
    zeroed(&mut s.at_dest_total, ng);
    zeroed(&mut s.at_dest_net, ng);
    for (a, &ua) in u.iter().enumerate() {
        if s.from_origins_units[a] == 0.0 {
            continue;
        }
        let lo = gk.crossings.offsets[a] as usize;
        for (i, e) in gk.crossings.row(a).iter().enumerate() {
            let b = e.col as usize;
            let w = f64::from(e.k) * v[b];
            if w == 0.0 {
                continue;
            }
            let share = w / ua;
            let units = s.from_origins_units[a] * share;
            let got = units / (e.tau * e.border_tau);
            let net = got / (1.0 + f64::from(e.tariff));
            crossing_value[lo + i] = net * s.dest_value[b];
            crossing_units[lo + i] = s.from_origins_dispatched[a] * share;
            s.at_dest_units[usize::from(e.lag)][b] += got;
            s.at_dest_total[b] += got;
            s.at_dest_net[b] += net;
            *lost += units - got;
        }
    }

    // Destination gateways to destinations.
    for d in 0..np {
        let lo = gk.access.offsets[d] as usize;
        for (i, e) in gk.access.row(d).iter().enumerate() {
            let b = e.col as usize;
            let w = f64::from(e.k) * pt[d];
            if w == 0.0 || s.at_dest_total[b] == 0.0 {
                continue;
            }
            let share = w / v[b];
            let mut got_all = 0.0;
            for (at_dest, arrived) in s.at_dest_units.iter().zip(arrive.iter_mut()) {
                let units = at_dest[b] * share;
                let got = units / e.tau;
                arrived[d] += got;
                got_all += got;
                *lost += units - got;
            }
            let delivered = price[d] * got_all;
            let net = delivered * (s.at_dest_net[b] / s.at_dest_total[b]);
            access_in_value[lo + i] = net;
            paid[d] += delivered;
            tariff[d] += delivered - net;
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
    const FLOOR: f64 = 1e-100;

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

    /// 2 or 3 countries of 1 to 5 provinces and 1 or 2 gateways each, complete leg sets,
    /// random costs (lags from 0 to 3) and random tariffs.
    fn random_world(seed: u64) -> (Network, Wedges) {
        let mut rng = Rng(seed | 1);
        for _ in 0..4 {
            rng.next();
        }
        let n_countries = 2 + rng.below(2);
        let mut net = Network::default();
        for c in 0..n_countries {
            for _ in 0..1 + rng.below(5) {
                net.province_country.push(c as u16);
            }
            for _ in 0..1 + rng.below(2) {
                net.gateway_country.push(c as u16);
            }
        }
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

    fn kernel(net: &Network, wedges: &Wedges) -> Kernel {
        Kernel::build(net, &W, wedges, &TradeParams::default())
    }

    fn tau_leg(g: usize, l: &Leg) -> f64 {
        1.0 + l.cost_per_t * W[g] + TradeParams::default().time_cost[g] * l.hours / 24.0
    }

    /// K of a leg by the spec formula, stored as f32 like the kernel.
    fn k_leg(g: usize, l: &Leg, extra: f64) -> f64 {
        f64::powf(tau_leg(g, l) * extra, -TradeParams::default().theta) as f32 as f64
    }

    fn find(legs: &[Leg], from: usize, to: usize) -> &Leg {
        legs.iter()
            .find(|l| (l.from as usize, l.to as usize) == (from, to))
            .unwrap()
    }

    struct Hop {
        k: f64,
        tau: f64,
        lag: usize,
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
                lag: 0,
                tariff: 0.0,
            }];
        }
        if co == cd {
            let l = find(&net.domestic, o, d);
            return vec![Hop {
                k: k_leg(g, l, 1.0),
                tau: tau_leg(g, l),
                lag: 0,
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
                    lag: ((ab.hours / 730.0).round().min(3.0)) as usize,
                    tariff: t,
                });
            }
        }
        hs
    }

    #[derive(Default)]
    struct Dense {
        home: Vec<f64>,
        arrive: [Vec<f64>; 4],
        paid: Vec<f64>,
        received: Vec<f64>,
        tariff: Vec<f64>,
        shipped: Vec<f64>,
        lost: f64,
        netback: Vec<f64>,
    }

    /// The spec equations on a dense route table, independent of the Markov split. Every
    /// route pays the destination price per arriving unit; the tariff takes t / (1 + t) of
    /// that and the origin receives the rest.
    fn dense(
        net: &Network,
        wedges: &Wedges,
        g: usize,
        supply: &[f64],
        price: &[f64],
        active: &[bool],
    ) -> Dense {
        let n = net.n_provinces();
        let theta = TradeParams::default().theta;
        let pmax = (0..n)
            .filter(|&d| active[d])
            .map(|d| price[d])
            .fold(0.0, f64::max);
        let pt: Vec<f64> = (0..n)
            .map(|d| {
                let w = f64::powf(price[d] / pmax, theta);
                if active[d] && w >= FLOOR { w } else { 0.0 }
            })
            .collect();
        let mut r = Dense {
            home: vec![0.0; n],
            arrive: std::array::from_fn(|_| vec![0.0; n]),
            paid: vec![0.0; n],
            received: vec![0.0; n],
            tariff: vec![0.0; n],
            shipped: vec![0.0; n],
            netback: vec![0.0; n],
            lost: 0.0,
        };
        for o in 0..n {
            let all: Vec<Vec<Hop>> = (0..n).map(|d| hops(net, wedges, g, o, d)).collect();
            let pi: f64 = (0..n)
                .map(|d| all[d].iter().map(|h| h.k).sum::<f64>() * pt[d])
                .sum();
            if pi <= 0.0 {
                r.home[o] = supply[o];
                r.netback[o] = price[o];
                continue;
            }
            for d in 0..n {
                for h in &all[d] {
                    let share = h.k * pt[d] / pi;
                    let x = supply[o] * share;
                    r.netback[o] += share * price[d] / (h.tau * (1.0 + h.tariff));
                    if o == d {
                        r.home[o] += x;
                        continue;
                    }
                    let got = x / h.tau;
                    r.shipped[o] += x;
                    r.arrive[h.lag][d] += got;
                    r.lost += x - got;
                    let delivered = price[d] * got;
                    r.paid[d] += delivered;
                    r.received[o] += delivered / (1.0 + h.tariff);
                    r.tariff[d] += delivered * h.tariff / (1.0 + h.tariff);
                }
            }
        }
        r
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-10 * a.abs().max(b.abs()) + 1e-14
    }

    fn close12(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * a.abs().max(b.abs())
    }

    fn assert_close(what: &str, got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len(), "{what} length");
        for (i, (&a, &b)) in got.iter().zip(want).enumerate() {
            assert!(close(a, b), "{what}[{i}]: {a} vs {b}");
        }
    }

    /// seed, good, supply, log10 price, active; vectors cover up to 15 provinces.
    type Case = (u64, usize, Vec<f64>, Vec<f64>, Vec<bool>);

    fn case(log_price: std::ops::Range<f64>) -> impl Strategy<Value = Case> {
        (
            any::<u64>(),
            0..7usize,
            prop::collection::vec(0.0..1000.0f64, 15),
            prop::collection::vec(log_price, 15),
            prop::collection::vec(prop::bool::weighted(0.8), 15),
        )
    }

    struct Setup {
        net: Network,
        wedges: Wedges,
        kernel: Kernel,
        g: usize,
        supply: Vec<f64>,
        price: Vec<f64>,
        active: Vec<bool>,
    }

    fn setup((seed, g, supply, log_price, active): Case) -> Setup {
        let (net, wedges) = random_world(seed);
        let n = net.n_provinces();
        Setup {
            kernel: kernel(&net, &wedges),
            g,
            supply: supply[..n].to_vec(),
            price: log_price[..n].iter().map(|&e| 10f64.powf(e)).collect(),
            active: active[..n].to_vec(),
            net,
            wedges,
        }
    }

    fn run(s: &Setup) -> Routed {
        let mut out = Routed::new();
        route(&s.kernel, s.g, &s.supply, &s.price, &s.active, &mut out);
        out
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]

        #[test]
        fn routing_conserves_origin_mass(c in case(-2.0..2.0)) {
            let s = setup(c);
            let out = run(&s);
            for o in 0..s.net.n_provinces() {
                let total = out.home[o] + out.shipped[o];
                let rel = (total - s.supply[o]).abs() / s.supply[o].max(1e-300);
                prop_assert!(s.supply[o] == 0.0 && total == 0.0 || rel < 1e-12,
                    "origin {o}: {total} vs {}", s.supply[o]);
            }
        }

        #[test]
        fn markov_aggregation_matches_dense(c in case(-1.0..1.0)) {
            let s = setup(c);
            let out = run(&s);
            let want = dense(&s.net, &s.wedges, s.g, &s.supply, &s.price, &s.active);
            for lag in 0..4 {
                assert_close(&format!("arrive[{lag}]"), &out.arrive[lag], &want.arrive[lag]);
            }
            assert_close("home", &out.home, &want.home);
            assert_close("shipped", &out.shipped, &want.shipped);
            assert_close("paid", &out.paid, &want.paid);
            assert_close("received", &out.received, &want.received);
            assert_close("tariff", &out.tariff, &want.tariff);
            assert_close("netback", &out.netback, &want.netback);
            assert_close("lost", &[out.lost], &[want.lost]);
        }

        #[test]
        fn paid_equals_received_plus_tariff(c in case(-2.0..2.0)) {
            let s = setup(c);
            let out = run(&s);
            let paid: f64 = out.paid.iter().sum();
            let received: f64 = out.received.iter().sum();
            let tariff: f64 = out.tariff.iter().sum();
            prop_assert!(close12(paid, received + tariff), "{paid} vs {received} + {tariff}");

            // money moves d to b, b to a, a to o: each gateway pays out what it takes in
            let gk = s.kernel.good(s.g);
            let mut money_in = vec![0.0; s.kernel.n_gateways];
            let mut money_out = vec![0.0; s.kernel.n_gateways];
            for p in 0..s.kernel.n_provinces {
                let lo = gk.access.offsets[p] as usize;
                for (i, e) in gk.access.row(p).iter().enumerate() {
                    money_in[e.col as usize] += out.access_in_value[lo + i];
                    money_out[e.col as usize] += out.access_out_value[lo + i];
                }
            }
            for (a, m_in) in money_in.iter_mut().enumerate() {
                let lo = gk.crossings.offsets[a] as usize;
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    *m_in += out.crossing_value[lo + i];
                    money_out[e.col as usize] += out.crossing_value[lo + i];
                }
            }
            for (a, (m_in, m_out)) in money_in.iter().zip(&money_out).enumerate() {
                prop_assert!(close12(*m_in, *m_out), "gateway {a}: {m_in} vs {m_out}");
            }

            // per province, the leg values and the tariff add up to the totals
            let mut paid = out.tariff.clone();
            let mut received = vec![0.0; s.kernel.n_provinces];
            for p in 0..s.kernel.n_provinces {
                let lo = gk.domestic.offsets[p] as usize;
                for (i, e) in gk.domestic.row(p).iter().enumerate() {
                    received[p] += out.domestic_value[lo + i];
                    paid[e.col as usize] += out.domestic_value[lo + i];
                }
                let lo = gk.access.offsets[p] as usize;
                for i in 0..gk.access.row(p).len() {
                    received[p] += out.access_out_value[lo + i];
                    paid[p] += out.access_in_value[lo + i];
                }
            }
            for p in 0..s.kernel.n_provinces {
                prop_assert!(close12(paid[p], out.paid[p]), "paid {p}: {} vs {}", paid[p], out.paid[p]);
                prop_assert!(close12(received[p], out.received[p]),
                    "received {p}: {} vs {}", received[p], out.received[p]);
            }
        }

        #[test]
        fn origin_revenue_per_unit_is_netback(c in case(-1.0..1.0)) {
            let s = setup(c);
            let out = run(&s);
            let want = dense(&s.net, &s.wedges, s.g, &s.supply, &s.price, &s.active);
            for o in (0..s.net.n_provinces()).filter(|&o| out.shipped[o] > 0.0) {
                // the home route sells at the origin's own price
                let revenue = s.price[o] * out.home[o] + out.received[o];
                let per_unit = revenue / s.supply[o];
                prop_assert!(close12(per_unit, want.netback[o]),
                    "origin {o}: {per_unit} vs dense {}", want.netback[o]);
                prop_assert!(close12(per_unit, out.netback[o]),
                    "origin {o}: {per_unit} vs routed {}", out.netback[o]);
            }
        }

        #[test]
        fn tariff_matches_rate_times_crossing_value(c in case(-1.0..1.0)) {
            let s = setup(c);
            let out = run(&s);
            let gk = s.kernel.good(s.g);
            let mut due = 0.0;
            for a in 0..s.kernel.n_gateways {
                let lo = gk.crossings.offsets[a] as usize;
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    due += f64::from(e.tariff) * out.crossing_value[lo + i];
                }
            }
            let booked: f64 = out.tariff.iter().sum();
            prop_assert!(close(due, booked), "{due} vs {booked}");
        }

        #[test]
        fn goods_are_conserved_through_losses(c in case(-2.0..2.0)) {
            let s = setup(c);
            let out = run(&s);
            let shipped: f64 = out.shipped.iter().sum();
            let arrived: f64 = out.arrive.iter().flatten().sum();
            prop_assert!(close(shipped, arrived + out.lost), "{shipped} vs {arrived} + {}", out.lost);
        }

        #[test]
        fn extreme_prices_stay_finite(c in case(-80.0..80.0)) {
            let s = setup(c);
            let out = run(&s);
            let all_finite = |v: &[f64]| v.iter().all(|x| x.is_finite() && *x >= 0.0);
            prop_assert!(all_finite(&out.home) && all_finite(&out.shipped));
            prop_assert!(all_finite(&out.paid) && all_finite(&out.received));
            prop_assert!(all_finite(&out.tariff) && all_finite(&out.netback));
            prop_assert!(all_finite(&out.access_out_value));
            prop_assert!(all_finite(&out.crossing_value) && all_finite(&out.access_in_value));
            prop_assert!(out.arrive.iter().all(|a| all_finite(a)));
            prop_assert!(out.lost.is_finite() && out.lost >= 0.0);
            for o in 0..s.net.n_provinces() {
                let total = out.home[o] + out.shipped[o];
                let rel = (total - s.supply[o]).abs() / s.supply[o].max(1e-300);
                prop_assert!(s.supply[o] == 0.0 && total == 0.0 || rel < 1e-12);
            }
            let shipped: f64 = out.shipped.iter().sum();
            let arrived: f64 = out.arrive.iter().flatten().sum();
            prop_assert!(close(shipped, arrived + out.lost));
            let paid: f64 = out.paid.iter().sum();
            let received: f64 = out.received.iter().sum();
            let tariff: f64 = out.tariff.iter().sum();
            prop_assert!(close(paid, received + tariff), "{paid} vs {received} + {tariff}");
        }
    }

    #[test]
    fn flows_ignore_the_price_level() {
        let (net, wedges) = random_world(99);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = Rng(5);
        let supply: Vec<f64> = (0..n).map(|_| 10.0 + 90.0 * rng.next()).collect();
        let price: Vec<f64> = (0..n).map(|_| 0.5 + rng.next()).collect();
        let active = vec![true; n];
        let (mut a, mut b) = (Routed::new(), Routed::new());
        route(&k, 4, &supply, &price, &active, &mut a);
        let big: Vec<f64> = price.iter().map(|p| p * 1e40).collect();
        route(&k, 4, &supply, &big, &active, &mut b);
        for lag in 0..4 {
            assert_close("arrive", &a.arrive[lag], &b.arrive[lag]);
        }
        assert_close("home", &a.home, &b.home);
        let scaled: Vec<f64> = a.paid.iter().map(|v| v * 1e40).collect();
        assert_close("paid", &scaled, &b.paid);
    }

    /// Provinces 0 (country 0) and 1 (country 1), one gateway each.
    fn pair_world(tariff: f32) -> (Network, Wedges) {
        let leg = |from, to, cost_per_t, hours| Leg {
            from,
            to,
            cost_per_t,
            hours,
        };
        let net = Network {
            province_country: vec![0, 1],
            gateway_country: vec![0, 1],
            access: vec![leg(0, 0, 10.0, 24.0), leg(1, 1, 20.0, 48.0)],
            crossings: vec![leg(0, 1, 100.0, 1500.0), leg(1, 0, 100.0, 1500.0)],
            ..Network::default()
        };
        let mut wedges = Wedges::none(2);
        // country 1 imports from country 0, manufactured group
        wedges.tariff[2 * 2 + 1] = tariff;
        (net, wedges)
    }

    #[test]
    fn tariff_is_share_of_delivered_value() {
        let (net, wedges) = pair_world(0.25);
        let k = kernel(&net, &wedges);
        let g = 4;
        let (supply, price, active) = ([100.0, 0.0], [2.0, 3.0], [true, true]);
        let mut out = Routed::new();
        route(&k, g, &supply, &price, &active, &mut out);

        // exports of province 0: the home route competes with the one gateway route
        let (oa, ab, bd) = (&net.access[0], &net.crossings[0], &net.access[1]);
        let nu = 1.0 + TradeParams::default().border_nu;
        let kroute = k_leg(g, oa, 1.0) * k_leg(g, ab, nu * 1.25) * k_leg(g, bd, 1.0);
        let pt1 = (3.0f64 / 3.0).powf(5.0);
        let pt0 = (2.0f64 / 3.0).powf(5.0);
        let x = 100.0 * kroute * pt1 / (pt0 + kroute * pt1);
        assert!(close(out.shipped[0], x));
        assert!(close(out.home[0], 100.0 - x));
        let tau = tau_leg(g, oa) * tau_leg(g, ab) * nu * tau_leg(g, bd);
        let got = x / tau;
        let delivered = 3.0 * got;
        assert!(close12(out.paid[1], delivered));
        assert!(close12(out.tariff[1], 0.2 * delivered));
        assert!(close12(out.received[0], 0.8 * delivered));
        assert_eq!(out.tariff[0], 0.0);
        assert_eq!(out.paid[0], 0.0);
        // each leg carries the payment net of the tariff
        for leg in [
            out.access_in_value[1],
            out.crossing_value[0],
            out.access_out_value[0],
        ] {
            assert!(
                close12(leg, 0.8 * delivered),
                "{leg} vs {}",
                0.8 * delivered
            );
        }
        let netback = (pt0 * 2.0 + kroute * pt1 * 3.0 / (tau * 1.25)) / (pt0 + kroute * pt1);
        assert!(close12(out.netback[0], netback));
        assert!(close12(
            (2.0 * out.home[0] + out.received[0]) / 100.0,
            netback
        ));
        // province 1 has no supply; the reverse crossing carries nothing
        assert_eq!(out.crossing_value[1], 0.0);
        // lag round(1500 / 730) = 2
        assert!(close(out.arrive[2][1], got));
        assert!(
            out.arrive[0]
                .iter()
                .chain(&out.arrive[1])
                .chain(&out.arrive[3])
                .all(|&v| v == 0.0)
        );
        let lost = x - got;
        assert!(close(out.lost, lost));
    }

    #[test]
    fn domestic_arrivals_use_lag_zero_and_pay_the_origin_directly() {
        let net = Network {
            province_country: vec![0, 0],
            domestic: vec![
                Leg {
                    from: 0,
                    to: 1,
                    cost_per_t: 100.0,
                    hours: 3000.0,
                },
                Leg {
                    from: 1,
                    to: 0,
                    cost_per_t: 100.0,
                    hours: 3000.0,
                },
            ],
            ..Network::default()
        };
        let k = kernel(&net, &Wedges::none(1));
        let mut out = Routed::new();
        route(&k, 4, &[50.0, 0.0], &[1.0, 2.0], &[true, true], &mut out);
        assert!(out.shipped[0] > 0.0);
        assert!(out.arrive[0][1] > 0.0);
        assert!(
            out.arrive[1]
                .iter()
                .chain(&out.arrive[2])
                .chain(&out.arrive[3])
                .all(|&v| v == 0.0)
        );
        assert!(close12(out.paid[1], out.received[0]));
        assert!(close12(out.received[0], 2.0 * out.arrive[0][1]));
        assert!(out.received[0] < 2.0 * out.shipped[0]);
        assert_eq!(out.tariff, vec![0.0, 0.0]);
        assert!(out.crossing_value.is_empty());
    }

    #[test]
    fn inactive_destinations_get_no_flow() {
        let (net, wedges) = random_world(3);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut active = vec![true; n];
        active[0] = false;
        let mut out = Routed::new();
        route(&k, 0, &vec![10.0; n], &vec![1.0; n], &active, &mut out);
        assert_eq!(
            out.home[0], 0.0,
            "an inactive origin does not keep home supply"
        );
        assert_eq!(out.arrive.iter().map(|a| a[0]).sum::<f64>(), 0.0);
        assert_eq!(out.paid[0], 0.0);
        assert_eq!(out.tariff[0], 0.0);
        // its supply goes out whole
        assert!(close(out.shipped[0], 10.0));
    }

    #[test]
    fn unreachable_origin_keeps_supply_and_reports_its_price() {
        // no active destination anywhere: everything stays home
        let (net, wedges) = pair_world(0.0);
        let k = kernel(&net, &wedges);
        let mut out = Routed::new();
        route(&k, 4, &[5.0, 7.0], &[1.5, 2.5], &[false, false], &mut out);
        assert_eq!(out.home, vec![5.0, 7.0]);
        assert_eq!(out.shipped, vec![0.0, 0.0]);
        assert_eq!(out.netback, vec![1.5, 2.5]);
        assert_eq!(out.lost, 0.0);
        assert!(out.arrive.iter().flatten().all(|&v| v == 0.0));

        // domestic-only world with every market inactive: Pi = 0
        let net = Network {
            province_country: vec![0, 0],
            domestic: vec![
                Leg {
                    from: 0,
                    to: 1,
                    cost_per_t: 10.0,
                    hours: 10.0,
                },
                Leg {
                    from: 1,
                    to: 0,
                    cost_per_t: 10.0,
                    hours: 10.0,
                },
            ],
            ..Network::default()
        };
        let k = kernel(&net, &Wedges::none(1));
        route(&k, 3, &[4.0, 6.0], &[1.0, 1.0], &[false, false], &mut out);
        assert_eq!(out.home, vec![4.0, 6.0]);
    }

    #[test]
    fn netback_of_a_lone_province_is_its_price() {
        let net = Network {
            province_country: vec![0],
            ..Network::default()
        };
        let k = kernel(&net, &Wedges::none(1));
        let mut out = Routed::new();
        route(&k, 0, &[3.0], &[1.7], &[true], &mut out);
        assert_eq!(out.home, vec![3.0]);
        assert!(close(out.netback[0], 1.7));
    }

    #[test]
    fn zero_supply_ships_nothing_but_still_reports_netback() {
        let (net, wedges) = pair_world(0.1);
        let k = kernel(&net, &wedges);
        let mut out = Routed::new();
        route(&k, 4, &[0.0, 0.0], &[1.0, 2.0], &[true, true], &mut out);
        assert!(out.home.iter().chain(&out.shipped).all(|&v| v == 0.0));
        assert_eq!(out.lost, 0.0);
        assert!(out.netback.iter().all(|&v| v > 0.0 && v.is_finite()));
    }

    #[test]
    fn reuse_overwrites_and_does_not_reallocate() {
        let (net, wedges) = random_world(11);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = Rng(8);
        let mut draw = |scale: f64| -> (Vec<f64>, Vec<f64>) {
            (
                (0..n).map(|_| scale * rng.next()).collect(),
                (0..n).map(|_| 0.5 + rng.next()).collect(),
            )
        };
        let active = vec![true; n];
        let mut reused = Routed::new();
        let (s1, p1) = draw(100.0);
        route(&k, 5, &s1, &p1, &active, &mut reused);
        let ptrs = (
            reused.home.as_ptr(),
            reused.arrive[2].as_ptr(),
            reused.crossing_value.as_ptr(),
            reused.scratch.price_theta.as_ptr(),
        );
        for g in [0, 3, 6] {
            let (s, p) = draw(50.0);
            route(&k, g, &s, &p, &active, &mut reused);
            let mut fresh = Routed::new();
            route(&k, g, &s, &p, &active, &mut fresh);
            assert_eq!(reused.home, fresh.home);
            assert_eq!(reused.arrive, fresh.arrive);
            assert_eq!(reused.crossing_value, fresh.crossing_value);
            assert_eq!(reused.lost, fresh.lost);
            assert_eq!(reused.netback, fresh.netback);
        }
        let after = (
            reused.home.as_ptr(),
            reused.arrive[2].as_ptr(),
            reused.crossing_value.as_ptr(),
            reused.scratch.price_theta.as_ptr(),
        );
        assert_eq!(ptrs, after);
    }

    #[test]
    fn route_is_deterministic_to_the_bit() {
        let (net, wedges) = random_world(21);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let supply: Vec<f64> = (0..n).map(|i| 3.0 + i as f64).collect();
        let price: Vec<f64> = (0..n).map(|i| 1.0 + 0.1 * i as f64).collect();
        let active = vec![true; n];
        let (mut a, mut b) = (Routed::new(), Routed::new());
        route(&k, 4, &supply, &price, &active, &mut a);
        route(&k, 4, &supply, &price, &active, &mut b);
        let bits = |r: &Routed| -> Vec<u64> {
            r.arrive
                .iter()
                .flatten()
                .chain(&r.home)
                .chain(&r.paid)
                .chain(&r.tariff)
                .chain(&r.netback)
                .chain(&[r.lost])
                .map(|v| v.to_bits())
                .collect()
        };
        assert_eq!(bits(&a), bits(&b));
    }
}
