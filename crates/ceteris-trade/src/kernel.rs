//! Two-level trade kernel (`specs/trade/SPEC.md`, Equations).
//!
//! Domestic pairs use the direct leg, `K_od = tau_od^-theta_g`, dense within a country. A
//! cross-border pair is reached through gateways, `K_od = sum_ab K_oa K_ab K_bd`, and never
//! materialized: [`Kernel::origin_access_into`] evaluates the sums through gateway aggregates.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::goods::{N_GOODS, TRADED, tariff_group};
use crate::network::{Leg, Network, Wedges};
use crate::params::TradeParams;

/// Longest international transit lag in months.
const MAX_LAG: f64 = 3.0;
/// Hours per month for transit lags.
const HOURS_PER_MONTH: f64 = 730.0;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Rows of entries in compressed sparse row layout.
#[derive(Clone, Debug, Default)]
pub struct Csr<E> {
    pub offsets: Vec<u32>,
    pub entries: Vec<E>,
}

impl<E> Csr<E> {
    pub fn n_rows(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    pub fn row(&self, i: usize) -> &[E] {
        &self.entries[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }

    /// Entries of a row keep the order of the input legs; `entry` gets the leg's index.
    fn from_legs(n_rows: usize, legs: &[Leg], entry: impl Fn(usize, &Leg) -> E) -> Self {
        let mut order: Vec<usize> = (0..legs.len()).collect();
        order.sort_by_key(|&i| legs[i].from);
        let mut offsets = vec![0u32; n_rows + 1];
        for l in legs {
            offsets[l.from as usize + 1] += 1;
        }
        for i in 0..n_rows {
            offsets[i + 1] += offsets[i];
        }
        Self {
            offsets,
            entries: order.into_iter().map(|i| entry(i, &legs[i])).collect(),
        }
    }
}

/// A domestic or access leg: `tau = 1 + c w + lambda T` on the least-cost leg (S05), or the
/// route-choice factor when the network carries one, and `k = tau^-theta_g`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub col: u32,
    pub k: f32,
    pub tau: f64,
}

/// A gateway crossing. `tau` is the leg factor without border friction or tariff;
/// `border_tau = 1 + nu` is the iceberg border loss, destroyed at the crossing. The tariff
/// is a routing wedge only. `k = (tau * border_tau * (1 + tariff))^-theta_g`. `lag` is the
/// extra transit time in months, `round(hours / 730)` capped at 3, from the least-cost leg's
/// hours or the good's expected route hours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub col: u32,
    pub k: f32,
    pub tau: f64,
    pub border_tau: f64,
    pub tariff: f32,
    pub lag: u8,
}

/// Kernel legs of one traded good.
#[derive(Clone, Debug)]
pub struct GoodKernel {
    /// Rows over origin provinces, same-country destinations.
    pub domestic: Csr<Entry>,
    /// Rows over provinces, columns are gateways of the province's country. Read as
    /// province to gateway for origin access and gateway to province for destination access.
    pub access: Csr<Entry>,
    /// Rows over gateways, columns are gateways of other countries.
    pub crossings: Csr<Crossing>,
}

#[derive(Clone, Debug)]
pub struct Kernel {
    pub n_provinces: usize,
    pub n_gateways: usize,
    /// P01 per good as used to build `k`; sourcing needs it for `p^-theta_g`. Kernels are
    /// per good: in P2 each good is its own weight class, so a theta class adds no kernel
    /// (ADR 0018).
    pub theta: [f64; N_GOODS],
    goods: [Option<GoodKernel>; N_GOODS],
    /// Unique per build and per change, so `source` can tell a kernel it has seen.
    id: u64,
}

/// Results of the origin-side low-rank evaluation for one good, written by
/// [`Kernel::origin_access_into`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Access {
    /// Origin access at each gateway, `v'_a = sum_{o in A} K_oa w_o`.
    pub v: Vec<f64>,
    /// Access of each destination gateway to every foreign origin, `u'_b = sum_a v'_a K_ab`.
    pub u: Vec<f64>,
    /// Sourcing normalizer per destination province, `Phi_d = sum_o K_od w_o`, home term
    /// `w_d` included.
    pub pi: Vec<f64>,
}

impl Kernel {
    /// `weight` is tonnes per unit of value per good. Panics on legs that break the
    /// [`Network`] contract. Tariffs are looked up with the country count implied by `wedges`.
    pub fn build(
        net: &Network,
        weight: &[f64; N_GOODS],
        wedges: &Wedges,
        p: &TradeParams,
    ) -> Kernel {
        let (np, ng) = (net.n_provinces(), net.n_gateways());
        validate(net, np, ng);
        if !net.crossings.is_empty() {
            let n_countries = wedges.n_countries();
            let max = net
                .province_country
                .iter()
                .chain(&net.gateway_country)
                .max()
                .map_or(0, |&c| usize::from(c));
            assert!(
                max < n_countries,
                "wedges cover {n_countries} countries, network has {}",
                max + 1
            );
        }
        let goods = std::array::from_fn(|g| {
            TRADED[g].then(|| good_kernel(net, np, ng, g, weight[g], wedges, p))
        });
        Kernel {
            n_provinces: np,
            n_gateways: ng,
            theta: p.theta,
            goods,
            id: next_id(),
        }
    }

    /// Identifies this kernel's contents: kernels with the same id are equal.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Kernel of a traded good; panics for a local good.
    pub fn good(&self, g: usize) -> &GoodKernel {
        self.goods[g]
            .as_ref()
            .unwrap_or_else(|| panic!("good {g} is not traded"))
    }

    pub fn try_good(&self, g: usize) -> Option<&GoodKernel> {
        self.goods[g].as_ref()
    }

    /// Origin-side low-rank evaluation into reused buffers. `weight` must be floored
    /// sourcing weights as [`crate::source::origin_weights`] writes them: `(p_o / p_min)^-theta_g`
    /// for an active origin at or above `WEIGHT_FLOOR`, 0 otherwise. Unfloored weights can
    /// make products with the f32 kernel factors subnormal and the shares inaccurate.
    /// Writes `v'_a = sum_{o in A} K_oa w_o`, `u'_b = sum_a v'_a K_ab` and
    /// `Phi_d = sum_{o in B} K_od w_o + sum_b K_bd u'_b`, home term `K_dd w_d = w_d` included.
    pub fn origin_access_into(&self, g: usize, weight: &[f64], out: &mut Access) {
        assert_eq!(weight.len(), self.n_provinces);
        let gk = self.good(g);

        out.v.clear();
        out.v.resize(self.n_gateways, 0.0);
        for (o, &w) in weight.iter().enumerate() {
            for e in gk.access.row(o) {
                out.v[e.col as usize] += f64::from(e.k) * w;
            }
        }
        out.u.clear();
        out.u.resize(self.n_gateways, 0.0);
        for (a, &va) in out.v.iter().enumerate() {
            for e in gk.crossings.row(a) {
                out.u[e.col as usize] += va * f64::from(e.k);
            }
        }
        out.pi.clear();
        out.pi.extend_from_slice(weight);
        for (o, &w) in weight.iter().enumerate() {
            for e in gk.domestic.row(o) {
                out.pi[e.col as usize] += f64::from(e.k) * w;
            }
        }
        for (d, phi) in out.pi.iter_mut().enumerate() {
            let abroad: f64 = gk
                .access
                .row(d)
                .iter()
                .map(|e| f64::from(e.k) * out.u[e.col as usize])
                .sum();
            *phi += abroad;
        }
    }
}

impl Kernel {
    /// Sets the transit lag of every crossing of every good to `lag` months, at most
    /// `MAX_LAG`. Lags only time deliveries, so steady states do not change; for nesting
    /// tests (lags 0) and stability studies (the longest lag).
    pub fn set_crossing_lags(&mut self, lag: u8) {
        assert!(f64::from(lag) <= MAX_LAG, "lag {lag} above {MAX_LAG}");
        self.id = next_id();
        for gk in self.goods.iter_mut().flatten() {
            gk.crossings.entries.iter_mut().for_each(|e| e.lag = lag);
        }
    }
}

fn validate(net: &Network, np: usize, ng: usize) {
    for l in &net.domestic {
        let (o, d) = (l.from as usize, l.to as usize);
        assert!(o < np && d < np && o != d, "bad domestic leg {l:?}");
        assert_eq!(
            net.province_country[o], net.province_country[d],
            "domestic leg crosses a border: {l:?}"
        );
    }
    for l in &net.access {
        let (o, a) = (l.from as usize, l.to as usize);
        assert!(o < np && a < ng, "bad access leg {l:?}");
        assert_eq!(
            net.province_country[o], net.gateway_country[a],
            "access leg leaves the country: {l:?}"
        );
    }
    for l in &net.crossings {
        let (a, b) = (l.from as usize, l.to as usize);
        assert!(a < ng && b < ng, "bad crossing {l:?}");
        assert_ne!(
            net.gateway_country[a], net.gateway_country[b],
            "crossing within a country: {l:?}"
        );
    }
    for l in net.domestic.iter().chain(&net.access).chain(&net.crossings) {
        assert!(
            l.cost_per_t >= 0.0 && l.hours >= 0.0,
            "negative leg cost: {l:?}"
        );
    }
    if let Some(rt) = &net.route_taus {
        let lists = [
            (
                "domestic",
                &rt.domestic,
                &rt.domestic_hours,
                net.domestic.len(),
            ),
            ("access", &rt.access, &rt.access_hours, net.access.len()),
            (
                "crossing",
                &rt.crossings,
                &rt.crossing_hours,
                net.crossings.len(),
            ),
        ];
        for (name, taus, hours, n) in lists {
            assert!(
                taus.len() == n && hours.len() == n,
                "{name} route taus: {} factors and {} hours for {n} legs",
                taus.len(),
                hours.len()
            );
            for (i, (t, h)) in taus.iter().zip(hours).enumerate() {
                for g in (0..N_GOODS).filter(|&g| TRADED[g]) {
                    // t >= 1 also rejects NaN; below 1 a leg would create goods
                    assert!(
                        t[g] >= 1.0,
                        "{name} route tau {} below 1 at leg {i}, good {g}",
                        t[g]
                    );
                    assert!(
                        h[g] >= 0.0,
                        "{name} route hours {} at leg {i}, good {g}",
                        h[g]
                    );
                }
            }
        }
    }
}

fn good_kernel(
    net: &Network,
    np: usize,
    ng: usize,
    g: usize,
    w: f64,
    wedges: &Wedges,
    p: &TradeParams,
) -> GoodKernel {
    let least = |l: &Leg| 1.0 + l.cost_per_t * w + p.time_cost[g] * l.hours / 24.0;
    let rt = net.route_taus.as_ref();
    let k_of = |total: f64| libm::pow(total, -p.theta[g]) as f32;
    let plain = |taus: Option<&Vec<[f64; N_GOODS]>>, i: usize, l: &Leg| {
        let tau = taus.map_or_else(|| least(l), |t| t[i][g]);
        Entry {
            col: l.to,
            k: k_of(tau),
            tau,
        }
    };
    let group = tariff_group(g);
    let border_tau = 1.0 + p.border_nu;
    let crossing = |i: usize, l: &Leg| {
        let (tau, hours) = rt.map_or_else(
            || (least(l), l.hours),
            |rt| (rt.crossings[i][g], rt.crossing_hours[i][g]),
        );
        let importer = usize::from(net.gateway_country[l.to as usize]);
        let exporter = usize::from(net.gateway_country[l.from as usize]);
        let tariff = wedges.rate(importer, exporter, group);
        Crossing {
            col: l.to,
            k: k_of(tau * border_tau * (1.0 + f64::from(tariff))),
            tau,
            border_tau,
            tariff,
            lag: (hours / HOURS_PER_MONTH).round().min(MAX_LAG) as u8,
        }
    };
    GoodKernel {
        domestic: Csr::from_legs(np, &net.domestic, |i, l| {
            plain(rt.map(|r| &r.domestic), i, l)
        }),
        access: Csr::from_legs(np, &net.access, |i, l| plain(rt.map(|r| &r.access), i, l)),
        crossings: Csr::from_legs(ng, &net.crossings, crossing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goods::{N_GOODS, tariff_group};
    use crate::network::{Leg, LegTaus, Network, Wedges};
    use crate::params::TradeParams;

    const W: [f64; N_GOODS] = [0.5, 0.8, 0.9, 0.3, 0.002, 0.004, 0.001, 0.0, 0.0, 0.0];

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn leg(&mut self, from: u32, to: u32) -> Leg {
            Leg {
                from,
                to,
                cost_per_t: 5.0 + 400.0 * self.next(),
                hours: 1.0 + 2000.0 * self.next(),
            }
        }
    }

    /// K of a leg by the spec formula, independent of the implementation, stored as f32.
    fn leg_k(p: &TradeParams, g: usize, leg: &Leg, extra: f64) -> f64 {
        let tau = 1.0 + leg.cost_per_t * W[g] + p.time_cost[g] * leg.hours / 24.0;
        f64::powf(tau * extra, -p.theta[g]) as f32 as f64
    }

    /// Provinces 0..5 in country 0, 5..10 in country 1; gateways 0 and 1 in country 0,
    /// gateway 2 in country 1. Costs random.
    fn world(rng: &mut Rng) -> Network {
        let province_country: Vec<u16> = (0..10).map(|i| u16::from(i >= 5)).collect();
        let gateway_country = vec![0, 0, 1];
        let mut net = Network {
            province_country: province_country.clone(),
            gateway_country: gateway_country.clone(),
            ..Network::default()
        };
        for o in 0..10u32 {
            for d in 0..10u32 {
                if o != d && province_country[o as usize] == province_country[d as usize] {
                    net.domestic.push(rng.leg(o, d));
                }
            }
            for a in 0..3u32 {
                if province_country[o as usize] == gateway_country[a as usize] {
                    net.access.push(rng.leg(o, a));
                }
            }
        }
        for a in 0..3u32 {
            for b in 0..3u32 {
                if gateway_country[a as usize] != gateway_country[b as usize] {
                    net.crossings.push(rng.leg(a, b));
                }
            }
        }
        net
    }

    fn origin_access(k: &Kernel, g: usize, weight: &[f64]) -> Access {
        let mut out = Access::default();
        k.origin_access_into(g, weight, &mut out);
        out
    }

    fn two_provinces(cost: f64, hours: f64) -> Network {
        Network {
            province_country: vec![0, 0],
            domestic: vec![
                Leg {
                    from: 0,
                    to: 1,
                    cost_per_t: cost,
                    hours,
                },
                Leg {
                    from: 1,
                    to: 0,
                    cost_per_t: cost,
                    hours,
                },
            ],
            ..Network::default()
        }
    }

    #[test]
    fn leg_factor_matches_spec() {
        let p = TradeParams::default();
        let net = two_provinces(150.0, 48.0);
        let k = Kernel::build(&net, &W, &Wedges::none(1), &p);
        let e = k.good(4).domestic.row(0)[0];
        // 1 + 150 * 0.002 + 0.010 * 2 days
        assert!((e.tau - 1.32).abs() < 1e-12);
        let want = f64::powf(1.32, -p.theta[4]) as f32;
        assert!((e.k - want).abs() <= want * 1e-6);
        assert_eq!(e.col, 1);
    }

    #[test]
    fn kernel_uses_theta_per_good() {
        let mut p = TradeParams::default();
        p.theta[0] = 2.0;
        p.theta[4] = 8.0;
        let net = world(&mut Rng(5));
        let mut wedges = Wedges::none(2);
        wedges.tariff.iter_mut().for_each(|t| *t = 0.2);
        let k = Kernel::build(&net, &W, &wedges, &p);
        assert_eq!((k.theta[0], k.theta[4]), (2.0, 8.0));
        let nu = 1.0 + p.border_nu;
        for g in [0, 4] {
            let gk = k.good(g);
            for e in gk.domestic.entries.iter().chain(&gk.access.entries) {
                assert_eq!(e.k, libm::pow(e.tau, -p.theta[g]) as f32);
            }
            for e in &gk.crossings.entries {
                let total = e.tau * nu * (1.0 + f64::from(e.tariff));
                assert_eq!(e.k, libm::pow(total, -p.theta[g]) as f32);
            }
        }
        // same legs, the other theta: the factors differ
        let e = k.good(0).domestic.entries[0];
        assert_ne!(e.k, libm::pow(e.tau, -8.0) as f32);
    }

    #[test]
    fn local_goods_have_no_kernel() {
        let k = Kernel::build(
            &two_provinces(1.0, 1.0),
            &W,
            &Wedges::none(1),
            &TradeParams::default(),
        );
        assert!((0..N_GOODS).all(|g| k.try_good(g).is_some() == (g < 7)));
    }

    #[test]
    fn crossing_includes_border_and_tariff() {
        let p = TradeParams::default();
        let net = Network {
            province_country: vec![0, 1],
            gateway_country: vec![0, 1],
            access: vec![
                Leg {
                    from: 0,
                    to: 0,
                    cost_per_t: 10.0,
                    hours: 5.0,
                },
                Leg {
                    from: 1,
                    to: 1,
                    cost_per_t: 10.0,
                    hours: 5.0,
                },
            ],
            crossings: vec![
                Leg {
                    from: 0,
                    to: 1,
                    cost_per_t: 100.0,
                    hours: 1500.0,
                },
                Leg {
                    from: 1,
                    to: 0,
                    cost_per_t: 100.0,
                    hours: 5000.0,
                },
            ],
            ..Network::default()
        };
        // country 1 imports from 0 at 10% (primary) and 25% (manufactured); country 0
        // imports from 1 at 40% on primary only
        let mut wedges = Wedges::none(2);
        let at =
            |importer: usize, exporter: usize, group: usize| (importer * 2 + exporter) * 2 + group;
        wedges.tariff[at(1, 0, 0)] = 0.10;
        wedges.tariff[at(1, 0, 1)] = 0.25;
        wedges.tariff[at(0, 1, 0)] = 0.40;
        let k = Kernel::build(&net, &W, &wedges, &p);

        // 0 -> 1, good 4 (manufactured, group 1): lag round(1500 / 730) = 2
        let e = k.good(4).crossings.row(0)[0];
        let tau = 1.0 + 100.0 * W[4] + p.time_cost[4] * 1500.0 / 24.0;
        assert!((e.tau - tau).abs() < 1e-12);
        assert!((e.border_tau - 1.3).abs() < 1e-12);
        assert_eq!(e.tariff, 0.25);
        assert_eq!(e.lag, 2);
        let want = f64::powf(tau * 1.3 * 1.25, -p.theta[4]) as f32;
        assert!((e.k - want).abs() <= want * 1e-6);

        // same leg, good 0 (primary, group 0)
        let e = k.good(0).crossings.row(0)[0];
        assert_eq!(e.tariff, 0.10);
        let tau = 1.0 + 100.0 * W[0] + p.time_cost[0] * 1500.0 / 24.0;
        let want = f64::powf(tau * 1.3 * 1.10, -p.theta[0]) as f32;
        assert!((e.k - want).abs() <= want * 1e-6);

        // 1 -> 0, good 0: lag round(5000 / 730) = 7 capped at 3; importer is country 0
        let e = k.good(0).crossings.row(1)[0];
        assert_eq!(e.lag, 3);
        assert_eq!(e.tariff, 0.40);
        // good 4 in the same direction carries no tariff
        assert_eq!(k.good(4).crossings.row(1)[0].tariff, 0.0);
        assert_eq!(tariff_group(4), 1);

        // forced lags change nothing but the lags
        let mut forced = k.clone();
        forced.set_crossing_lags(0);
        for g in 0..7 {
            let (a, b) = (
                &k.good(g).crossings.entries,
                &forced.good(g).crossings.entries,
            );
            for (x, y) in a.iter().zip(b) {
                assert_eq!((y.lag, Crossing { lag: x.lag, ..*y }), (0, *x));
            }
        }
    }

    #[test]
    fn kernel_uses_route_taus_when_present() {
        let p = TradeParams::default();
        let mut net = world(&mut Rng(11));
        let least = Kernel::build(&net, &W, &Wedges::none(2), &p);
        // per-good factors unrelated to the least-cost legs; hours give lags 0 to 3
        let taus = |n: usize, base: f64| -> Vec<[f64; N_GOODS]> {
            (0..n)
                .map(|i| std::array::from_fn(|g| base + 0.01 * i as f64 + 0.1 * g as f64))
                .collect()
        };
        let hours = |n: usize| -> Vec<[f64; N_GOODS]> {
            (0..n)
                .map(|i| std::array::from_fn(|g| 730.0 * ((i + g) % 4) as f64))
                .collect()
        };
        let (nd, na, nc) = (net.domestic.len(), net.access.len(), net.crossings.len());
        let rt = LegTaus {
            domestic: taus(nd, 1.1),
            access: taus(na, 1.2),
            crossings: taus(nc, 1.3),
            domestic_hours: hours(nd),
            access_hours: hours(na),
            crossing_hours: hours(nc),
        };
        net.route_taus = Some(rt.clone());
        let k = Kernel::build(&net, &W, &Wedges::none(2), &p);
        let nu = 1.0 + p.border_nu;
        // entries keep leg order within a row; find each leg's entry by (row, col)
        let entry = |csr: &Csr<Entry>, l: &Leg| -> Entry {
            *csr.row(l.from as usize)
                .iter()
                .find(|e| e.col == l.to)
                .unwrap()
        };
        for g in 0..7 {
            let gk = k.good(g);
            for (i, l) in net.domestic.iter().enumerate() {
                let e = entry(&gk.domestic, l);
                assert_eq!(e.tau, rt.domestic[i][g]);
                assert_eq!(e.k, libm::pow(rt.domestic[i][g], -p.theta[g]) as f32);
            }
            for (i, l) in net.access.iter().enumerate() {
                let e = entry(&gk.access, l);
                assert_eq!(e.tau, rt.access[i][g]);
            }
            for (i, l) in net.crossings.iter().enumerate() {
                let e = gk
                    .crossings
                    .row(l.from as usize)
                    .iter()
                    .find(|e| e.col == l.to)
                    .unwrap();
                let tau = rt.crossings[i][g];
                assert_eq!(e.tau, tau);
                assert_eq!(e.k, libm::pow(tau * nu, -p.theta[g]) as f32);
                let lag = (rt.crossing_hours[i][g] / 730.0).round().min(3.0) as u8;
                assert_eq!(e.lag, lag);
            }
            // the least-cost kernel differs: the route taus were used
            assert_ne!(
                least.good(g).domestic.entries[0].tau,
                gk.domestic.entries[0].tau
            );
        }
        let lags: Vec<u8> = k.good(0).crossings.entries.iter().map(|e| e.lag).collect();
        assert!(lags.contains(&0) && lags.contains(&3), "{lags:?}");
    }

    #[test]
    #[should_panic(expected = "route tau")]
    fn route_taus_below_one_are_rejected() {
        let mut net = two_provinces(10.0, 10.0);
        net.route_taus = Some(LegTaus {
            domestic: vec![[0.9; N_GOODS]; 2],
            domestic_hours: vec![[1.0; N_GOODS]; 2],
            ..LegTaus::default()
        });
        Kernel::build(&net, &W, &Wedges::none(1), &TradeParams::default());
    }

    #[test]
    fn home_kernel_is_one() {
        let p = TradeParams::default();
        let net = world(&mut Rng(7));
        let k = Kernel::build(&net, &W, &Wedges::none(2), &p);
        for g in 0..7 {
            // no stored entry is a home entry
            let dom = &k.good(g).domestic;
            for o in 0..10 {
                assert!(dom.row(o).iter().all(|e| e.col as usize != o));
            }
            // only the destination's own weight reaches it at home: Phi_d = w_d exactly
            for d in 0..10 {
                let mut w = vec![0.0; 10];
                w[d] = 2.5;
                assert_eq!(origin_access(&k, g, &w).pi[d], 2.5);
            }
        }
    }

    #[test]
    fn phi_matches_dense_kernel() {
        let p = TradeParams::default();
        let mut rng = Rng(0x9e3779b97f4a7c15);
        let net = world(&mut rng);
        let mut wedges = Wedges::none(2);
        for t in &mut wedges.tariff {
            *t = (0.3 * rng.next()) as f32;
        }
        let kernel = Kernel::build(&net, &W, &wedges, &p);
        let nu = 1.0 + p.border_nu;

        for g in [0usize, 3, 4, 6] {
            let pt: Vec<f64> = (0..10)
                .map(|d| if d == 8 { 0.0 } else { 0.2 + 3.0 * rng.next() })
                .collect();

            // dense K_od by explicit enumeration of gateway routes
            let dense: Vec<Vec<f64>> = (0..10usize)
                .map(|o| {
                    (0..10usize)
                        .map(|d| {
                            if o == d {
                                1.0
                            } else if net.province_country[o] == net.province_country[d] {
                                let leg = net
                                    .domestic
                                    .iter()
                                    .find(|l| (l.from, l.to) == (o as u32, d as u32));
                                leg_k(&p, g, leg.unwrap(), 1.0)
                            } else {
                                let mut s = 0.0;
                                for a in 0..3usize {
                                    for b in 0..3usize {
                                        let ca = net.gateway_country[a] as usize;
                                        let cb = net.gateway_country[b] as usize;
                                        if ca != net.province_country[o] as usize
                                            || cb != net.province_country[d] as usize
                                        {
                                            continue;
                                        }
                                        let oa = net
                                            .access
                                            .iter()
                                            .find(|l| (l.from, l.to) == (o as u32, a as u32))
                                            .unwrap();
                                        let ab = net
                                            .crossings
                                            .iter()
                                            .find(|l| (l.from, l.to) == (a as u32, b as u32))
                                            .unwrap();
                                        let bd = net
                                            .access
                                            .iter()
                                            .find(|l| (l.from, l.to) == (d as u32, b as u32))
                                            .unwrap();
                                        let t = f64::from(wedges.rate(cb, ca, tariff_group(g)));
                                        s += leg_k(&p, g, oa, 1.0)
                                            * leg_k(&p, g, ab, nu * (1.0 + t))
                                            * leg_k(&p, g, bd, 1.0);
                                    }
                                }
                                s
                            }
                        })
                        .collect()
                })
                .collect();

            let acc = origin_access(&kernel, g, &pt);
            assert_eq!(acc.pi.len(), 10);
            assert_eq!(acc.v.len(), 3);
            assert_eq!(acc.u.len(), 3);
            for (d, &phi) in acc.pi.iter().enumerate() {
                let want: f64 = dense.iter().zip(&pt).map(|(row, p)| row[d] * p).sum();
                let rel = (phi - want).abs() / want;
                assert!(rel < 1e-12, "g {g} d {d}: {phi} vs {want}");
            }
        }
    }
}
