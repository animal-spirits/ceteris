//! Destination sourcing on the two-level kernel (ADRs 0015 and 0016): destination spending
//! spread over origins, and the forward dispatch of what the origin markets fill.
//!
//! Destination d spends E_d (numeraire, tariff included) on good g over routes by
//! `X_od = E_d K_od p_o^-theta / Phi_d`, `Phi_d = sum_o K_od p_o^-theta`, theta the good's
//! P01 value, so its delivered price index is `P_d = Phi_d^(-1/theta)`. The kernel on a crossing carries the tariff, so X
//! is what d pays: the origin ships `X / (p_o (1 + t))` units and receives `p_o` per unit, the
//! importer's rebate account `t p_o` per unit, and d receives the shipped units over the
//! physical loss factor of the route. Domestic and home routes have t = 0.
//!
//! Weights and access are held in the log scale of each country (ADR 0093,
//! [`crate::kernel::Access`]), so a price spread between countries makes no destination
//! unreachable: only a destination with no path from an active origin buys nothing. Losses and
//! tariffs are applied by multiplying with the kernel's stored reciprocals (ADR 0097).

use rayon::prelude::*;

use crate::kernel::{Access, Kernel};

/// Whether a destination with access `phi`, in its country's scale, reaches an active origin.
/// Access below the smallest normal f64 is underflow of kernel factors, not a path.
fn reachable(phi: f64) -> bool {
    phi >= f64::MIN_POSITIVE
}

/// Provinces per parallel task at least, in loops over provinces.
const ROWS: usize = 64;

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
    /// `f64::INFINITY` where no active origin is reachable. Per province.
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
    /// Origin units shipped over each crossing, before any loss: `crossing_value` read in
    /// units at the prices of the origins it came from. Aligned with `crossing_value`.
    pub crossing_units: Vec<f64>,
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
    /// Kernel id, good, prices and active origins the weights, access and price index were
    /// last computed for: they depend on nothing else, so a repeat call reuses them.
    prepared: Option<(u64, usize)>,
    price: Vec<f64>,
    active: Vec<bool>,
    price_index: Vec<f64>,
    /// Log weights and country shifts of [`origin_weights`], and the weights `exp(ln_w)`.
    ln_w: Vec<f64>,
    shift: Vec<f64>,
    weight: Vec<f64>,
    access: Access,
    /// Spending per unit of access by class, `E_d / Phi_d` in d's country scale; 0 where
    /// unreachable.
    reach: Vec<[f64; 2]>,
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
    /// Per leg, what the leg adds to the sums at its target: aligned with the kernel entries
    /// of each pass, the home purchase per province.
    home: Vec<f64>,
    domestic_legs: Vec<DomesticLeg>,
    origin_legs: Vec<OriginLeg>,
    crossing_legs: Vec<CrossingLeg>,
    /// Loss on every leg in the order of a serial pass over the legs: per origin its
    /// domestic then its access legs, every crossing, every arrival leg by lag.
    loss: Vec<f64>,
}

/// Value entering a destination gateway, the same net of tariff, and units by lag.
#[derive(Clone, Copy, Debug, Default)]
struct Inflow {
    gross: f64,
    net: f64,
    units: [f64; 4],
}

#[derive(Clone, Copy, Debug, Default)]
struct DomesticLeg {
    value: f64,
    got: f64,
}

/// Origin to origin gateway: value, the same in units at the gateway and in origin units.
#[derive(Clone, Copy, Debug, Default)]
struct OriginLeg {
    gross: f64,
    units: f64,
    origin_units: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct CrossingLeg {
    gross: f64,
    net: f64,
    got: f64,
    lag: u8,
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
    let (np, ng) = (k.n_provinces(), k.n_gateways());
    assert_eq!(spend.len(), np, "spend must have one entry per province");
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
        s.price_index
            .extend(acc.pi.iter().zip(k.province_country()).map(|(&phi, &c)| {
                let ln_phi = if reachable(phi) {
                    acc.dest_shift[usize::from(c)] + libm::log(phi)
                } else {
                    f64::NEG_INFINITY
                };
                price_index(p_min, ln_phi, k.theta(g))
            }));
        s.price.clear();
        s.price.extend_from_slice(price);
        s.active.clear();
        s.active.extend_from_slice(active);
        s.prepared = Some((k.id(), g));
    }
    let (w, local, v, u, phi) = (
        &s.weight,
        &s.access.local,
        &s.access.v,
        &s.access.u,
        &s.access.pi,
    );
    let (gc, nc, carry) = (k.gateway_country(), k.n_countries(), &s.access.carry);

    out.price_index.clone_from(&s.price_index);
    s.reach.clear();
    s.reach.extend((0..np).map(|d| {
        if reachable(phi[d]) {
            [spend[d][0] / phi[d], spend[d][1] / phi[d]]
        } else {
            [0.0; 2]
        }
    }));
    let reach = &s.reach;

    // Orders are accumulated as value net of tariff and turned into units at the end. Both
    // classes go through each loop together; every accumulator is per class, so each sees
    // the additions of a pass of its own. Loops over origins and over origin gateways write
    // only their own entries and run in parallel.
    let orders = &mut out.orders;
    zeroed2(orders, np);
    // home and domestic routes
    orders
        .par_iter_mut()
        .enumerate()
        .with_min_len(ROWS)
        .filter(|(o, _)| w[*o] > 0.0)
        .for_each(|(o, order)| {
            let mut x = [local[o] * reach[o][0], local[o] * reach[o][1]];
            for e in gk.domestic.row(o) {
                let (kw, r) = (e.k * local[o], reach[e.col as usize]);
                x[0] += kw * r[0];
                x[1] += kw * r[1];
            }
            order[0] += x[0];
            order[1] += x[1];
        });
    // destinations to their gateways
    let to_b = &mut s.to_dest_gateway;
    zeroed2(to_b, ng);
    for (d, r) in reach.iter().enumerate() {
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
    orders
        .par_iter_mut()
        .enumerate()
        .with_min_len(ROWS)
        .filter(|(o, _)| w[*o] > 0.0)
        .for_each(|(o, order)| {
            for e in gk.access.row(o) {
                let a = e.col as usize;
                let t = at_a[a];
                if t == [0.0; 2] {
                    continue;
                }
                let factor = e.k * w[o] / v[a];
                for c in (0..2).filter(|&c| t[c] != 0.0) {
                    order[c] += t[c] * factor;
                }
            }
            for x in order {
                *x /= price[o];
            }
        });
}

/// Dispatches the filled orders of the preceding [`source`] call for the same good: origin o
/// ships `fill[o][c]` of its class-c orders along the routes they were placed on. `price` is
/// the one passed to `source`. The forward pass is the backward split read the other way:
/// origin o sends value `fill w_o K_oa z_a` to gateway a, a splits by `K_ab y_b / z_a`, b by
/// `K_bd reach_d / y_b`, with `reach_d = E_d / Phi_d`. Each leg multiplies the units by its
/// stored reciprocal physical factor (iceberg, and the border loss on crossings), and a
/// crossing the value by its `1 / (1 + t)`, so arrivals are the sum over routes of
/// `X / (p_o (1 + t) tau_route)`. Divisions are by reciprocals hoisted per origin, per
/// origin gateway and per destination gateway.
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
    let (dom, acc, cross) = (&gk.domestic, &gk.access, &gk.crossings);
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
        crossing_units,
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
    zeroed(domestic_value, dom.entries.len());
    zeroed(access_in_value, acc.entries.len());
    zeroed(crossing_value, cross.entries.len());
    zeroed(crossing_units, cross.entries.len());
    zeroed(access_out_value, acc.entries.len());
    *lost = 0.0;
    let Scratch {
        weight: w,
        access,
        reach,
        y,
        inv_y,
        z,
        net_ratio,
        gross_a,
        units_a,
        origin_units_a,
        at_b,
        home,
        domestic_legs,
        origin_legs,
        crossing_legs,
        loss,
        ..
    } = s;
    // every pass writes all of its entries, so these need no zeroing
    for v in [
        &mut *y,
        &mut *inv_y,
        &mut *z,
        &mut *net_ratio,
        &mut *gross_a,
        &mut *units_a,
        &mut *origin_units_a,
    ] {
        v.resize(ng, 0.0);
    }
    at_b.resize(ng, Inflow::default());
    home.resize(np, 0.0);
    domestic_legs.resize(dom.entries.len(), DomesticLeg::default());
    origin_legs.resize(acc.entries.len(), OriginLeg::default());
    crossing_legs.resize(cross.entries.len(), CrossingLeg::default());
    let n_origin = dom.entries.len() + acc.entries.len();
    loss.resize(n_origin + cross.entries.len() + 4 * acc.entries.len(), 0.0);
    let [arrive_home, arrive_1, arrive_2, arrive_3, arrive_4] = arrive;
    let (w, reach) = (&*w, &*reach);
    let (gc, nc) = (k.gateway_country(), k.n_countries());
    let (carry, local) = (&access.carry, &access.local);

    for c in 0..2 {
        // a class that spends nothing anywhere ships nothing: its pass would add only zeros
        if reach.iter().all(|r| r[c] == 0.0) {
            continue;
        }
        let (origin_loss, rest) = loss.split_at_mut(n_origin);
        let (crossing_loss, arrival_loss) = rest.split_at_mut(cross.entries.len());

        // Every sum below at a gateway or province gathers its terms in the order a serial
        // pass over the legs would add them, so its bits do not depend on the thread count.
        // A leg that carries nothing adds 0, which leaves these sums of non-negative terms
        // unchanged.

        // destination-side access sums on the spending weights of this class
        y.par_iter_mut()
            .zip(inv_y.par_iter_mut())
            .enumerate()
            .for_each(|(b, (y, inv))| {
                let mut sum = 0.0;
                for &(d, j) in k.access_by_gateway().column(b) {
                    let r = reach[d as usize][c];
                    if r > 0.0 {
                        sum += acc.entries[j as usize].k * r;
                    }
                }
                (*y, *inv) = (sum, 1.0 / sum);
            });
        let (y, inv_y) = (&*y, &*inv_y);
        z.par_iter_mut()
            .zip(net_ratio.par_iter_mut())
            .enumerate()
            .for_each(|(a, (z, ratio))| {
                let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
                let (mut gross, mut net) = (0.0, 0.0);
                for e in cross.row(a) {
                    let b = e.col as usize;
                    let x = e.k * carry_a[usize::from(gc[b])] * y[b];
                    gross += x;
                    net += x * e.inv_duty;
                }
                *z = gross;
                *ratio = if gross > 0.0 { net / gross } else { 0.0 };
            });
        let (z, net_ratio) = (&*z, &*net_ratio);

        // Origins: home, domestic and into the first gateway.
        let origin_bound = |o: usize| (dom.offsets[o] + acc.offsets[o]) as usize;
        let rows: Vec<_> = shipped
            .chunks_mut(ROWS)
            .zip(received.chunks_mut(ROWS))
            .zip(arrive_home.chunks_mut(ROWS))
            .zip(home.chunks_mut(ROWS))
            .zip(split_rows(ROWS, domestic_value, np, |o| {
                dom.offsets[o] as usize
            }))
            .zip(split_rows(ROWS, domestic_legs, np, |o| {
                dom.offsets[o] as usize
            }))
            .zip(split_rows(ROWS, access_out_value, np, |o| {
                acc.offsets[o] as usize
            }))
            .zip(split_rows(ROWS, origin_legs, np, |o| {
                acc.offsets[o] as usize
            }))
            .zip(split_rows(ROWS, origin_loss, np, origin_bound))
            .collect();
        rows.into_par_iter().enumerate().for_each(
            |(i, ((((((((shipped, received), arrive), home), dv), dl), av), ol), loss))| {
                let first = i * ROWS;
                let base = (
                    dom.offsets[first] as usize,
                    acc.offsets[first] as usize,
                    origin_bound(first),
                );
                for r in 0..shipped.len() {
                    let o = first + r;
                    let (dom_row, acc_row) = (dom.row(o), acc.row(o));
                    let d0 = dom.offsets[o] as usize - base.0;
                    let a0 = acc.offsets[o] as usize - base.1;
                    let l0 = origin_bound(o) - base.2;
                    let (dv, dl) = (
                        &mut dv[d0..][..dom_row.len()],
                        &mut dl[d0..][..dom_row.len()],
                    );
                    let (av, ol) = (
                        &mut av[a0..][..acc_row.len()],
                        &mut ol[a0..][..acc_row.len()],
                    );
                    let (dom_loss, acc_loss) =
                        loss[l0..][..dom_row.len() + acc_row.len()].split_at_mut(dom_row.len());
                    home[r] = 0.0;
                    let f = fill[o][c];
                    if w[o] == 0.0 || f == 0.0 {
                        dl.fill(DomesticLeg::default());
                        ol.fill(OriginLeg::default());
                        dom_loss.fill(0.0);
                        acc_loss.fill(0.0);
                        continue;
                    }
                    let (m, m_local, inv_p) = (f * w[o], f * local[o], 1.0 / price[o]);
                    let (mut sent, mut got_paid) = (shipped[r], received[r]);
                    let x = m_local * reach[o][c];
                    if x > 0.0 {
                        let units = x * inv_p;
                        arrive[r][c] += units;
                        sent += units;
                        got_paid += x;
                        home[r] = x;
                    }
                    for (((e, value), leg), loss) in dom_row.iter().zip(dv).zip(dl).zip(dom_loss) {
                        let x = m_local * e.k * reach[e.col as usize][c];
                        if x == 0.0 {
                            (*leg, *loss) = (DomesticLeg::default(), 0.0);
                            continue;
                        }
                        let units = x * inv_p;
                        let got = units * e.inv_tau;
                        sent += units;
                        *value += x;
                        got_paid += x;
                        (*leg, *loss) = (DomesticLeg { value: x, got }, units - got);
                    }
                    for (((e, value), leg), loss) in acc_row.iter().zip(av).zip(ol).zip(acc_loss) {
                        let a = e.col as usize;
                        let x = m * e.k * z[a];
                        if x == 0.0 {
                            (*leg, *loss) = (OriginLeg::default(), 0.0);
                            continue;
                        }
                        let net = x * net_ratio[a];
                        let units = net * inv_p;
                        *value += net;
                        got_paid += net;
                        sent += units;
                        let origin_units = x * inv_p;
                        *leg = OriginLeg {
                            gross: x,
                            units: origin_units * e.inv_tau,
                            origin_units,
                        };
                        *loss = units - units * e.inv_tau;
                    }
                    (shipped[r], received[r]) = (sent, got_paid);
                }
            },
        );
        let (home, domestic_legs, origin_legs) = (&*home, &*domestic_legs, &*origin_legs);
        gross_a
            .par_iter_mut()
            .zip(units_a.par_iter_mut())
            .zip(origin_units_a.par_iter_mut())
            .enumerate()
            .for_each(|(a, ((gross, units), origin_units))| {
                let mut sum = OriginLeg::default();
                for &(_, j) in k.access_by_gateway().column(a) {
                    let leg = origin_legs[j as usize];
                    sum.gross += leg.gross;
                    sum.units += leg.units;
                    sum.origin_units += leg.origin_units;
                }
                (*gross, *units, *origin_units) = (sum.gross, sum.units, sum.origin_units);
            });
        let (gross_a, units_a, origin_units_a) = (&*gross_a, &*units_a, &*origin_units_a);

        // Origin gateways onto crossings; units are kept by transit lag from here on.
        let rows: Vec<_> = split_rows(1, crossing_value, ng, |a| cross.offsets[a] as usize)
            .into_iter()
            .zip(split_rows(1, crossing_units, ng, |a| {
                cross.offsets[a] as usize
            }))
            .zip(split_rows(1, crossing_legs, ng, |a| {
                cross.offsets[a] as usize
            }))
            .zip(split_rows(1, crossing_loss, ng, |a| {
                cross.offsets[a] as usize
            }))
            .collect();
        rows.into_par_iter()
            .enumerate()
            .for_each(|(a, (((values, units_out), legs), loss))| {
                // as the scatter's `gross_a > 0` filter, NaN included
                let sends = gross_a[a] > 0.0;
                if !sends {
                    legs.fill(CrossingLeg::default());
                    loss.fill(0.0);
                    return;
                }
                let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
                let inv_z = 1.0 / z[a];
                let row = cross.row(a).iter().zip(values).zip(units_out);
                for (((e, value), units_out), (leg, loss)) in row.zip(legs.iter_mut().zip(loss)) {
                    let b = e.col as usize;
                    let weight = e.k * carry_a[usize::from(gc[b])] * y[b];
                    if weight == 0.0 {
                        (*leg, *loss) = (CrossingLeg::default(), 0.0);
                        continue;
                    }
                    let share = over(weight, inv_z, z[a]);
                    let gross = gross_a[a] * share;
                    let net = gross * e.inv_duty;
                    let units = units_a[a] * share * e.inv_duty;
                    let got = units * e.inv_loss;
                    *value += net;
                    *units_out += origin_units_a[a] * share * e.inv_duty;
                    let lag = e.lag;
                    (*leg, *loss) = (
                        CrossingLeg {
                            gross,
                            net,
                            got,
                            lag,
                        },
                        units - got,
                    );
                }
            });
        let crossing_legs = &*crossing_legs;
        at_b.par_iter_mut().enumerate().for_each(|(b, at)| {
            let mut sum = Inflow::default();
            for &(_, j) in k.crossings_by_target().column(b) {
                let leg = crossing_legs[j as usize];
                // a fixed index keeps each lag's sum in a register
                for (l, units) in sum.units.iter_mut().enumerate() {
                    *units += if l == usize::from(leg.lag) {
                        leg.got
                    } else {
                        0.0
                    };
                }
                sum.gross += leg.gross;
                sum.net += leg.net;
            }
            *at = sum;
        });
        let at_b = &*at_b;

        // Destinations: the home purchase and domestic shipments in origin order, then what
        // arrives from their gateways.
        let rows: Vec<_> = arrive_1
            .chunks_mut(ROWS)
            .zip(arrive_2.chunks_mut(ROWS))
            .zip(arrive_3.chunks_mut(ROWS))
            .zip(arrive_4.chunks_mut(ROWS))
            .zip(paid.chunks_mut(ROWS))
            .zip(paid_by_class.chunks_mut(ROWS))
            .zip(tariff.chunks_mut(ROWS))
            .zip(split_rows(ROWS, access_in_value, np, |d| {
                acc.offsets[d] as usize
            }))
            .zip(split_rows(ROWS, arrival_loss, np, |d| {
                4 * acc.offsets[d] as usize
            }))
            .collect();
        rows.into_par_iter().enumerate().for_each(
            |(i, ((((((((a1, a2), a3), a4), paid), by_class), tariff), av), loss))| {
                let first = i * ROWS;
                let base = acc.offsets[first] as usize;
                for r in 0..paid.len() {
                    let d = first + r;
                    let mut slots = [a1[r][c], a2[r][c], a3[r][c], a4[r][c]];
                    let (mut pay, mut pay_class, mut duty) = (paid[r], by_class[r][c], tariff[r]);
                    let mut home_paid = false;
                    for &(o, j) in k.domestic_by_destination().column(d) {
                        if !home_paid && o as usize > d {
                            pay += home[d];
                            pay_class += home[d];
                            home_paid = true;
                        }
                        let leg = domestic_legs[j as usize];
                        slots[0] += leg.got;
                        pay += leg.value;
                        pay_class += leg.value;
                    }
                    if !home_paid {
                        pay += home[d];
                        pay_class += home[d];
                    }
                    let a0 = acc.offsets[d] as usize - base;
                    let row = acc.row(d);
                    let (av, loss) = (
                        &mut av[a0..][..row.len()],
                        &mut loss[4 * a0..][..4 * row.len()],
                    );
                    let rc = reach[d][c];
                    for ((e, value), loss) in row.iter().zip(av).zip(loss.chunks_exact_mut(4)) {
                        let b = e.col as usize;
                        let weight = e.k * rc;
                        let buys = rc > 0.0;
                        if !buys || weight == 0.0 {
                            loss.fill(0.0);
                            continue;
                        }
                        let share = over(weight, inv_y[b], y[b]);
                        for ((slot, loss), &at) in slots.iter_mut().zip(loss).zip(&at_b[b].units) {
                            let units = at * share;
                            let got = units * e.inv_tau;
                            *slot += got;
                            *loss = units - got;
                        }
                        let gross = at_b[b].gross * share;
                        let net = at_b[b].net * share;
                        *value += net;
                        pay += gross;
                        pay_class += gross;
                        duty += gross - net;
                    }
                    [a1[r][c], a2[r][c], a3[r][c], a4[r][c]] = slots;
                    (paid[r], by_class[r][c], tariff[r]) = (pay, pay_class, duty);
                }
            },
        );

        // one add chain over every leg, in the order of a serial pass
        for x in &*loss {
            *lost += x;
        }
    }
}

/// `v` split into the parts of consecutive groups of `rows` of its `n_rows` rows, row i
/// starting at `start(i)`.
fn split_rows<T>(
    rows: usize,
    mut v: &mut [T],
    n_rows: usize,
    start: impl Fn(usize) -> usize,
) -> Vec<&mut [T]> {
    let mut parts = Vec::with_capacity(n_rows.div_ceil(rows));
    for first in (0..n_rows).step_by(rows) {
        let len = start((first + rows).min(n_rows)) - start(first);
        let (part, rest) = std::mem::take(&mut v).split_at_mut(len);
        parts.push(part);
        v = rest;
    }
    parts
}

/// The forward pass of [`dispatch`] as one serial scatter over the legs, in the order whose
/// sums `dispatch` reproduces.
#[cfg(test)]
fn dispatch_scatter(k: &Kernel, g: usize, price: &[f64], fill: &[[f64; 2]], out: &mut Sourced) {
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
        arrive,
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
    zeroed(crossing_units, gk.crossings.entries.len());
    zeroed(access_out_value, gk.access.entries.len());
    *lost = 0.0;

    let (w, reach) = (&s.weight, &s.reach);
    let (gc, nc) = (k.gateway_country(), k.n_countries());
    let (carry, local) = (&s.access.carry, &s.access.local);
    for c in 0..2 {
        // a class that spends nothing anywhere ships nothing: its pass would add only zeros
        if reach.iter().all(|r| r[c] == 0.0) {
            continue;
        }
        // destination-side access sums on the spending weights of this class
        zeroed(&mut s.y, ng);
        for d in (0..np).filter(|&d| reach[d][c] > 0.0) {
            for e in gk.access.row(d) {
                s.y[e.col as usize] += e.k * reach[d][c];
            }
        }
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

        // Origins: home, domestic and into the first gateway.
        zeroed(&mut s.gross_a, ng);
        zeroed(&mut s.units_a, ng);
        zeroed(&mut s.origin_units_a, ng);
        for o in 0..np {
            let f = fill[o][c];
            if w[o] == 0.0 || f == 0.0 {
                continue;
            }
            let (m, m_local, inv_p) = (f * w[o], f * local[o], 1.0 / price[o]);
            let x = m_local * reach[o][c];
            if x > 0.0 {
                let units = x * inv_p;
                arrive[0][o][c] += units;
                shipped[o] += units;
                paid[o] += x;
                paid_by_class[o][c] += x;
                received[o] += x;
            }
            let lo = gk.domestic.offsets[o] as usize;
            for (i, e) in gk.domestic.row(o).iter().enumerate() {
                let d = e.col as usize;
                let x = m_local * e.k * reach[d][c];
                if x == 0.0 {
                    continue;
                }
                let units = x * inv_p;
                let got = units * e.inv_tau;
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
                let x = m * e.k * s.z[a];
                if x == 0.0 {
                    continue;
                }
                let net = x * s.net_ratio[a];
                let units = net * inv_p;
                access_out_value[lo + i] += net;
                received[o] += net;
                shipped[o] += units;
                *lost += units - units * e.inv_tau;
                let origin_units = x * inv_p;
                s.gross_a[a] += x;
                s.units_a[a] += origin_units * e.inv_tau;
                s.origin_units_a[a] += origin_units;
            }
        }

        // Origin gateways onto crossings; units are kept by transit lag from here on.
        let (mut gross_b, mut net_b) = (vec![0.0; ng], vec![0.0; ng]);
        let mut units_b: [Vec<f64>; 4] = std::array::from_fn(|_| vec![0.0; ng]);
        for a in (0..ng).filter(|&a| s.gross_a[a] > 0.0) {
            let lo = gk.crossings.offsets[a] as usize;
            let carry_a = &carry[usize::from(gc[a]) * nc..][..nc];
            let inv_z = 1.0 / s.z[a];
            for (i, e) in gk.crossings.row(a).iter().enumerate() {
                let b = e.col as usize;
                let weight = e.k * carry_a[usize::from(gc[b])] * s.y[b];
                if weight == 0.0 {
                    continue;
                }
                let share = over(weight, inv_z, s.z[a]);
                let gross = s.gross_a[a] * share;
                let net = gross * e.inv_duty;
                let units = s.units_a[a] * share * e.inv_duty;
                let got = units * e.inv_loss;
                crossing_value[lo + i] += net;
                crossing_units[lo + i] += s.origin_units_a[a] * share * e.inv_duty;
                *lost += units - got;
                units_b[usize::from(e.lag)][b] += got;
                gross_b[b] += gross;
                net_b[b] += net;
            }
        }

        // Destination gateways to destinations.
        for d in (0..np).filter(|&d| reach[d][c] > 0.0) {
            let lo = gk.access.offsets[d] as usize;
            for (i, e) in gk.access.row(d).iter().enumerate() {
                let b = e.col as usize;
                let weight = e.k * reach[d][c];
                if weight == 0.0 {
                    continue;
                }
                let share = over(weight, 1.0 / s.y[b], s.y[b]);
                for (lag, at_b) in units_b.iter().enumerate() {
                    let units = at_b[b] * share;
                    let got = units * e.inv_tau;
                    arrive[1 + lag][d][c] += got;
                    *lost += units - got;
                }
                let gross = gross_b[b] * share;
                let net = net_b[b] * share;
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

    /// Kernel at `theta` for every good.
    fn kernel_at(net: &Network, wedges: &Wedges, theta: f64) -> Kernel {
        let p = TradeParams {
            theta: [theta; N_GOODS],
            ..TradeParams::default()
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
                let (oa, ab, bd) = (
                    find(&net.access, o, a),
                    find(&net.crossings, a, b),
                    find(&net.access, d, b),
                );
                let t = f64::from(wedges.rate(cd as usize, co as usize, tariff_group(g)));
                hs.push(Hop {
                    k: k_leg(g, oa, 1.0, theta)
                        * k_leg(g, ab, nu * (1.0 + t), theta)
                        * k_leg(g, bd, 1.0, theta),
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

    /// ADR 0015 on a dense route table, independent of the Markov split and of the country
    /// scales: d spends E_d over routes by K_route p_o^-theta / Phi_d, weights relative to one
    /// world p_min, unfloored; the origin ships X / (p_o (1 + t)) units, d receives that over
    /// the physical tau, pays X and the tariff takes t / (1 + t).
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
        let w: Vec<f64> = (0..n)
            .map(|o| {
                if active[o] {
                    f64::powf(price[o] / p_min, -theta)
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
            let all: Vec<Vec<Hop>> = (0..n).map(|o| hops(net, wedges, g, o, d, theta)).collect();
            let phi: f64 = (0..n)
                .map(|o| all[o].iter().map(|h| h.k).sum::<f64>() * w[o])
                .sum();
            if phi == 0.0 {
                continue;
            }
            r.price_index[d] = p_min * f64::powf(phi, -1.0 / theta);
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
    fn parallel_splits_do_not_depend_on_the_thread_count() {
        // enough provinces and gateways that the loops over them split into several tasks
        let provinces = (0..300).map(|i| (i / 100) as u16).collect();
        let gateways = (0..24).map(|i| (i / 8) as u16).collect();
        let mut r = rng(7);
        let (net, wedges) = complete_world(&mut r, provinces, gateways);
        let k = kernel(&net, &wedges);
        let np = net.n_provinces();
        assert!(np > 4 * ROWS && net.n_gateways() > 16);
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
                source(&k, 4, &price, &active, &spend, &mut out);
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
                source(&k, g, &price, &active, &spend, &mut out);
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
        let p = TradeParams::default();
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
            source(&k, g, &price, &active, &spend, &mut base);
            for lambda in [1e-6, 1e3] {
                let scaled: Vec<f64> = price.iter().map(|p| p * lambda).collect();
                let mut out = Sourced::new();
                source(&k, g, &scaled, &active, &spend, &mut out);
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
                source(&k, g, &scaled, &active, &spend_l, &mut out);
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
            let (mut m_in, mut m_out) = (vec![0.0; k.n_gateways()], vec![0.0; k.n_gateways()]);
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

    /// Kernel, price, active, spend and fill.
    type GatherCase = (Kernel, Vec<f64>, Vec<bool>, Vec<[f64; 2]>, Vec<[f64; 2]>);

    /// 2 to 4 countries of 1 to 5 provinces and 1 to 3 gateways each, with random prices,
    /// inactive origins, and spending and fills that are often 0 or 1.
    fn gather_case(seed: u64) -> GatherCase {
        let mut rng = rng(seed);
        let n_countries = 2 + rng.below(3);
        let (mut provinces, mut gateways) = (vec![], vec![]);
        for c in 0..n_countries as u16 {
            provinces.extend(std::iter::repeat_n(c, 1 + rng.below(5)));
            gateways.extend(std::iter::repeat_n(c, 1 + rng.below(3)));
        }
        let (net, wedges) = complete_world(&mut rng, provinces, gateways);
        let n = net.n_provinces();
        let amount = |rng: &mut Rng, hi: f64| match rng.below(5) {
            0 => 0.0,
            1 => 1.0,
            _ => hi * rng.next(),
        };
        let price = (0..n)
            .map(|_| 10f64.powf(-2.0 + 4.0 * rng.next()))
            .collect();
        let active = (0..n).map(|_| rng.below(5) != 0).collect();
        let spend = (0..n)
            .map(|_| [amount(&mut rng, 1000.0), amount(&mut rng, 1000.0)])
            .collect();
        let fill = (0..n)
            .map(|_| [amount(&mut rng, 1.0), amount(&mut rng, 1.0)])
            .collect();
        (kernel(&net, &wedges), price, active, spend, fill)
    }

    fn assert_same_dispatch(g: usize, gather: &Sourced, scatter: &Sourced) {
        let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        let bits2 = |v: &[[f64; 2]]| v.iter().flatten().map(|x| x.to_bits()).collect::<Vec<_>>();
        let fields: [(&str, &[f64], &[f64]); 9] = [
            ("shipped", &gather.shipped, &scatter.shipped),
            ("paid", &gather.paid, &scatter.paid),
            ("received", &gather.received, &scatter.received),
            ("tariff", &gather.tariff, &scatter.tariff),
            (
                "domestic_value",
                &gather.domestic_value,
                &scatter.domestic_value,
            ),
            (
                "access_in_value",
                &gather.access_in_value,
                &scatter.access_in_value,
            ),
            (
                "crossing_value",
                &gather.crossing_value,
                &scatter.crossing_value,
            ),
            (
                "crossing_units",
                &gather.crossing_units,
                &scatter.crossing_units,
            ),
            (
                "access_out_value",
                &gather.access_out_value,
                &scatter.access_out_value,
            ),
        ];
        for (name, a, b) in fields {
            assert_eq!(bits(a), bits(b), "good {g}: {name}");
        }
        for (slot, (a, b)) in gather.arrive.iter().zip(&scatter.arrive).enumerate() {
            assert_eq!(bits2(a), bits2(b), "good {g}: arrive[{slot}]");
        }
        assert_eq!(
            bits2(&gather.paid_by_class),
            bits2(&scatter.paid_by_class),
            "good {g}: paid_by_class"
        );
        assert_eq!(
            gather.lost.to_bits(),
            scatter.lost.to_bits(),
            "good {g}: lost"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(300))]

        #[test]
        fn gather_dispatch_equals_scatter_bit_for_bit(seed in any::<u64>(), g in 0..7usize) {
            let (k, price, active, spend, fill) = gather_case(seed);
            let mut gather = Sourced::new();
            source(&k, g, &price, &active, &spend, &mut gather);
            let mut scatter = gather.clone();
            dispatch(&k, g, &price, &fill, &mut gather);
            dispatch_scatter(&k, g, &price, &fill, &mut scatter);
            assert_same_dispatch(g, &gather, &scatter);
        }
    }

    #[test]
    fn gather_dispatch_equals_scatter_across_chunks() {
        // more provinces than one task takes, so the origin and destination passes split
        let provinces = (0..300).map(|i| (i / 100) as u16).collect();
        let gateways = (0..24).map(|i| (i / 8) as u16).collect();
        let mut r = rng(5);
        let (net, wedges) = complete_world(&mut r, provinces, gateways);
        let k = kernel(&net, &wedges);
        let np = net.n_provinces();
        assert!(np > 4 * ROWS);
        let price = draw(&mut r, np, 0.5, 2.0);
        let active: Vec<bool> = (0..np).map(|o| o % 7 != 3).collect();
        let spend = draw2(&mut r, np, 0.0, 10.0);
        let fill = draw2(&mut r, np, 0.0, 1.0);
        for g in 0..7 {
            let mut gather = Sourced::new();
            source(&k, g, &price, &active, &spend, &mut gather);
            let mut scatter = gather.clone();
            dispatch(&k, g, &price, &fill, &mut gather);
            dispatch_scatter(&k, g, &price, &fill, &mut scatter);
            assert_same_dispatch(g, &gather, &scatter);
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
