//! Destination sourcing on the two-level kernel (ADRs 0015 and 0016): destination spending
//! spread over origins, and the forward dispatch of what the origin markets fill.
//!
//! Destination d spends E_d (numeraire, tariff included) on good g over routes by
//! `X_od = E_d K_od p_o^-theta / Phi_d`, `Phi_d = sum_o K_od p_o^-theta`, theta the good's
//! P01 value, so its delivered price index is `P_d = Phi_d^(-1/theta)`. The kernel on a crossing carries the tariff, so X
//! is what d pays: the origin ships `X / (p_o (1 + t))` units and receives `p_o` per unit, the
//! importer's rebate account `t p_o` per unit. Domestic and home routes have t = 0.
//!
//! Buyers pay `q_d = Gamma_d P_d` (ADR 0200, selection on trade links), `Gamma_d` a constant per
//! destination and good given by the caller. Each route delivers `X / q_d` units: the shipped
//! units melt by the route's physical loss factor and the rest becomes `X / q_d` by the route's
//! selection multiplier `s = p_o (1 + t) tau / q_d`, so a unit of spending buys `1 / q_d` units
//! whatever its origin. [`Sourced::selected`] tallies what selection adds beside the melt in
//! [`Sourced::lost`]; it is negative on a route whose physical cost is below `q_d`.
//!
//! Weights and access are held in the log scales of each country (ADR 0093,
//! [`crate::kernel::Access`]), so a price spread between countries alone makes no
//! destination unreachable: a destination buys nothing only without a path from an active
//! origin, or when every such path underflows (kernel factors near 1e-308). Losses and
//! tariffs are applied by multiplying with the kernel's stored reciprocals (ADR 0097).

use rayon::prelude::*;

use crate::kernel::{Access, Kernel};

/// Provinces per parallel task at least, in loops over provinces.
const ROWS: usize = 64;

/// Origins and origin gateways per parallel task of [`dispatch`] and [`arrivals`], fixed so
/// that their sums do not depend on the thread count.
const ORIGIN_CHUNK: usize = 256;
const GATEWAY_CHUNK: usize = 32;

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
/// Units: `orders`, `shipped`, `home` are origin units, `arrive` delivered units, `X / q_d` per
/// route (ADR 0200). Values are numeraire at origin prices: d pays `p_o (1 + t)` per
/// unit shipped to it, at dispatch. The tariff `t p_o` per unit goes to the importer's rebate
/// account; the rest moves d to b to a to o, so the leg value columns are what moves across
/// each leg in money terms and every gateway pays out exactly what it takes in. The home
/// purchase is in `paid`, `paid_by_class` and `received` but moves no money across legs: it is
/// `p_d` times `arrive[0][d]`.
#[derive(Clone, Debug, Default)]
pub struct Sourced {
    /// Units ordered at each origin by class, at its price `p_o`. Per province.
    pub orders: Vec<[f64; 2]>,
    /// A bound on the rounding of `orders` from products below the normal range, which round
    /// by up to 2^-1075 absolute (trade SPEC, Invariants, Dispatch mass): per product of an
    /// order that carries the origin's weight, 2^-1075 times the factors after it. 0 where every
    /// such product is normal. Per province and class.
    pub orders_rounding: Vec<[f64; 2]>,
    /// Delivered price index `P_d = Phi_d^(-1/theta)` in price units, tariff included;
    /// `f64::INFINITY` where no active origin is reachable. Per province.
    pub price_index: Vec<f64>,
    /// Buyer price `q_d = Gamma_d P_d` (ADR 0200), infinite where `P_d` is. Per province.
    pub buyer_price: Vec<f64>,
    /// Units leaving each origin market, home purchases included: the filled orders. Per
    /// province.
    pub shipped: Vec<f64>,
    /// The bound of `orders_rounding` for `shipped`, from the products of [`dispatch`]'s routes
    /// below the normal range. Per province.
    pub shipped_rounding: Vec<f64>,
    /// Delivered units arriving at each destination by class, by [`ARRIVAL_SLOTS`] slot: what
    /// the class paid on the routes of the slot over `q_d`.
    pub arrive: [Vec<[f64; 2]>; ARRIVAL_SLOTS],
    /// Origin units each province bought at home by class, the shipped units behind `arrive[0]`.
    pub home: Vec<[f64; 2]>,
    /// Shipped units that survive the losses into each destination by [`ARRIVAL_SLOTS`] slot,
    /// both classes: its arrivals before selection.
    pub survived: [Vec<f64>; ARRIVAL_SLOTS],
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
    /// Origin units shipped over each crossing, before any loss: `crossing_value` read in
    /// units at the prices of the origins it came from. Aligned with `crossing_value`.
    pub crossing_units: Vec<f64>,
    /// Money gateway a pays origin o for everything o sent through a. Aligned with
    /// `Kernel::good(g).access.entries` read as origin province to gateway.
    pub access_out_value: Vec<f64>,
    /// Quantity destroyed by iceberg and border losses on every leg, all provinces.
    pub lost: f64,
    /// Delivered units less the shipped units that survive the losses, all routes (ADR 0200):
    /// what selection adds, negative where it takes away.
    pub selected: f64,
    scratch: Scratch,
}

#[derive(Clone, Debug, Default)]
struct Scratch {
    /// Good of the last `source` call.
    good: Option<usize>,
    /// Kernel id, good, prices and active origins the weights, access and price index were
    /// last computed for: they depend on nothing else, so a repeat call reuses them.
    prepared: Option<(u64, usize)>,
    price: Vec<f64>,
    active: Vec<bool>,
    price_index: Vec<f64>,
    /// `1 / q_d`, 0 where `q_d` is infinite.
    inv_q: Vec<f64>,
    /// Log weights and country shifts of [`origin_weights`], and the weights `exp(ln_w)`.
    ln_w: Vec<f64>,
    shift: Vec<f64>,
    weight: Vec<f64>,
    access: Access,
    /// Spending per unit of access by class, `E_d / Phi_d`, in the scale of d's home part
    /// (home and domestic routes) and of its abroad part (through its gateways); 0 where the
    /// part is absent. Both hold `2^-up` of these values: `up` is 0 unless some `E_d / Phi_d`
    /// is beyond f64, though every flow built on it is at most `E_d`.
    reach: Vec<[f64; 2]>,
    reach_abroad: Vec<[f64; 2]>,
    up: i32,
    /// Backward pass, per gateway: value sent to destination gateway b, and value net of
    /// tariff arriving back at origin gateway a.
    to_dest_gateway: Vec<[f64; 2]>,
    at_origin_gateway: Vec<[f64; 2]>,
    /// Forward pass, per gateway: destination access `y_b = sum_d K_bd reach_d` and its
    /// reciprocal, onward access `z_a = sum_b K_ab carry(A, B) y_b` and the share of z_a left
    /// after tariffs.
    y: Vec<f64>,
    inv_y: Vec<f64>,
    z: Vec<f64>,
    net_ratio: Vec<f64>,
    /// Value entering origin gateway a, the same over `p_o tau_oa` (units at a before the
    /// tariff division) and over `p_o` (origin units before the tariff division).
    gross_a: Vec<f64>,
    units_a: Vec<f64>,
    origin_units_a: Vec<f64>,
    /// What enters each destination gateway.
    at_b: Vec<Inflow>,
}

/// Value entering a destination gateway, the same net of tariff, units after the losses by lag
/// and value by lag.
#[derive(Clone, Copy, Debug, Default)]
struct Inflow {
    gross: f64,
    net: f64,
    units: [f64; 4],
    value: [f64; 4],
}

impl Inflow {
    fn add(&mut self, other: &Inflow) {
        self.gross += other.gross;
        self.net += other.net;
        for lag in 0..4 {
            self.units[lag] += other.units[lag];
            self.value[lag] += other.value[lag];
        }
    }
}

/// One chunk of origins of a dispatch pass: what its origins own (shipments, their legs'
/// values), carried on from the pass so far, and what they add to every destination and
/// origin gateway, summed from zero.
struct OriginPart {
    shipped: Vec<f64>,
    rounding: Vec<f64>,
    received: Vec<f64>,
    home: Vec<f64>,
    arrive0: Vec<f64>,
    survived0: Vec<f64>,
    domestic_value: Vec<f64>,
    access_out_value: Vec<f64>,
    paid: Vec<f64>,
    arrive1: Vec<f64>,
    survived1: Vec<f64>,
    gross_a: Vec<f64>,
    units_a: Vec<f64>,
    origin_units_a: Vec<f64>,
    lost: f64,
    selected: f64,
}

/// One chunk of origin gateways of a dispatch pass: its crossings' values and units, carried
/// on, and what it brings to every destination gateway, summed from zero.
struct CrossingPart {
    value: Vec<f64>,
    units: Vec<f64>,
    at_b: Vec<Inflow>,
    lost: f64,
}

impl Sourced {
    pub fn new() -> Self {
        Self::default()
    }
}

/// `x / d` by the reciprocal `inv = 1 / d`, or by division for a subnormal d, whose
/// reciprocal overflows.
fn over(x: f64, inv: f64, d: f64) -> f64 {
    if inv.is_finite() { x * inv } else { x / d }
}

fn zeroed(v: &mut Vec<f64>, n: usize) {
    v.clear();
    v.resize(n, 0.0);
}

fn zeroed2(v: &mut Vec<[f64; 2]>, n: usize) {
    v.clear();
    v.resize(n, [0.0; 2]);
}

/// Log sourcing weights of good `g`, each relative to its country's best (ADR 0093):
/// `ln_w[o] = -theta ln(p_o / p_A)` for an active origin o of country A, `p_A` the lowest
/// active price in A, minus infinity for an inactive origin; and per country the shift
/// `shift[A] = -theta ln(p_A / p_min)`, minus infinity for a country without an active origin.
/// Returns `p_min`, the lowest active price (`f64::INFINITY` when no origin is active). Prices
/// of inactive origins are not read.
pub fn origin_weights(
    k: &Kernel,
    g: usize,
    price: &[f64],
    active: &[bool],
    ln_w: &mut Vec<f64>,
    shift: &mut Vec<f64>,
) -> f64 {
    let np = k.n_provinces();
    assert!(price.len() == np && active.len() == np);
    let (country, theta) = (k.province_country(), k.theta(g));
    // the lowest active price per country first, then its shift in place
    shift.clear();
    shift.resize(k.n_countries(), f64::INFINITY);
    let mut p_min = f64::INFINITY;
    for (o, &p) in price.iter().enumerate() {
        if active[o] {
            assert!(p.is_finite() && p > 0.0, "active market {o} has price {p}");
            let c = usize::from(country[o]);
            shift[c] = shift[c].min(p);
            p_min = p_min.min(p);
        }
    }
    ln_w.clear();
    ln_w.extend((0..np).map(|o| {
        if active[o] {
            -theta * libm::log(price[o] / shift[usize::from(country[o])])
        } else {
            f64::NEG_INFINITY
        }
    }));
    for s in shift.iter_mut() {
        *s = if s.is_finite() {
            -theta * libm::log(*s / p_min)
        } else {
            f64::NEG_INFINITY
        };
    }
    p_min
}

/// Delivered price index `p_min exp(-ln_phi / theta)` from the log access `ln_phi` relative
/// to `p_min^-theta`, a destination's shift plus the log of its access of
/// [`Kernel::origin_access_into`]; infinite for minus infinity (unreachable).
pub fn price_index(p_min: f64, ln_phi: f64, theta: f64) -> f64 {
    if ln_phi > f64::NEG_INFINITY {
        p_min * libm::exp(-ln_phi / theta)
    } else {
        f64::INFINITY
    }
}

/// Spreads the spending `spend[d] = [intermediate, household]` of every destination on good
/// `g` over the active origins, filling `out.orders`, `out.price_index` and `out.buyer_price`
/// (`gamma[d]` times the price index, ADR 0200). `price`, `active` and `gamma` are per
/// province. The split is a backward Markov chain over the gateways: d sends
/// `E_d K_bd u'_b / Phi_d` to destination gateway b, b splits by `K_ab v'_a / u'_b` over
/// origin gateways (the tariff stays at the crossing), a by `K_oa w_o / v'_a` over origins.
/// A destination that reaches no active origin places no orders.
pub fn source(
    k: &Kernel,
    g: usize,
    price: &[f64],
    active: &[bool],
    gamma: &[f64],
    spend: &[[f64; 2]],
    out: &mut Sourced,
) {
    let (np, ng) = (k.n_provinces(), k.n_gateways());
    assert_eq!(spend.len(), np, "spend must have one entry per province");
    assert_eq!(gamma.len(), np, "gamma must have one entry per province");
    assert!(
        gamma.iter().all(|g| g.is_finite() && *g > 0.0),
        "gamma must be finite and positive"
    );
    let gk = k.good(g);
    let s = &mut out.scratch;
    s.good = Some(g);
    let same_prices = s.price.len() == price.len()
        && s.price
            .iter()
            .zip(price)
            .all(|(a, b)| a.to_bits() == b.to_bits());
    if s.prepared != Some((k.id(), g)) || s.active != active || !same_prices {
        let p_min = origin_weights(k, g, price, active, &mut s.ln_w, &mut s.shift);
        s.weight.clear();
        s.weight.extend(s.ln_w.iter().map(|&x| libm::exp(x)));
        k.origin_access_into(g, &s.weight, &s.shift, &mut s.access);
        let acc = &s.access;
        s.price_index.clear();
        s.price_index.extend(
            k.province_country()
                .iter()
                .enumerate()
                .map(|(d, &c)| price_index(p_min, acc.ln_phi(d, usize::from(c)), k.theta(g))),
        );
        s.price.clear();
        s.price.extend_from_slice(price);
        s.active.clear();
        s.active.extend_from_slice(active);
        s.prepared = Some((k.id(), g));
    }
    let (w, v, u) = (&s.weight, &s.access.v, &s.access.u);
    let (gc, nc, carry) = (k.gateway_country(), k.n_countries(), &s.access.carry);

    out.price_index.clone_from(&s.price_index);
    out.buyer_price.clear();
    out.buyer_price
        .extend(s.price_index.iter().zip(gamma).map(|(p, g)| g * p));
    s.inv_q.clear();
    s.inv_q.extend(out.buyer_price.iter().map(|q| 1.0 / q));
    s.up = 0;
    fill_reach(k, spend, &s.access, 0, &mut s.reach, &mut s.reach_abroad);
    if s.reach
        .iter()
        .chain(&s.reach_abroad)
        .flatten()
        .any(|x| !x.is_finite())
    {
        s.up = reach_exponent(k, spend, &s.access);
        fill_reach(k, spend, &s.access, s.up, &mut s.reach, &mut s.reach_abroad);
    }
    let (reach, reach_abroad, up) = (&s.reach, &s.reach_abroad, s.up);

    // Orders are accumulated as value net of tariff and turned into units at the end. Both
    // classes go through each loop together; every accumulator is per class, so each sees
    // the additions of a pass of its own. Loops over origins and over origin gateways write
    // only their own entries and run in parallel.
    let (orders, rounding) = (&mut out.orders, &mut out.orders_rounding);
    zeroed2(orders, np);
    zeroed2(rounding, np);
    // home and domestic routes
    (orders.par_iter_mut(), rounding.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .with_min_len(ROWS)
        .filter(|(o, _)| w[*o] > 0.0)
        .for_each(|(o, (order, round))| {
            let mut x = [w[o] * reach[o][0], w[o] * reach[o][1]];
            for c in 0..2 {
                round[c] += below_normal(x[c]);
            }
            for e in gk.domestic.row(o) {
                let (kw, r) = (e.k * w[o], reach[e.col as usize]);
                x[0] += kw * r[0];
                x[1] += kw * r[1];
                for c in 0..2 {
                    round[c] += below_normal(kw) * r[c] + below_normal(kw * r[c]);
                }
            }
            order[0] += x[0];
            order[1] += x[1];
        });
    // destinations to their gateways
    let to_b = &mut s.to_dest_gateway;
    zeroed2(to_b, ng);
    for (d, r) in reach_abroad.iter().enumerate() {
        let on = [r[0] > 0.0, r[1] > 0.0];
        if !(on[0] || on[1]) {
            continue;
        }
        for e in gk.access.row(d) {
            let b = e.col as usize;
            for c in (0..2).filter(|&c| on[c]) {
                to_b[b][c] += r[c] * e.k * u[b];
            }
        }
    }
    let to_b = &s.to_dest_gateway;
    // across the border, net of tariff
    zeroed2(&mut s.at_origin_gateway, ng);
    s.at_origin_gateway
        .par_iter_mut()
        .enumerate()
        .filter(|(a, _)| v[*a] > 0.0)
        .for_each(|(a, at_a)| {
            let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
            for e in gk.crossings.row(a) {
                let b = e.col as usize;
                let t = to_b[b];
                if t == [0.0; 2] {
                    continue;
                }
                let share = e.k * carry_a[usize::from(gc[b])] * v[a] / u[b];
                for c in (0..2).filter(|&c| t[c] != 0.0) {
                    at_a[c] += t[c] * share * e.inv_duty;
                }
            }
        });
    let at_a = &s.at_origin_gateway;
    // origin gateways to origins, then value to units
    (orders.par_iter_mut(), rounding.par_iter_mut())
        .into_par_iter()
        .enumerate()
        .with_min_len(ROWS)
        .filter(|(o, _)| w[*o] > 0.0)
        .for_each(|(o, (order, round))| {
            for e in gk.access.row(o) {
                let a = e.col as usize;
                let t = at_a[a];
                if t == [0.0; 2] {
                    continue;
                }
                let kw = e.k * w[o];
                let factor = kw / v[a];
                for c in (0..2).filter(|&c| t[c] != 0.0) {
                    order[c] += t[c] * factor;
                    round[c] += (below_normal(kw) / v[a] + below_normal(factor)) * t[c]
                        + below_normal(t[c] * factor);
                }
            }
            for (x, r) in order.iter_mut().zip(round) {
                let units = *x / price[o];
                *r = times_two_to(*r, up) / price[o] + times_two_to(below_normal(units), up);
                *x = times_two_to(units, up);
            }
        });
}

/// The smallest subnormal f64, 2^-1074, where `v` is below the normal range (0 included, as a
/// product may underflow to it), else 0: the most a product rounds to that range loses,
/// absolute (trade SPEC, Invariants, Dispatch mass).
fn below_normal(v: f64) -> f64 {
    if v.abs() < f64::MIN_POSITIVE {
        f64::from_bits(1)
    } else {
        0.0
    }
}

/// `x 2^n`, exact unless it leaves the normal range.
fn times_two_to(x: f64, n: i32) -> f64 {
    if n == 0 { x } else { libm::scalbn(x, n) }
}

/// Writes `2^-up E_d / Phi_d` per class for the home and abroad part of every destination.
fn fill_reach(
    k: &Kernel,
    spend: &[[f64; 2]],
    access: &Access,
    up: i32,
    reach: &mut Vec<[f64; 2]>,
    reach_abroad: &mut Vec<[f64; 2]>,
) {
    reach.clear();
    reach_abroad.clear();
    for (d, &c) in k.province_country().iter().enumerate() {
        let (home, abroad) = access.reach(d, usize::from(c));
        let e = spend[d].map(|e| times_two_to(e, -up));
        reach.push([e[0] * home, e[1] * home]);
        reach_abroad.push([e[0] * abroad, e[1] * abroad]);
    }
}

/// The binary exponent that brings every `E_d / Phi_d` to at most about 2^961, leaving
/// headroom for the sums over destinations and gateways built on them.
fn reach_exponent(k: &Kernel, spend: &[[f64; 2]], access: &Access) -> i32 {
    let mut top = i32::MIN;
    for (d, &c) in k.province_country().iter().enumerate() {
        let (home, abroad) = access.reach(d, usize::from(c));
        for at in [home, abroad].into_iter().filter(|&at| at > 0.0) {
            for &e in spend[d].iter().filter(|&&e| e > 0.0) {
                top = top.max(libm::ilogb(e) + libm::ilogb(at) + 1);
            }
        }
    }
    top - 960
}

/// Dispatches the filled orders of the preceding [`source`] call for the same good: origin o
/// ships `fill[o][c]` of its class-c orders along the routes they were placed on. `price` is
/// the one passed to `source`. The forward pass is the backward split read the other way:
/// origin o sends value `fill w_o K_oa z_a` to gateway a, a splits by `K_ab y_b / z_a`, b by
/// `K_bd reach_d / y_b`, with `reach_d = E_d / Phi_d`. Each leg multiplies the units by its
/// stored reciprocal physical factor (iceberg, and the border loss on crossings), and a
/// crossing the value by its `1 / (1 + t)`, so the units that survive the melt are the sum over
/// routes of `X / (p_o (1 + t) tau_route)`; value is carried by arrival slot alongside, and
/// arrivals are `X / q_d` per route (ADR 0200). Divisions are by reciprocals hoisted per origin, per
/// origin gateway and per destination gateway. Each good is one serial pass: the tick runs the
/// goods in parallel.
pub fn dispatch(k: &Kernel, g: usize, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced) {
    let (np, ng) = (k.n_provinces(), k.n_gateways());
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
        shipped_rounding,
        arrive,
        home,
        survived,
        paid,
        paid_by_class,
        received,
        tariff,
        domestic_value,
        access_in_value,
        crossing_value,
        crossing_units,
        access_out_value,
        lost,
        selected,
        scratch: s,
        ..
    } = out;
    zeroed(shipped, np);
    zeroed(shipped_rounding, np);
    arrive.iter_mut().for_each(|a| zeroed2(a, np));
    zeroed2(home, np);
    survived.iter_mut().for_each(|v| zeroed(v, np));
    zeroed(paid, np);
    zeroed2(paid_by_class, np);
    zeroed(received, np);
    zeroed(tariff, np);
    zeroed(domestic_value, gk.domestic.entries.len());
    zeroed(access_in_value, gk.access.entries.len());
    zeroed(crossing_value, gk.crossings.entries.len());
    zeroed(crossing_units, gk.crossings.entries.len());
    zeroed(access_out_value, gk.access.entries.len());
    *lost = 0.0;
    *selected = 0.0;

    let (w, reach, reach_abroad, inv_q) = (&s.weight, &s.reach, &s.reach_abroad, &s.inv_q);
    let (gc, nc, carry) = (k.gateway_country(), k.n_countries(), &s.access.carry);
    for c in 0..2 {
        // a class that spends nothing anywhere ships nothing: its pass would add only zeros
        if reach.iter().chain(reach_abroad).all(|r| r[c] == 0.0) {
            continue;
        }
        // destination-side access sums on the spending weights of this class
        zeroed(&mut s.y, ng);
        for d in (0..np).filter(|&d| reach_abroad[d][c] > 0.0) {
            for e in gk.access.row(d) {
                s.y[e.col as usize] += e.k * reach_abroad[d][c];
            }
        }
        s.inv_y.clear();
        s.inv_y.extend(s.y.iter().map(|y| 1.0 / y));
        zeroed(&mut s.z, ng);
        zeroed(&mut s.net_ratio, ng);
        for a in 0..ng {
            let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
            let (mut gross, mut net) = (0.0, 0.0);
            for e in gk.crossings.row(a) {
                let b = e.col as usize;
                let x = e.k * carry_a[usize::from(gc[b])] * s.y[b];
                gross += x;
                net += x * e.inv_duty;
            }
            s.z[a] = gross;
            if gross > 0.0 {
                s.net_ratio[a] = net / gross;
            }
        }

        // Origins: home, domestic and into the first gateway, in fixed chunks of origins in
        // parallel. What an origin owns (its shipments, its legs) is carried on from the
        // previous class in the same order; what it adds to destinations and gateways is
        // summed per chunk and the chunks are added in order.
        let parts: Vec<OriginPart> = (0..np.div_ceil(ORIGIN_CHUNK))
            .into_par_iter()
            .map(|k| {
                let os = k * ORIGIN_CHUNK..((k + 1) * ORIGIN_CHUNK).min(np);
                let dom =
                    gk.domestic.offsets[os.start] as usize..gk.domestic.offsets[os.end] as usize;
                let acc = gk.access.offsets[os.start] as usize..gk.access.offsets[os.end] as usize;
                let mut p = OriginPart {
                    shipped: shipped[os.clone()].to_vec(),
                    rounding: shipped_rounding[os.clone()].to_vec(),
                    received: received[os.clone()].to_vec(),
                    home: home[os.clone()].iter().map(|h| h[c]).collect(),
                    arrive0: arrive[0][os.clone()].iter().map(|x| x[c]).collect(),
                    survived0: survived[0][os.clone()].to_vec(),
                    domestic_value: domestic_value[dom.clone()].to_vec(),
                    access_out_value: access_out_value[acc.clone()].to_vec(),
                    paid: vec![0.0; np],
                    arrive1: vec![0.0; np],
                    survived1: vec![0.0; np],
                    gross_a: vec![0.0; ng],
                    units_a: vec![0.0; ng],
                    origin_units_a: vec![0.0; ng],
                    lost: 0.0,
                    selected: 0.0,
                };
                for o in os.clone() {
                    let j = o - os.start;
                    let f = fill[o][c];
                    if w[o] == 0.0 || f == 0.0 {
                        continue;
                    }
                    // reach is 2^-up of E_d / Phi_d; every flow below is linear in m
                    let (m, inv_p) = (times_two_to(f * w[o], s.up), 1.0 / price[o]);
                    // the rounding of m, then of each route's products below the normal range,
                    // times the factors after them, in units (`Sourced::shipped_rounding`)
                    let dm = times_two_to(below_normal(f * w[o]), s.up) + below_normal(m);
                    let units_of = |value: f64, err: f64| {
                        (err + below_normal(value)) * inv_p + below_normal(value * inv_p)
                    };
                    let x = m * reach[o][c];
                    p.rounding[j] += units_of(x, dm * reach[o][c]);
                    if x > 0.0 {
                        let units = x * inv_p;
                        let got = x * inv_q[o];
                        p.arrive0[j] += got;
                        p.home[j] += units;
                        p.survived0[j] += units;
                        p.selected += got - units;
                        p.shipped[j] += units;
                        p.paid[o] += x;
                        p.received[j] += x;
                    }
                    let lo = gk.domestic.offsets[o] as usize - dom.start;
                    for (i, e) in gk.domestic.row(o).iter().enumerate() {
                        let d = e.col as usize;
                        let x = m * e.k * reach[d][c];
                        let mk = (dm * e.k + below_normal(m * e.k)) * reach[d][c];
                        p.rounding[j] += units_of(x, mk);
                        if x == 0.0 {
                            continue;
                        }
                        let units = x * inv_p;
                        let melted = units * e.inv_tau;
                        let got = x * inv_q[d];
                        p.arrive1[d] += got;
                        p.survived1[d] += melted;
                        p.lost += units - melted;
                        p.selected += got - melted;
                        p.shipped[j] += units;
                        p.domestic_value[lo + i] += x;
                        p.paid[d] += x;
                        p.received[j] += x;
                    }
                    let lo = gk.access.offsets[o] as usize - acc.start;
                    for (i, e) in gk.access.row(o).iter().enumerate() {
                        let a = e.col as usize;
                        let x = m * e.k * s.z[a];
                        let mk = ((dm * e.k + below_normal(m * e.k)) * s.z[a] + below_normal(x))
                            * s.net_ratio[a];
                        p.rounding[j] += units_of(x * s.net_ratio[a], mk);
                        if x == 0.0 {
                            continue;
                        }
                        let net = x * s.net_ratio[a];
                        let units = net * inv_p;
                        p.access_out_value[lo + i] += net;
                        p.received[j] += net;
                        p.shipped[j] += units;
                        p.lost += units - units * e.inv_tau;
                        let origin_units = x * inv_p;
                        p.gross_a[a] += x;
                        p.units_a[a] += origin_units * e.inv_tau;
                        p.origin_units_a[a] += origin_units;
                    }
                }
                p
            })
            .collect();
        zeroed(&mut s.gross_a, ng);
        zeroed(&mut s.units_a, ng);
        zeroed(&mut s.origin_units_a, ng);
        for (k, p) in parts.iter().enumerate() {
            let o0 = k * ORIGIN_CHUNK;
            let n = p.shipped.len();
            shipped[o0..o0 + n].copy_from_slice(&p.shipped);
            shipped_rounding[o0..o0 + n].copy_from_slice(&p.rounding);
            received[o0..o0 + n].copy_from_slice(&p.received);
            survived[0][o0..o0 + n].copy_from_slice(&p.survived0);
            for j in 0..n {
                home[o0 + j][c] = p.home[j];
                arrive[0][o0 + j][c] = p.arrive0[j];
            }
            let dom = gk.domestic.offsets[o0] as usize;
            domestic_value[dom..dom + p.domestic_value.len()].copy_from_slice(&p.domestic_value);
            let acc = gk.access.offsets[o0] as usize;
            access_out_value[acc..acc + p.access_out_value.len()]
                .copy_from_slice(&p.access_out_value);
            *lost += p.lost;
            *selected += p.selected;
            for a in 0..ng {
                s.gross_a[a] += p.gross_a[a];
                s.units_a[a] += p.units_a[a];
                s.origin_units_a[a] += p.origin_units_a[a];
            }
        }
        // destination sums over the chunks in chunk order, destinations in parallel
        (
            paid.par_iter_mut(),
            paid_by_class.par_iter_mut(),
            arrive[1].par_iter_mut(),
            survived[1].par_iter_mut(),
        )
            .into_par_iter()
            .enumerate()
            .with_min_len(ROWS)
            .for_each(|(d, (paid, by_class, arrive, survived))| {
                for p in &parts {
                    *paid += p.paid[d];
                    by_class[c] += p.paid[d];
                    arrive[c] += p.arrive1[d];
                    *survived += p.survived1[d];
                }
            });

        // Origin gateways onto crossings, in fixed chunks of gateways in parallel; units are
        // kept by transit lag from here on.
        let (gross_a, units_a, origin_units_a) = (&s.gross_a, &s.units_a, &s.origin_units_a);
        let (y, z) = (&s.y, &s.z);
        let parts: Vec<CrossingPart> = (0..ng.div_ceil(GATEWAY_CHUNK))
            .into_par_iter()
            .map(|k| {
                let gs = k * GATEWAY_CHUNK..((k + 1) * GATEWAY_CHUNK).min(ng);
                let legs =
                    gk.crossings.offsets[gs.start] as usize..gk.crossings.offsets[gs.end] as usize;
                let mut p = CrossingPart {
                    value: crossing_value[legs.clone()].to_vec(),
                    units: crossing_units[legs.clone()].to_vec(),
                    at_b: vec![Inflow::default(); ng],
                    lost: 0.0,
                };
                for a in gs.filter(|&a| gross_a[a] > 0.0) {
                    let lo = gk.crossings.offsets[a] as usize - legs.start;
                    let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
                    let inv_z = 1.0 / z[a];
                    for (i, e) in gk.crossings.row(a).iter().enumerate() {
                        let b = e.col as usize;
                        let weight = e.k * carry_a[usize::from(gc[b])] * y[b];
                        if weight == 0.0 {
                            continue;
                        }
                        let share = over(weight, inv_z, z[a]);
                        let gross = gross_a[a] * share;
                        let net = gross * e.inv_duty;
                        let units = units_a[a] * share * e.inv_duty;
                        let got = units * e.inv_loss;
                        p.value[lo + i] += net;
                        p.units[lo + i] += origin_units_a[a] * share * e.inv_duty;
                        p.lost += units - got;
                        let at = &mut p.at_b[b];
                        at.units[usize::from(e.lag)] += got;
                        at.value[usize::from(e.lag)] += gross;
                        at.gross += gross;
                        at.net += net;
                    }
                }
                p
            })
            .collect();
        s.at_b.clear();
        s.at_b.resize(ng, Inflow::default());
        for (k, p) in parts.iter().enumerate() {
            let lo = gk.crossings.offsets[k * GATEWAY_CHUNK] as usize;
            crossing_value[lo..lo + p.value.len()].copy_from_slice(&p.value);
            crossing_units[lo..lo + p.units.len()].copy_from_slice(&p.units);
            *lost += p.lost;
            for (at, part) in s.at_b.iter_mut().zip(&p.at_b) {
                at.add(part);
            }
        }

        // Destination gateways to destinations.
        for d in (0..np).filter(|&d| reach_abroad[d][c] > 0.0) {
            let lo = gk.access.offsets[d] as usize;
            for (i, e) in gk.access.row(d).iter().enumerate() {
                let b = e.col as usize;
                let weight = e.k * reach_abroad[d][c];
                if weight == 0.0 {
                    continue;
                }
                let share = over(weight, s.inv_y[b], s.y[b]);
                let at = &s.at_b[b];
                for (lag, (&units, &value)) in at.units.iter().zip(&at.value).enumerate() {
                    let units = units * share;
                    let melted = units * e.inv_tau;
                    let got = value * share * inv_q[d];
                    arrive[1 + lag][d][c] += got;
                    survived[1 + lag][d] += melted;
                    *lost += units - melted;
                    *selected += got - melted;
                }
                let gross = at.gross * share;
                let net = at.net * share;
                access_in_value[lo + i] += net;
                paid[d] += gross;
                paid_by_class[d][c] += gross;
                tariff[d] += gross - net;
            }
        }
    }
}

/// [`dispatch`] at full fill (every origin ships all its orders) reduced to what it writes in
/// `out.arrive`, which this leaves exactly as that dispatch would: the same products summed in
/// the same order. The other outputs of `dispatch` are left as they were. For the delivered
/// prices and lead times of the unit spending, which read only the arrivals.
pub fn arrivals(k: &Kernel, g: usize, out: &mut Sourced) {
    let (np, ng) = (k.n_provinces(), k.n_gateways());
    assert_eq!(
        out.scratch.good,
        Some(g),
        "arrivals must follow source for good {g}"
    );
    let gk = k.good(g);
    let Sourced {
        arrive, scratch: s, ..
    } = out;
    arrive.iter_mut().for_each(|a| zeroed2(a, np));
    let (w, reach, reach_abroad, inv_q) = (&s.weight, &s.reach, &s.reach_abroad, &s.inv_q);
    let (gc, nc, carry) = (k.gateway_country(), k.n_countries(), &s.access.carry);
    for c in 0..2 {
        if reach.iter().chain(reach_abroad).all(|r| r[c] == 0.0) {
            continue;
        }
        zeroed(&mut s.y, ng);
        for d in (0..np).filter(|&d| reach_abroad[d][c] > 0.0) {
            for e in gk.access.row(d) {
                s.y[e.col as usize] += e.k * reach_abroad[d][c];
            }
        }
        s.inv_y.clear();
        s.inv_y.extend(s.y.iter().map(|y| 1.0 / y));
        zeroed(&mut s.z, ng);
        for a in 0..ng {
            let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
            let mut gross = 0.0;
            for e in gk.crossings.row(a) {
                gross += e.k * carry_a[usize::from(gc[e.col as usize])] * s.y[e.col as usize];
            }
            s.z[a] = gross;
        }

        // the chunks and the order of their sums are those of `dispatch`
        let parts: Vec<(Vec<f64>, Vec<f64>)> = (0..np.div_ceil(ORIGIN_CHUNK))
            .into_par_iter()
            .map(|k| {
                let (mut arrive1, mut gross_a) = (vec![0.0; np], vec![0.0; ng]);
                let os = k * ORIGIN_CHUNK..((k + 1) * ORIGIN_CHUNK).min(np);
                for o in os.filter(|&o| w[o] != 0.0) {
                    let m = times_two_to(w[o], s.up);
                    for e in gk.domestic.row(o) {
                        let d = e.col as usize;
                        let x = m * e.k * reach[d][c];
                        if x != 0.0 {
                            arrive1[d] += x * inv_q[d];
                        }
                    }
                    for e in gk.access.row(o) {
                        let a = e.col as usize;
                        let x = m * e.k * s.z[a];
                        if x != 0.0 {
                            gross_a[a] += x;
                        }
                    }
                }
                (arrive1, gross_a)
            })
            .collect();
        for o in (0..np).filter(|&o| w[o] != 0.0) {
            let x = times_two_to(w[o], s.up) * reach[o][c];
            if x > 0.0 {
                arrive[0][o][c] += x * inv_q[o];
            }
        }
        zeroed(&mut s.gross_a, ng);
        for (arrive1, gross_a) in &parts {
            for (a, x) in arrive[1].iter_mut().zip(arrive1) {
                a[c] += x;
            }
            for (a, x) in s.gross_a.iter_mut().zip(gross_a) {
                *a += x;
            }
        }

        let (gross_a, y, z) = (&s.gross_a, &s.y, &s.z);
        let parts: Vec<Vec<[f64; 4]>> = (0..ng.div_ceil(GATEWAY_CHUNK))
            .into_par_iter()
            .map(|k| {
                let mut value = vec![[0.0; 4]; ng];
                let gs = k * GATEWAY_CHUNK..((k + 1) * GATEWAY_CHUNK).min(ng);
                for a in gs.filter(|&a| gross_a[a] > 0.0) {
                    let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
                    let inv_z = 1.0 / z[a];
                    for e in gk.crossings.row(a) {
                        let b = e.col as usize;
                        let weight = e.k * carry_a[usize::from(gc[b])] * y[b];
                        if weight == 0.0 {
                            continue;
                        }
                        value[b][usize::from(e.lag)] += gross_a[a] * over(weight, inv_z, z[a]);
                    }
                }
                value
            })
            .collect();
        s.at_b.clear();
        s.at_b.resize(ng, Inflow::default());
        for value in &parts {
            for (at, v) in s.at_b.iter_mut().zip(value) {
                at.value.iter_mut().zip(v).for_each(|(a, x)| *a += x);
            }
        }

        for d in (0..np).filter(|&d| reach_abroad[d][c] > 0.0) {
            for e in gk.access.row(d) {
                let b = e.col as usize;
                let weight = e.k * reach_abroad[d][c];
                if weight == 0.0 {
                    continue;
                }
                let share = over(weight, s.inv_y[b], s.y[b]);
                for (lag, &value) in s.at_b[b].value.iter().enumerate() {
                    arrive[1 + lag][d][c] += value * share * inv_q[d];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goods::Goods;
    use crate::goods::p2::N as N_GOODS;
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
    /// to 3), random gateway masses and random tariffs.
    fn complete_world(rng: &mut Rng, provinces: Vec<u16>, gateways: Vec<u16>) -> (Network, Wedges) {
        let n_countries = usize::from(*provinces.iter().chain(&gateways).max().unwrap()) + 1;
        let raw: Vec<f64> = gateways.iter().map(|_| 0.2 + rng.next()).collect();
        let mut total = vec![0.0; n_countries];
        for (&c, m) in gateways.iter().zip(&raw) {
            total[usize::from(c)] += m;
        }
        let gateway_mass = gateways
            .iter()
            .zip(&raw)
            .map(|(&c, m)| m / total[usize::from(c)])
            .collect();
        let mut net = Network {
            province_country: provinces,
            gateway_country: gateways,
            gateway_mass,
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

    /// Kernel at `theta` for every good.
    fn kernel_at(net: &Network, wedges: &Wedges, theta: f64) -> Kernel {
        let p = TradeParams {
            theta: vec![theta; N_GOODS],
            ..TradeParams::default().bulk_off()
        };
        Kernel::build(net, &W, wedges, &p)
    }

    /// Kernel at theta = [`THETA`] for every good, the value most oracles below use.
    fn kernel(net: &Network, wedges: &Wedges) -> Kernel {
        kernel_at(net, wedges, THETA)
    }

    fn tau_leg(g: usize, l: &Leg) -> f64 {
        1.0 + l.cost_per_t * W[g] + TradeParams::default().time_cost[g] * l.hours / 24.0
    }

    /// K of a leg by the spec formula.
    fn k_leg(g: usize, l: &Leg, extra: f64, theta: f64) -> f64 {
        f64::powf(tau_leg(g, l) * extra, -theta)
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
    fn hops(net: &Network, wedges: &Wedges, g: usize, o: usize, d: usize, theta: f64) -> Vec<Hop> {
        let nu = 1.0 + TradeParams::default().border_nu_of(g);
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
                k: k_leg(g, l, 1.0, theta),
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
                // a province without an access leg to a gateway has no route through it
                let leg = |legs: &[Leg], from: usize, to: usize| {
                    legs.iter()
                        .find(|l| (l.from as usize, l.to as usize) == (from, to))
                        .copied()
                };
                let (Some(oa), Some(bd)) = (leg(&net.access, o, a), leg(&net.access, d, b)) else {
                    continue;
                };
                let ab = find(&net.crossings, a, b);
                let t =
                    f64::from(wedges.rate(cd as usize, co as usize, Goods::p2().tariff_group(g)));
                let m = net.gateway_mass[a] * net.gateway_mass[b];
                hs.push(Hop {
                    k: k_leg(g, &oa, 1.0, theta)
                        * m
                        * k_leg(g, ab, nu * (1.0 + t), theta)
                        * k_leg(g, &bd, 1.0, theta),
                    tau: tau_leg(g, &oa) * tau_leg(g, ab) * nu * tau_leg(g, &bd),
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
            f64::from(wedges.rate(cd as usize, co as usize, Goods::p2().tariff_group(g)))
        }
    }

    /// Selection constants of the tests, one per province, spread around 1 (ADR 0200).
    fn gammas(n: usize) -> Vec<f64> {
        (0..n).map(|d| 0.9 + 0.07 * (d % 5) as f64).collect()
    }

    #[derive(Default)]
    struct Dense {
        orders: Vec<[f64; 2]>,
        price_index: Vec<f64>,
        buyer_price: Vec<f64>,
        home: Vec<[f64; 2]>,
        survived: [Vec<f64>; 5],
        shipped: Vec<f64>,
        arrive: [Vec<[f64; 2]>; 5],
        paid: Vec<f64>,
        received: Vec<f64>,
        tariff: Vec<f64>,
        lost: f64,
        selected: f64,
    }

    /// ADR 0015 on a dense route table, independent of the Markov split and of the country
    /// scales: d spends E_d over routes by K_route p_o^-theta / Phi_d, in log space (each
    /// route's ln K + ln w, ln Phi_d by log-sum-exp per destination), so no price spread
    /// underflows; the origin ships X / (p_o (1 + t)) units, which melt by the physical tau; d
    /// receives X / q_d, q_d = gamma_d P_d with [`gammas`], pays X and the tariff takes
    /// t / (1 + t).
    #[allow(clippy::too_many_arguments)]
    fn dense(
        net: &Network,
        wedges: &Wedges,
        g: usize,
        price: &[f64],
        active: &[bool],
        spend: &[[f64; 2]],
        fill: &[[f64; 2]],
        theta: f64,
    ) -> Dense {
        let n = net.n_provinces();
        let p_min = (0..n)
            .filter(|&o| active[o])
            .map(|o| price[o])
            .fold(f64::INFINITY, f64::min);
        let ln_w: Vec<f64> = (0..n)
            .map(|o| {
                if active[o] {
                    -theta * (price[o] / p_min).ln()
                } else {
                    f64::NEG_INFINITY
                }
            })
            .collect();
        let gamma = gammas(n);
        let mut r = Dense {
            orders: vec![[0.0; 2]; n],
            price_index: vec![f64::INFINITY; n],
            buyer_price: vec![f64::INFINITY; n],
            home: vec![[0.0; 2]; n],
            survived: std::array::from_fn(|_| vec![0.0; n]),
            shipped: vec![0.0; n],
            arrive: std::array::from_fn(|_| vec![[0.0; 2]; n]),
            paid: vec![0.0; n],
            received: vec![0.0; n],
            tariff: vec![0.0; n],
            lost: 0.0,
            selected: 0.0,
        };
        for (d, sd) in spend.iter().enumerate() {
            let all: Vec<Vec<Hop>> = (0..n).map(|o| hops(net, wedges, g, o, d, theta)).collect();
            let terms: Vec<(usize, &Hop, f64)> = (0..n)
                .flat_map(|o| all[o].iter().map(move |h| (o, h)))
                .map(|(o, h)| (o, h, ln_w[o] + h.k.ln()))
                .filter(|t| t.2 > f64::NEG_INFINITY)
                .collect();
            let top = terms.iter().map(|t| t.2).fold(f64::NEG_INFINITY, f64::max);
            if top == f64::NEG_INFINITY {
                continue;
            }
            let ln_phi = top + terms.iter().map(|t| (t.2 - top).exp()).sum::<f64>().ln();
            r.price_index[d] = p_min * (-ln_phi / theta).exp();
            r.buyer_price[d] = gamma[d] * r.price_index[d];
            for &(o, h, term) in &terms {
                let share = (term - ln_phi).exp();
                for c in 0..2 {
                    let x = sd[c] * share;
                    let units = x / (1.0 + h.tariff) / price[o];
                    r.orders[o][c] += units;
                    let (xf, uf) = (x * fill[o][c], units * fill[o][c]);
                    let melted = uf / h.tau;
                    let got = xf / r.buyer_price[d];
                    r.shipped[o] += uf;
                    r.arrive[h.slot][d][c] += got;
                    if h.slot == 0 {
                        r.home[o][c] += uf;
                    }
                    r.survived[h.slot][d] += melted;
                    r.lost += uf - melted;
                    r.selected += got - melted;
                    r.paid[d] += xf;
                    r.received[o] += xf / (1.0 + h.tariff);
                    r.tariff[d] += xf - xf / (1.0 + h.tariff);
                }
            }
        }
        r
    }

    fn close(a: f64, b: f64, tol: f64) -> bool {
        a == b || (a - b).abs() <= tol * a.abs().max(b.abs()) + 1e-300
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
        source(k, g, price, active, &gammas(price.len()), spend, &mut out);
        dispatch(k, g, price, fill, &mut out);
        out
    }

    /// The buyer price is gamma P_d: a gamma of 0 would make it 0 (and NaN or a negative gamma
    /// a NaN or negative price), so it is refused.
    #[test]
    #[should_panic(expected = "gamma must be finite and positive")]
    fn source_refuses_a_gamma_that_is_not_positive() {
        let (net, wedges) = random_world(3);
        let k = kernel(&net, &wedges);
        let np = net.n_provinces();
        let mut gamma = gammas(np);
        gamma[0] = 0.0;
        let mut out = Sourced::new();
        let (price, spend) = (vec![1.0; np], vec![[1.0, 1.0]; np]);
        source(&k, 0, &price, &vec![true; np], &gamma, &spend, &mut out);
    }

    #[test]
    fn arrivals_are_the_arrivals_of_a_full_dispatch() {
        // the last world has several chunks of origins and of gateways
        for seed in 0..9 {
            let (net, wedges) = if seed < 8 {
                random_world(seed)
            } else {
                let provinces = (0..600).map(|i| (i / 200) as u16).collect();
                let gateways = (0..40).map(|i| (i / 8) as u16).collect();
                complete_world(&mut rng(seed), provinces, gateways)
            };
            let k = kernel(&net, &wedges);
            let np = net.n_provinces();
            let mut r = rng(seed + 100);
            let price = draw(&mut r, np, 0.5, 2.0);
            let active: Vec<bool> = (0..np).map(|o| o % 5 != 2).collect();
            // one class without spending, as in the unit spending of the price refresh
            let spend: Vec<[f64; 2]> = (0..np)
                .map(|_| [r.next() * 10.0, if seed % 2 == 0 { 0.0 } else { r.next() }])
                .collect();
            for g in Goods::p2().traded_goods() {
                let full = run(&k, g, &price, &active, &spend, &vec![[1.0; 2]; np]);
                let mut out = Sourced::new();
                source(&k, g, &price, &active, &gammas(np), &spend, &mut out);
                arrivals(&k, g, &mut out);
                let bits = |a: &[Vec<[f64; 2]>]| -> Vec<u64> {
                    a.iter().flatten().flatten().map(|x| x.to_bits()).collect()
                };
                assert_eq!(
                    bits(&full.arrive),
                    bits(&out.arrive),
                    "seed {seed} good {g}"
                );
            }
        }
    }

    #[test]
    fn parallel_splits_do_not_depend_on_the_thread_count() {
        // enough provinces and gateways that the loops over them split into several tasks
        let provinces = (0..300).map(|i| (i / 100) as u16).collect();
        let gateways = (0..40).map(|i| (i / 8) as u16).collect();
        let mut r = rng(7);
        let (net, wedges) = complete_world(&mut r, provinces, gateways);
        let k = kernel(&net, &wedges);
        let np = net.n_provinces();
        assert!(np > 4 * ROWS && np > ORIGIN_CHUNK && net.n_gateways() > GATEWAY_CHUNK);
        let price = draw(&mut r, np, 0.5, 2.0);
        let active: Vec<bool> = (0..np).map(|o| o % 7 != 3).collect();
        let spend = draw2(&mut r, np, 0.0, 10.0);
        let fill = draw2(&mut r, np, 0.2, 1.0);
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            pool.install(|| {
                let mut out = Sourced::new();
                source(&k, 4, &price, &active, &gammas(np), &spend, &mut out);
                dispatch(&k, 4, &price, &fill, &mut out);
                format!("{out:?}")
            })
        };
        // Debug prints every f64 in its shortest round-trip form, one per bit pattern
        assert_eq!(run(1), run(4));
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
                source(
                    &k,
                    g,
                    &price,
                    &active,
                    &gammas(price.len()),
                    &spend,
                    &mut out,
                );
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
                source(
                    &k,
                    g,
                    &price,
                    &active,
                    &gammas(price.len()),
                    &spend,
                    &mut out,
                );
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
        source(&k, g, &price, &[true; 3], &[1.0; 3], &spend, &mut out);

        let kk = |o: usize, d: usize| {
            if o == d {
                1.0
            } else {
                k_leg(g, find(&net.domestic, o, d), 1.0, THETA)
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
                source(
                    &k,
                    g,
                    &price,
                    &active,
                    &gammas(price.len()),
                    &spend,
                    &mut out,
                );
                let want = dense(&net, &wedges, g, &price, &active, &spend, &fill, THETA);
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
                let want = dense(&net, &wedges, g, &price, &active, &spend, &fill, THETA);
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
                let buyer = &want.buyer_price;
                assert_close(&tag("buyer price"), &out.buyer_price, buyer, 1e-10);
                for s in 0..5 {
                    let (got, want) = (&out.survived[s], &want.survived[s]);
                    assert_close(&tag(&format!("survived[{s}]")), got, want, 1e-10);
                }
                let scale: f64 = want.arrive.iter().flatten().flatten().sum();
                let gap = (out.selected - want.selected).abs();
                assert!(gap <= 1e-10 * scale, "{}", tag("selected"));
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
                source(
                    &k,
                    g,
                    &price,
                    &active,
                    &[1.0; 10],
                    &[[1.0; 2]; 10],
                    &mut out,
                );
                for d in 0..10 {
                    let want: f64 = (0..10)
                        .filter(|&o| active[o])
                        .map(|o| {
                            let k_od: f64 = hops(&net, &wedges, g, o, d, THETA)
                                .iter()
                                .map(|h| h.k)
                                .sum();
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
        // per good, each at its own default theta
        let p = TradeParams::default().bulk_off();
        for (seed, g) in (0..7).map(|g| (3 + 14 * g as u64, g)) {
            let (net, wedges) = random_world(seed);
            let n = net.n_provinces();
            let k = Kernel::build(&net, &W, &wedges, &p);
            assert_eq!(k.theta(g), p.theta[g]);
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
            source(&k, g, &price, &active, &gammas(n), &spend, &mut base);
            for lambda in [1e-6, 1e3] {
                let scaled: Vec<f64> = price.iter().map(|p| p * lambda).collect();
                let mut out = Sourced::new();
                source(&k, g, &scaled, &active, &gammas(n), &spend, &mut out);
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
                source(&k, g, &scaled, &active, &gammas(n), &spend_l, &mut out);
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

    /// Trade SPEC, Invariants, Dispatch mass: fills that make f w_o subnormal leave the
    /// shipments off the filled orders by far more than 1e-12 of either, at units in the normal
    /// range, and within the rounding the kernel carries for them.
    #[test]
    fn dispatch_carries_the_rounding_below_the_normal_range() {
        let (net, wedges) = random_world(42);
        let n = net.n_provinces();
        let k = kernel(&net, &wedges);
        let mut rng = rng(42);
        let price = draw(&mut rng, n, 0.5, 1.5);
        let spend = draw2(&mut rng, n, 1e290, 1e300);
        let active = vec![true; n];
        let mut out = Sourced::new();
        source(&k, 6, &price, &active, &gammas(n), &spend, &mut out);
        let fill: Vec<[f64; 2]> = out
            .scratch
            .weight
            .iter()
            .map(|&w| [1e-318 / w, 3e-320 / w])
            .collect();
        dispatch(&k, 6, &price, &fill, &mut out);
        let mut beyond = 0;
        for (o, f) in fill.iter().enumerate() {
            let sold = f[0] * out.orders[o][0] + f[1] * out.orders[o][1];
            let sent = out.shipped[o];
            let gap = (sent - sold).abs();
            let rounding = out.shipped_rounding[o]
                + f[0] * out.orders_rounding[o][0]
                + f[1] * out.orders_rounding[o][1];
            let relative = 1e-12 * sent.max(sold);
            assert!(
                sold > 1e-200,
                "origin {o}: sales {sold} in the normal range"
            );
            beyond += usize::from(gap > relative + f64::MIN_POSITIVE);
            assert!(
                gap <= relative + rounding + f64::MIN_POSITIVE,
                "origin {o}: shipped {sent}, sales {sold}, rounding {rounding}"
            );
        }
        assert!(beyond > 0, "no shipment beyond 1e-12 of its sales");
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
            let (mut m_in, mut m_out) = (vec![0.0; k.n_gateways()], vec![0.0; k.n_gateways()]);
            let mut paid: Vec<f64> = out.tariff.clone();
            let mut received = vec![0.0; n];
            for p in 0..n {
                // the home purchase moves no money across legs
                let home = price[p] * (out.home[p][0] + out.home[p][1]);
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
                    due += (1.0 / e.inv_duty - 1.0) * out.crossing_value[lo + i];
                }
            }
            for a in 0..k.n_gateways() {
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

    /// With one gateway per country each crossing carries one country pair, so its origin
    /// units are the filled dense units of every route between the two countries.
    #[test]
    fn crossing_units_are_origin_units_by_country_pair() {
        for seed in 1..=10u64 {
            let mut rng = rng(seed * 31);
            let n_countries = 2 + rng.below(2);
            let provinces: Vec<u16> = (0..n_countries as u16)
                .flat_map(|c| std::iter::repeat_n(c, 1 + rng.below(4)))
                .collect();
            let gateways = (0..n_countries as u16).collect();
            let (net, wedges) = complete_world(&mut rng, provinces, gateways);
            let n = net.n_provinces();
            let k = kernel(&net, &wedges);
            let g = [0, 3, 5][seed as usize % 3];
            let price = draw(&mut rng, n, 0.3, 3.0);
            let spend = draw2(&mut rng, n, 0.0, 100.0);
            let fill = draw2(&mut rng, n, 0.0, 1.0);
            let out = run(&k, g, &price, &vec![true; n], &spend, &fill);

            let p_min = price.iter().copied().fold(f64::INFINITY, f64::min);
            let w: Vec<f64> = price.iter().map(|p| f64::powf(p / p_min, -THETA)).collect();
            let gk = k.good(g);
            for a in 0..net.n_gateways() {
                let lo = gk.crossings.offsets[a] as usize;
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    let (ca, cb) = (net.gateway_country[a], net.gateway_country[e.col as usize]);
                    let mut want = 0.0;
                    for d in (0..n).filter(|&d| net.province_country[d] == cb) {
                        let all: Vec<Vec<Hop>> = (0..n)
                            .map(|o| hops(&net, &wedges, g, o, d, THETA))
                            .collect();
                        let phi: f64 = (0..n)
                            .map(|o| all[o].iter().map(|h| h.k).sum::<f64>() * w[o])
                            .sum();
                        for o in (0..n).filter(|&o| net.province_country[o] == ca) {
                            for h in &all[o] {
                                for c in 0..2 {
                                    let x = spend[d][c] * h.k * w[o] / phi;
                                    want += fill[o][c] * x / (1.0 + h.tariff) / price[o];
                                }
                            }
                        }
                    }
                    let got = out.crossing_units[lo + i];
                    assert!(
                        close(got, want, 1e-10),
                        "seed {seed} {a}->{}: {got} vs {want}",
                        e.col
                    );
                }
            }
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
        // province 2 reaches only province 1, 1e18 times dearer than province 0: weights are
        // relative to their country's best, so it buys there; province 3 reaches nobody
        let k12 = k_leg(g, &road(1, 2), 1.0, THETA);
        assert!(close(
            out.price_index[2],
            1e18 * k12.powf(-1.0 / THETA),
            1e-12
        ));
        assert_eq!(out.price_index[3], f64::INFINITY);
        assert!(close(out.orders[0][0] * 1.0, 1.0, 1e-12));
        assert!(close(out.orders[1][0] * 1e18, 3.0 + 5.0, 1e-12));
        assert!(close(out.orders[1][1] * 1e18, 4.0 + 6.0, 1e-12));
        assert_eq!(out.orders[2], [0.0; 2]);
        assert_eq!(out.orders[3], [0.0; 2]);
        for (paid, want) in out.paid_by_class[2].iter().zip(spend[2]) {
            assert!(close(*paid, want, 1e-12));
        }
        assert_eq!(out.paid[3], 0.0);
        assert_eq!(out.paid_by_class[3], [0.0; 2]);
        assert!(out.arrive.iter().all(|a| a[3] == [0.0; 2]));
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

    /// ADR 0200: an inactive origin, an island no origin reaches and an origin left unfilled
    /// ship nothing, so their legs add nothing to the selection tally and nothing is NaN.
    #[test]
    fn inactive_leg_has_no_selection_term() {
        // country 0: provinces 0 and 1 and a gateway; country 1: province 2 and a gateway;
        // country 2: province 3, an island with no leg; province 1 is inactive
        let leg = |from, to, cost_per_t| Leg {
            from,
            to,
            cost_per_t,
            hours: 30.0,
        };
        let net = Network {
            province_country: vec![0, 0, 1, 2],
            gateway_country: vec![0, 1],
            gateway_mass: vec![1.0, 1.0],
            domestic: vec![leg(0, 1, 40.0), leg(1, 0, 40.0)],
            access: vec![leg(0, 0, 20.0), leg(1, 0, 20.0), leg(2, 1, 30.0)],
            crossings: vec![leg(0, 1, 150.0), leg(1, 0, 150.0)],
            ..Network::default()
        };
        let wedges = Wedges::none(3);
        let k = kernel(&net, &wedges);
        let g = 4;
        let price = [1.0, f64::NAN, 1.4, f64::NAN];
        let active = [true, false, true, false];
        let spend = [[2.0, 3.0], [4.0, 5.0], [6.0, 7.0], [8.0, 9.0]];
        // origin 2 unfilled: everything d buys from it stays unshipped
        let fill = [[1.0; 2], [1.0; 2], [0.0; 2], [1.0; 2]];
        let out = run(&k, g, &price, &active, &spend, &fill);
        assert_flows_finite(&out);
        assert!(out.selected.is_finite());
        assert_eq!(out.buyer_price[3], f64::INFINITY);
        assert!(out.arrive.iter().all(|a| a[3] == [0.0; 2]));
        assert_eq!((out.shipped[1], out.shipped[2]), (0.0, 0.0));
        assert_eq!((out.home[1], out.home[2]), ([0.0; 2], [0.0; 2]));
        let want = dense(&net, &wedges, g, &price, &active, &spend, &fill, THETA);
        assert_matches_dense("inactive legs", &out, &want, 1e-12);
        // only origin 0 ships: its routes are the whole tally
        let melted = out.shipped[0] - out.lost;
        let arrived: f64 = out.arrive.iter().flatten().flatten().sum();
        assert!(close(out.selected, arrived - melted, 1e-12));
    }

    /// ADR 0200: two countries of one province, prices fixed: with the selection constant held,
    /// the welfare change p_d / q_d of a costlier border is lambda_dd'^(-1/theta) relative to
    /// the start, the ACR formula, to 1e-12.
    #[test]
    fn two_country_welfare_equals_acr() {
        let leg = |from, to, cost_per_t| Leg {
            from,
            to,
            cost_per_t,
            hours: 10.0,
        };
        let world = |border: f64| Network {
            province_country: vec![0, 1],
            gateway_country: vec![0, 1],
            gateway_mass: vec![1.0, 1.0],
            access: vec![leg(0, 0, 20.0), leg(1, 1, 30.0)],
            crossings: vec![leg(0, 1, 60.0 * border), leg(1, 0, 80.0 * border)],
            ..Network::default()
        };
        let mut wedges = Wedges::none(2);
        wedges.tariff.fill(0.05);
        let price = [1.0, 0.8];
        let spend = [[0.0, 100.0], [0.0, 0.0]];
        let gamma = [1.17, 0.93];
        let at = |border: f64| {
            let k = kernel(&world(border), &wedges);
            let mut out = Sourced::new();
            source(&k, 4, &price, &[true; 2], &gamma, &spend, &mut out);
            dispatch(&k, 4, &price, &[[1.0; 2]; 2], &mut out);
            let lambda = out.home[0][1] * price[0] / spend[0][1];
            (out.buyer_price[0], lambda)
        };
        let (q, lambda) = at(1.0);
        assert!(lambda > 0.05 && lambda < 0.95, "{lambda}");
        for border in [1.1, 2.0, 10.0] {
            let (q1, lambda1) = at(border);
            let welfare = (price[0] / q1) / (price[0] / q);
            let acr = (lambda1 / lambda).powf(-1.0 / THETA);
            assert!(close(welfare, acr, 1e-12), "x{border}: {welfare} vs {acr}");
            assert!(welfare < 1.0);
        }
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
        source(&k, 5, &p, &active, &gammas(n), &s, &mut reused);
        dispatch(&k, 5, &p, &vec![[1.0; 2]; n], &mut reused);
        let ptrs = (reused.orders.as_ptr(), reused.arrive[3].as_ptr());
        for g in [0, 3, 6] {
            let p = draw(&mut rng, n, 0.5, 1.5);
            let s = draw2(&mut rng, n, 0.0, 100.0);
            let f = draw2(&mut rng, n, 0.0, 1.0);
            source(&k, g, &p, &active, &gammas(n), &s, &mut reused);
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
            prop_assert!(close(shipped - out.lost + out.selected, arrived, 1e-12),
                "{shipped} - {} + {} vs {arrived}", out.lost, out.selected);
            for ((r, p), q) in out.received.iter().zip(&price).zip(&out.shipped) {
                prop_assert!(close(*r, p * q, 1e-12));
            }
        }

        /// ADR 0200: a costlier link, at fixed origin prices and selection constants, never
        /// lowers a buyer price.
        #[test]
        fn raising_tau_never_lowers_the_buyer_price(
            c in case(-1.0..1.0),
            pick in any::<usize>(),
            factor in 1.0f64..=2.0,
        ) {
            let (seed, g) = (c.0, c.1);
            let (net, wedges) = random_world(seed);
            let n = net.n_provinces();
            let price: Vec<f64> = c.2[..n].iter().map(|&e| 10f64.powf(e)).collect();
            let (active, spend) = (&c.3[..n], &c.4[..n]);
            let mut dear = net.clone();
            let legs = dear.domestic.len() + dear.access.len() + dear.crossings.len();
            let l = pick % legs;
            let leg = if l < dear.domestic.len() {
                &mut dear.domestic[l]
            } else if l < dear.domestic.len() + dear.access.len() {
                &mut dear.access[l - dear.domestic.len()]
            } else {
                &mut dear.crossings[l - dear.domestic.len() - dear.access.len()]
            };
            // tau - 1 is linear in both, so this multiplies it by the factor
            leg.cost_per_t *= factor;
            leg.hours *= factor;
            let buyer = |net: &Network| {
                let mut out = Sourced::new();
                source(&kernel(net, &wedges), g, &price, active, &gammas(n), spend, &mut out);
                out.buyer_price
            };
            let (before, after) = (buyer(&net), buyer(&dear));
            for d in 0..n {
                prop_assert!(after[d] >= before[d] * (1.0 - 1e-12),
                    "d {d}: {} below {}", after[d], before[d]);
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
            prop_assert!(out.buyer_price.iter().all(|q| *q > 0.0 && !q.is_nan()));
            prop_assert!(out.selected.is_finite());
            // every destination receives what it paid over its buyer price
            for d in 0..n {
                let arrived: f64 = out.arrive.iter().map(|a| a[d][0] + a[d][1]).sum();
                let want = out.paid[d] / out.buyer_price[d];
                prop_assert!(close(arrived, want, 1e-12), "d {d}: {arrived} vs {want}");
            }
            // with no weight floor a source 1e57 dearer than its buyers' best still sells a
            // value that is a normal f64, but its units, value / p, can underflow: each leg's
            // units are then off by up to the smallest normal f64
            let legs = (n * n * 4) as f64;
            for ((r, p), q) in out.received.iter().zip(&price).zip(&out.shipped) {
                let slack = p * f64::MIN_POSITIVE * legs;
                prop_assert!((r - p * q).abs() <= 1e-12 * r.max(p * q) + slack);
            }
        }
    }

    fn assert_matches_dense(tag: &str, out: &Sourced, want: &Dense, tol: f64) {
        for c in 0..2 {
            let what = format!("{tag} orders class {c}");
            assert_close(&what, &class(&out.orders, c), &class(&want.orders, c), tol);
            for s in 0..5 {
                let (got, want) = (class(&out.arrive[s], c), class(&want.arrive[s], c));
                assert_close(&format!("{tag} arrive[{s}] class {c}"), &got, &want, tol);
            }
        }
        assert_close(
            &format!("{tag} index"),
            &out.price_index,
            &want.price_index,
            tol,
        );
        assert_close(&format!("{tag} shipped"), &out.shipped, &want.shipped, tol);
        assert_close(&format!("{tag} paid"), &out.paid, &want.paid, tol);
        assert_close(
            &format!("{tag} received"),
            &out.received,
            &want.received,
            tol,
        );
        assert_close(&format!("{tag} tariff"), &out.tariff, &want.tariff, tol);
        assert_close(&format!("{tag} lost"), &[out.lost], &[want.lost], tol);
        let buyer = &want.buyer_price;
        assert_close(&format!("{tag} buyer price"), &out.buyer_price, buyer, tol);
        for (s, (got, want)) in out.survived.iter().zip(&want.survived).enumerate() {
            assert_close(&format!("{tag} survived[{s}]"), got, want, tol);
        }
        for c in 0..2 {
            let (got, home) = (class(&out.home, c), class(&want.home, c));
            assert_close(&format!("{tag} home class {c}"), &got, &home, tol);
        }
        // selection is a difference of near sums: compare it on the scale of the arrivals
        let scale: f64 = want.arrive.iter().flatten().flatten().sum();
        let gap = (out.selected - want.selected).abs();
        assert!(
            gap <= tol * scale,
            "{tag} selected {} vs {}",
            out.selected,
            want.selected
        );
    }

    #[test]
    fn country_scales_keep_flows_beyond_one_f64_scale() {
        // country 1's prices are 1e40 times country 0's: at theta 10.57 its weights are e^-973
        // of the world's best, below any f64, so only the country scales keep its origins.
        // Both countries trade through their gateways, and province 4 has no access leg: its
        // access is home and domestic only, in its own country's scale.
        let theta = 10.57;
        let mut r = rng(23);
        let (mut net, wedges) = complete_world(&mut r, vec![0, 0, 0, 1, 1, 1], vec![0, 0, 1, 1]);
        net.access.retain(|l| l.from != 4);
        let k = kernel_at(&net, &wedges, theta);
        let price: Vec<f64> = (0..6)
            .map(|o| if o < 3 { 1.0 } else { 1e40 } * (0.8 + 0.4 * r.next()))
            .collect();
        let active = [true, true, false, true, true, false];
        let spend = draw2(&mut r, 6, 1.0, 100.0);
        let fill = draw2(&mut r, 6, 0.2, 1.0);
        for g in [0, 4] {
            let out = run(&k, g, &price, &active, &spend, &fill);
            assert!(out.price_index.iter().all(|p| p.is_finite()));
            // province 4 buys only at home and from province 3, both dear
            assert!(out.price_index[4] > 1e39, "{}", out.price_index[4]);
            let want = dense(&net, &wedges, g, &price, &active, &spend, &fill, theta);
            assert_matches_dense(&format!("good {g}"), &out, &want, 1e-12);
        }
    }

    #[test]
    fn wide_price_spread_keeps_both_countries_reachable() {
        // country 1's prices are 1e12 times country 0's: at theta 10.57 its weights are about
        // 1e-127 of the world's best, under the former floor of 1e-100. Without gateways each
        // country sources at home only, and with them the country scales are carried across
        // the crossings; either way flows match the dense f64 reference.
        let theta = 10.57;
        for gateways in [vec![], vec![0, 0, 1, 1]] {
            let mut r = rng(17);
            let (net, wedges) = complete_world(&mut r, vec![0, 0, 0, 1, 1, 1], gateways);
            let k = kernel_at(&net, &wedges, theta);
            let price: Vec<f64> = (0..6)
                .map(|o| if o < 3 { 1.0 } else { 1e12 } * (0.8 + 0.4 * r.next()))
                .collect();
            let active = [true, true, false, true, true, false];
            let spend = draw2(&mut r, 6, 1.0, 100.0);
            let fill = draw2(&mut r, 6, 0.2, 1.0);
            for g in [0, 4] {
                let out = run(&k, g, &price, &active, &spend, &fill);
                assert!(out.price_index.iter().all(|p| p.is_finite()));
                let want = dense(&net, &wedges, g, &price, &active, &spend, &fill, theta);
                let tag = format!("{} gateways, good {g}", net.n_gateways());
                assert_matches_dense(&tag, &out, &want, 1e-12);
            }
        }
    }

    #[test]
    fn duplicating_a_gateway_with_half_mass_changes_no_flow() {
        for seed in 1..=8u64 {
            let (net, wedges) = random_world(seed * 0x3c6e_f372);
            let n = net.n_provinces();
            // gateway a and its copy share a's legs and half its mass each
            let a = rng(seed).below(net.n_gateways());
            let copy = net.n_gateways() as u32;
            let mut split = net.clone();
            split.gateway_country.push(net.gateway_country[a]);
            split.gateway_mass[a] /= 2.0;
            split.gateway_mass.push(split.gateway_mass[a]);
            for l in net.access.iter().filter(|l| l.to as usize == a) {
                split.access.push(Leg { to: copy, ..*l });
            }
            for l in &net.crossings {
                if l.from as usize == a {
                    split.crossings.push(Leg { from: copy, ..*l });
                }
                if l.to as usize == a {
                    split.crossings.push(Leg { to: copy, ..*l });
                }
            }
            let (one, two) = (kernel(&net, &wedges), kernel(&split, &wedges));
            let mut r = rng(seed + 50);
            let price = draw(&mut r, n, 0.5, 2.0);
            let mut active = vec![true; n];
            active[r.below(n)] = false;
            let spend = draw2(&mut r, n, 0.0, 100.0);
            let fill = draw2(&mut r, n, 0.0, 1.0);
            for g in [0, 3, 5] {
                let base = run(&one, g, &price, &active, &spend, &fill);
                let out = run(&two, g, &price, &active, &spend, &fill);
                let tag = |what: &str| format!("seed {seed} g {g} {what}");
                for c in 0..2 {
                    let (got, want) = (class(&out.orders, c), class(&base.orders, c));
                    assert_close(&tag("orders"), &got, &want, 1e-12);
                    for s in 0..5 {
                        let (got, want) = (class(&out.arrive[s], c), class(&base.arrive[s], c));
                        assert_close(&tag("arrive"), &got, &want, 1e-12);
                    }
                }
                assert_close(&tag("index"), &out.price_index, &base.price_index, 1e-12);
                assert_close(&tag("shipped"), &out.shipped, &base.shipped, 1e-12);
                assert_close(&tag("paid"), &out.paid, &base.paid, 1e-12);
                assert_close(&tag("received"), &out.received, &base.received, 1e-12);
                assert_close(&tag("tariff"), &out.tariff, &base.tariff, 1e-12);
                assert_close(&tag("lost"), &[out.lost], &[base.lost], 1e-12);
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

    /// Asserts that each present part of every destination's spending per unit of access
    /// is `exp(-(ln Phi_d - shift))` of its scale.
    fn assert_reach_matches_ln_phi(k: &Kernel, out: &Sourced) {
        let acc = &out.scratch.access;
        for (d, &c) in k.province_country().iter().enumerate() {
            let c = usize::from(c);
            let ln_phi = acc.ln_phi(d, c);
            let (at_home, at_abroad) = acc.reach(d, c);
            for (at, shift) in [(at_home, acc.shift[c]), (at_abroad, acc.dest_shift[c])] {
                if at > 0.0 {
                    let got = shift - at.ln();
                    assert!(close(got, ln_phi, 1e-12), "d {d}: {got} vs {ln_phi}");
                }
            }
        }
    }

    /// With full fills each destination pays exactly its spending.
    fn assert_spends_all(out: &Sourced, spend: &[[f64; 2]]) {
        for (d, (paid, want)) in out.paid_by_class.iter().zip(spend).enumerate() {
            for (&paid, &want) in paid.iter().zip(want) {
                assert!(close(paid, want, 1e-12), "d {d}: {paid} vs {want}");
            }
        }
    }

    fn assert_flows_finite(out: &Sourced) {
        let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
        let finite2 = |v: &[[f64; 2]]| v.iter().flatten().all(|x| x.is_finite());
        assert!(finite2(&out.orders) && finite2(&out.paid_by_class));
        assert!(finite(&out.shipped) && finite(&out.paid) && finite(&out.received));
        assert!(finite(&out.tariff) && finite(&out.domestic_value));
        assert!(finite(&out.access_in_value) && finite(&out.access_out_value));
        assert!(finite(&out.crossing_value) && finite(&out.crossing_units));
        assert!(out.arrive.iter().all(|a| finite2(a)));
        assert!(out.lost.is_finite());
    }

    #[test]
    fn home_part_keeps_its_share_across_a_scale_gap_beyond_exp() {
        // province 0 of country 0 is e^gap times dearer than province 1 of country 1, gap
        // above 709.78, so exp(gap) overflows; its only import route crosses a costly border,
        // so its abroad part is near the smallest normal f64 and the abroad part in the home
        // scale is e^4: the home part keeps 1 / (1 + e^4) of the spending
        let g = 0;
        let leg = |from, to, cost_per_t| Leg {
            from,
            to,
            cost_per_t,
            hours: 0.0,
        };
        let net = Network {
            province_country: vec![0, 1],
            gateway_country: vec![0, 1],
            gateway_mass: vec![1.0, 1.0],
            access: vec![leg(0, 0, 1.0), leg(1, 1, 1.0)],
            crossings: vec![leg(1, 0, 2.0 * 140.4f64.exp() / 1.3), leg(0, 1, 1.0)],
            ..Network::default()
        };
        let wedges = Wedges::none(2);
        let k = kernel(&net, &wedges);
        let nu = 1.0 + TradeParams::default().border_nu_of(g);
        let abroad =
            k_leg(g, &net.access[0], 1.0, THETA).powi(2) * k_leg(g, &net.crossings[0], nu, THETA);
        let gap = 4.0 - abroad.ln();
        assert!(
            gap > 709.79 && abroad >= f64::MIN_POSITIVE,
            "{gap} {abroad}"
        );
        let price = [(gap / THETA).exp(), 1.0];
        let spend = [[10.0, 5.0], [10.0, 20.0]];
        let fill = [[1.0; 2]; 2];
        let out = run(&k, g, &price, &[true; 2], &spend, &fill);

        assert_flows_finite(&out);
        assert_reach_matches_ln_phi(&k, &out);
        let home_share = 1.0 / (1.0 + 4f64.exp());
        assert!(close(out.orders[0][0] * price[0], 10.0 * home_share, 1e-9));
        assert_spends_all(&out, &spend);
        let want = dense(&net, &wedges, g, &price, &[true; 2], &spend, &fill, THETA);
        assert_matches_dense("scale gap", &out, &want, 1e-9);
    }

    #[test]
    fn spending_beyond_f64_per_unit_of_access_stays_finite() {
        // province 1 reaches only province 0, over a domestic leg with K about 1e-307, and
        // province 2 only through a crossing whose route K is about 1e-306: their spending
        // per unit of access, E / Phi, is beyond f64, though every flow is at most E
        let g = 0;
        let leg = |from, to, cost_per_t| Leg {
            from,
            to,
            cost_per_t,
            hours: 0.0,
        };
        let net = Network {
            province_country: vec![0, 0, 1],
            gateway_country: vec![0, 1],
            gateway_mass: vec![1.0, 1.0],
            domestic: vec![leg(0, 1, 2.0 * 141.38f64.exp()), leg(1, 0, 1.0)],
            access: vec![leg(0, 0, 1.0), leg(1, 0, 1.0), leg(2, 1, 1.0)],
            crossings: vec![leg(0, 1, 2.0 * 140.04f64.exp() / 1.3), leg(1, 0, 1.0)],
            ..Network::default()
        };
        let wedges = Wedges::none(2);
        let k = kernel(&net, &wedges);
        let price = [1.0, f64::NAN, f64::NAN];
        let active = [true, false, false];
        let spend = [[3.0, 4.0], [100.0, 1e6], [1e6, 100.0]];
        let fill = [[1.0; 2]; 3];
        let out = run(&k, g, &price, &active, &spend, &fill);

        let acc = &out.scratch.access;
        for (part, e) in [(acc.home[1], 100.0), (acc.abroad[2], 1e6)] {
            assert!(f64::MIN_POSITIVE <= part && part < e / f64::MAX, "{part}");
        }
        assert_flows_finite(&out);
        assert_reach_matches_ln_phi(&k, &out);
        assert_spends_all(&out, &spend);
        let want = dense(&net, &wedges, g, &[1.0; 3], &active, &spend, &fill, THETA);
        assert_matches_dense("tiny access", &out, &want, 1e-9);
    }
}
