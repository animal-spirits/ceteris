//! Two-level trade kernel (`specs/trade/SPEC.md`, Equations).
//!
//! Domestic pairs use the direct leg, `K_od = tau_od^-theta_g`, dense within a country. A
//! cross-border pair is reached through gateways, `K_od = sum_ab K_oa m_a K_ab m_b K_bd` with
//! the gateway masses m (ADR 0092), and never materialized: [`Kernel::origin_access_into`]
//! evaluates the sums through gateway aggregates. Factors are f64, which holds `tau^-theta`
//! down to about 1e-308 (tau 1e4 at theta 64.85, ADR 0093).

use std::sync::atomic::{AtomicU64, Ordering};

use rayon::prelude::*;

use ceteris_core::rows::Rows;

use crate::goods::Goods;
use crate::network::{Leg, Network, Wedges};
use crate::params::TradeParams;

/// Longest international transit lag in months.
const MAX_LAG: f64 = 3.0;
/// Hours per month for transit lags.
const HOURS_PER_MONTH: f64 = 730.0;
/// Largest deviation from 1 of a country's summed gateway masses.
const MASS_TOLERANCE: f64 = 1e-12;

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

/// Entries of a [`Csr`] grouped by column, stable: the entries of column c are
/// `pairs[offsets[c]..offsets[c + 1]]`, as (row, index into `Csr::entries`) in row order.
#[derive(Clone, Debug, Default)]
pub struct ByColumn {
    pub offsets: Vec<u32>,
    pub pairs: Vec<(u32, u32)>,
}

impl ByColumn {
    fn new<E>(csr: &Csr<E>, col: impl Fn(&E) -> usize, n_cols: usize) -> Self {
        let mut offsets = vec![0u32; n_cols + 1];
        for e in &csr.entries {
            offsets[col(e) + 1] += 1;
        }
        for c in 0..n_cols {
            offsets[c + 1] += offsets[c];
        }
        let mut next = offsets.clone();
        let mut pairs = vec![(0, 0); csr.entries.len()];
        for row in 0..csr.n_rows() {
            let lo = csr.offsets[row];
            for (j, e) in csr.row(row).iter().enumerate() {
                let c = col(e);
                pairs[next[c] as usize] = (row as u32, lo + j as u32);
                next[c] += 1;
            }
        }
        Self { offsets, pairs }
    }

    pub fn column(&self, c: usize) -> &[(u32, u32)] {
        &self.pairs[self.offsets[c] as usize..self.offsets[c + 1] as usize]
    }
}

/// A domestic or access leg: `k = tau^-theta_g` with `tau = 1 + c w + lambda T` on the
/// least-cost leg (S05), or the route-choice factor when the network carries one, and
/// `inv_tau = 1 / tau`, the share of the units shipped over the leg that arrive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub col: u32,
    pub k: f64,
    pub inv_tau: f64,
}

/// A gateway crossing a to b. With `tau` the leg factor, `1 + nu` the iceberg border loss,
/// destroyed at the crossing, and t the tariff, a routing wedge only:
/// `k = m_a m_b (tau (1 + nu) (1 + t))^-theta_g` with the gateway masses m (ADR 0092),
/// `inv_duty = 1 / (1 + t)`, the share of the value the importer pays that is not tariff, and
/// `inv_loss = 1 / (tau (1 + nu))`, the share of the units shipped that arrive. `lag` is the
/// extra transit time in months, `round(hours / 730)` capped at 3, from the least-cost leg's
/// hours or the good's expected route hours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    pub col: u32,
    pub k: f64,
    pub inv_duty: f64,
    pub inv_loss: f64,
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
    /// Whether gateway a has a crossing with `k > 0` into a gateway of country B, at
    /// `a * n_countries + B`.
    pub links: Vec<bool>,
}

/// Fields are private so that every change goes through a method that renews `id`.
#[derive(Clone, Debug)]
pub struct Kernel {
    n_provinces: usize,
    n_gateways: usize,
    n_countries: usize,
    province_country: Vec<u16>,
    gateway_country: Vec<u16>,
    /// P01 per good times P150, as used to build `k`; sourcing needs it for `p^-theta_g`. Kernels are
    /// per good: in P2 each good is its own weight class, so a theta class adds no kernel
    /// (ADR 0018).
    theta: Vec<f64>,
    goods: Vec<Option<GoodKernel>>,
    /// Crossings by destination gateway and access legs by gateway, the same for every good:
    /// all goods share the network's legs and their order.
    crossings_by_target: ByColumn,
    access_by_gateway: ByColumn,
    domestic_by_destination: ByColumn,
    /// Unique per build and per change, so `source` can tell a kernel it has seen.
    id: u64,
}

/// Results of the origin-side low-rank evaluation for one good, written by
/// [`Kernel::origin_access_into`], in log scales per country (ADR 0093): a value x here stands
/// for `x e^s` with a shift s of its country. Origins, their gateways and each destination's
/// home part (its home and domestic routes) use the origin shift of their country, the log
/// weight of its best origin. Destination gateways and each destination's abroad part use the
/// destination shift of their country, the largest origin shift among the foreign countries
/// with a gateway that has access and a crossing with `k > 0` into it. Each destination
/// combines its two parts in log space, so neither part is measured in the other's scale, and
/// a term lost to underflow is below about 1e-308 of its part's scale, kernel factors aside.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Access {
    /// Origin access at each gateway, `v'_a = sum_{o in A} K_oa w_o`, in A's origin scale.
    pub v: Vec<f64>,
    /// Origin shift per country, as passed in.
    pub shift: Vec<f64>,
    /// Destination shift per country, 0 for a country that no foreign country supplies.
    pub dest_shift: Vec<f64>,
    /// `exp(shift_A - dest_shift_B)` at `A * n_countries + B`, at most 1: moves a crossing's
    /// terms from the origin scale of A into the destination scale of B; 0 unless a gateway of
    /// A has access and a crossing with `k > 0` into B.
    pub carry: Vec<f64>,
    /// Access of each destination gateway to every foreign origin,
    /// `u'_b = sum_a v'_a K_ab carry(A, B)`, in B's destination scale.
    pub u: Vec<f64>,
    /// Home part of each destination's access, `w_d + sum_{o in B} K_od w_o`, in B's origin
    /// scale.
    pub home: Vec<f64>,
    /// Abroad part of each destination's access, `sum_b K_bd u'_b`, in B's destination scale.
    pub abroad: Vec<f64>,
    /// Whether some gateway of A with access links to B, at `A * n_countries + B`.
    linked: Vec<bool>,
}

/// A part of a destination's access below the smallest normal f64 is underflow of kernel
/// factors, not a path.
fn present(x: f64) -> bool {
    x >= f64::MIN_POSITIVE
}

impl Access {
    /// `ln Phi_d` of destination d of country c, relative to the scale of weight 1 at the
    /// shift 0 (`p_min^-theta` for [`crate::source::origin_weights`]); minus infinity when d
    /// reaches no active origin.
    pub fn ln_phi(&self, d: usize, c: usize) -> f64 {
        let (home, abroad) = (self.home[d], self.abroad[d]);
        let (s, t) = (self.shift[c], self.dest_shift[c]);
        match (present(home), present(abroad)) {
            (false, false) => f64::NEG_INFINITY,
            (true, false) => s + libm::log(home),
            (false, true) => t + libm::log(abroad),
            (true, true) if t >= s => t + libm::log(abroad + home * libm::exp(s - t)),
            (true, true) => s + libm::log(home + abroad * libm::exp(t - s)),
        }
    }

    /// Spending per unit of access of destination d of country c, `E / Phi_d` for E = 1, in
    /// the scale of its home part and of its abroad part; 0 for a part that is not present.
    /// Each is finite, at most `1 / f64::MIN_POSITIVE`; times a spending it can overflow, which
    /// [`crate::source::source`] handles with a binary scale.
    pub fn reach(&self, d: usize, c: usize) -> (f64, f64) {
        let (home, abroad) = (self.home[d], self.abroad[d]);
        let (s, t) = (self.shift[c], self.dest_shift[c]);
        let (on_home, on_abroad) = (present(home), present(abroad));
        // the other part in this part's scale; when exp(gap) alone overflows, a part near
        // the smallest normal f64 can still bring the product back into range
        let cross = |x: f64, gap: f64| {
            let e = libm::exp(gap);
            if e.is_finite() {
                x * e
            } else {
                libm::exp(libm::log(x) + gap)
            }
        };
        // only a total beyond f64 overflows: its part's share is below part / f64::MAX
        let inv = |total: f64| if total.is_finite() { 1.0 / total } else { 0.0 };
        let at_home = match (on_home, on_abroad) {
            (false, _) => 0.0,
            (true, false) => inv(home),
            (true, true) => inv(home + cross(abroad, t - s)),
        };
        let at_abroad = match (on_abroad, on_home) {
            (false, _) => 0.0,
            (true, false) => inv(abroad),
            (true, true) => inv(abroad + cross(home, s - t)),
        };
        (at_home, at_abroad)
    }
}

impl Kernel {
    /// `weight` is tonnes per unit of value per good of `p.goods`. Panics on legs that break
    /// the [`Network`] contract, and on parameters or weights not sized to the goods. Tariffs
    /// are looked up with the country count implied by `wedges`.
    pub fn build(net: &Network, weight: &[f64], wedges: &Wedges, p: &TradeParams) -> Kernel {
        if let Err(e) = p.check_goods() {
            panic!("{e}");
        }
        assert_eq!(weight.len(), p.goods.n(), "weight per good");
        let (np, ng) = (net.n_provinces(), net.n_gateways());
        validate(net, np, ng, &p.goods);
        let (want, built) = (
            p.bulk_rate(),
            net.bulk.as_ref().map(|b| (b.classes, b.sea, b.port)),
        );
        assert!(
            want == built,
            "bulk freight rate {want:?} of the parameters against {built:?} of the network (ADR 0247)"
        );
        if let Some(gl) = &net.good_legs {
            for g in p.goods.traded_goods() {
                let want = crate::network::kappa(weight[g], p.time_cost[g]);
                assert!(
                    want.to_bits() == gl.kappa[g].to_bits(),
                    "good {g}: kappa {want} of the parameters against {} of the network's good legs (ADR 0279)",
                    gl.kappa[g]
                );
            }
        }
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
        let n_countries = net
            .province_country
            .iter()
            .chain(&net.gateway_country)
            .max()
            .map_or(0, |&c| usize::from(c) + 1);
        let theta: Vec<f64> = p.theta.iter().map(|t| t * p.theta_scale).collect();
        let goods: Vec<Option<GoodKernel>> = (0..p.goods.n())
            .map(|g| {
                p.goods.is_traded(g).then(|| {
                    good_kernel(net, np, ng, n_countries, g, weight[g], theta[g], wedges, p)
                })
            })
            .collect();
        let first: Option<&GoodKernel> = goods.iter().flatten().next();
        let (crossings_by_target, access_by_gateway, domestic_by_destination) =
            first.map_or_else(Default::default, |gk| {
                (
                    ByColumn::new(&gk.crossings, |e| e.col as usize, ng),
                    ByColumn::new(&gk.access, |e| e.col as usize, ng),
                    ByColumn::new(&gk.domestic, |e| e.col as usize, np),
                )
            });
        Kernel {
            n_provinces: np,
            n_gateways: ng,
            n_countries,
            province_country: net.province_country.clone(),
            gateway_country: net.gateway_country.clone(),
            theta,
            goods,
            crossings_by_target,
            access_by_gateway,
            domestic_by_destination,
            id: next_id(),
        }
    }

    pub fn n_provinces(&self) -> usize {
        self.n_provinces
    }

    pub fn n_gateways(&self) -> usize {
        self.n_gateways
    }

    /// One more than the largest country index of a province or gateway.
    pub fn n_countries(&self) -> usize {
        self.n_countries
    }

    pub fn province_country(&self) -> &[u16] {
        &self.province_country
    }

    pub fn gateway_country(&self) -> &[u16] {
        &self.gateway_country
    }

    /// Goods the kernel was built for, traded and local.
    pub fn n_goods(&self) -> usize {
        self.theta.len()
    }

    /// P01 of good `g` times P150, as the kernel of `g` was built with.
    pub fn theta(&self, g: usize) -> f64 {
        self.theta[g]
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

    /// Every good's `crossings` grouped by destination gateway b: (origin gateway a, entry).
    pub fn crossings_by_target(&self) -> &ByColumn {
        &self.crossings_by_target
    }

    /// Every good's `access` grouped by gateway: (province, entry).
    pub fn access_by_gateway(&self) -> &ByColumn {
        &self.access_by_gateway
    }

    /// Every good's `domestic` legs grouped by destination: (origin, entry).
    pub fn domestic_by_destination(&self) -> &ByColumn {
        &self.domestic_by_destination
    }

    /// Origin-side low-rank evaluation into reused buffers, in the country scales of
    /// [`Access`]. `weight` holds each origin's weight relative to its country's best,
    /// `w_o = exp(ln_w_o)` as [`crate::source::origin_weights`] writes `ln_w_o`, 0 for an
    /// inactive origin; `shift` the origin shift per country, minus infinity for a country
    /// without an active origin. Writes `v'_a = sum_{o in A} K_oa w_o`,
    /// `u'_b = sum_a v'_a K_ab carry(A, B)` and the two parts of
    /// `Phi_d = w_d + sum_{o in B} K_od w_o + sum_b K_bd u'_b`, home term `K_dd w_d = w_d`
    /// included.
    pub fn origin_access_into(&self, g: usize, weight: &[f64], shift: &[f64], out: &mut Access) {
        assert_eq!(weight.len(), self.n_provinces);
        assert_eq!(shift.len(), self.n_countries);
        let gk = self.good(g);
        let (gc, nc) = (&self.gateway_country, self.n_countries);

        out.v.clear();
        out.v.resize(self.n_gateways, 0.0);
        for (o, &w) in weight.iter().enumerate() {
            for e in gk.access.row(o) {
                out.v[e.col as usize] += e.k * w;
            }
        }
        let v = &out.v;

        out.shift.clear();
        out.shift.extend_from_slice(shift);
        out.linked.clear();
        out.linked.resize(nc * nc, false);
        let dest = &mut out.dest_shift;
        dest.clear();
        dest.resize(nc, f64::NEG_INFINITY);
        for (a, &ca) in gc.iter().enumerate().filter(|&(a, _)| v[a] > 0.0) {
            let ca = usize::from(ca);
            let links = &gk.links[a * nc..][..nc];
            for (cb, _) in links.iter().enumerate().filter(|(_, l)| **l) {
                out.linked[ca * nc + cb] = true;
                dest[cb] = dest[cb].max(shift[ca]);
            }
        }
        for s in dest.iter_mut().filter(|s| **s == f64::NEG_INFINITY) {
            *s = 0.0;
        }
        let (dest, linked) = (&out.dest_shift, &out.linked);
        out.carry.clear();
        out.carry.resize(nc * nc, 0.0);
        out.carry
            .par_chunks_mut(nc.max(1))
            .zip(linked.par_chunks(nc.max(1)))
            .enumerate()
            .for_each(|(ca, (row, linked))| {
                for ((x, &d), _) in row.iter_mut().zip(dest).zip(linked).filter(|(_, l)| **l) {
                    *x = libm::exp(shift[ca] - d);
                }
            });
        let carry = &out.carry;

        // u and Phi gather what flows into each gateway and province, in the order of the
        // rows feeding them, and run in parallel
        let u = &mut out.u;
        u.clear();
        u.resize(self.n_gateways, 0.0);
        u.par_iter_mut().enumerate().for_each(|(b, u)| {
            let cb = usize::from(gc[b]);
            for &(a, j) in self.crossings_by_target.column(b) {
                let ca = usize::from(gc[a as usize]);
                *u += v[a as usize] * gk.crossings.entries[j as usize].k * carry[ca * nc + cb];
            }
        });
        let u = &out.u;
        out.home.clear();
        out.home.extend_from_slice(weight);
        out.abroad.clear();
        out.abroad.resize(self.n_provinces, 0.0);
        out.home
            .par_iter_mut()
            .zip(out.abroad.par_iter_mut())
            .enumerate()
            .with_min_len(64)
            .for_each(|(d, (home, abroad))| {
                for &(o, j) in self.domestic_by_destination.column(d) {
                    *home += gk.domestic.entries[j as usize].k * weight[o as usize];
                }
                *abroad = gk
                    .access
                    .row(d)
                    .iter()
                    .map(|e| e.k * u[e.col as usize])
                    .sum();
            });
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

fn validate(net: &Network, np: usize, ng: usize, goods: &Goods) {
    assert_eq!(
        net.gateway_mass.len(),
        ng,
        "gateway masses: {} for {ng} gateways",
        net.gateway_mass.len()
    );
    let n_countries = net
        .gateway_country
        .iter()
        .max()
        .map_or(0, |&c| usize::from(c) + 1);
    let mut total = vec![0.0; n_countries];
    for (a, (&m, &c)) in net
        .gateway_mass
        .iter()
        .zip(&net.gateway_country)
        .enumerate()
    {
        // also rejects NaN
        assert!(m > 0.0 && m <= 1.0, "gateway {a} has mass {m}");
        total[usize::from(c)] += m;
    }
    for (c, t) in total.iter().enumerate() {
        assert!(
            *t == 0.0 || (t - 1.0).abs() <= MASS_TOLERANCE,
            "gateway masses of country {c} sum to {t}"
        );
    }
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
    if let Some(b) = &net.bulk {
        assert!(
            net.route_taus.is_none(),
            "route taus and bulk legs together (ADR 0247)"
        );
        for (name, bulk, base) in [
            ("domestic", &b.domestic, &net.domestic),
            ("access", &b.access, &net.access),
            ("crossing", &b.crossings, &net.crossings),
        ] {
            assert_eq!(bulk.len(), base.len(), "bulk {name} legs: count");
            for (x, y) in bulk.iter().zip(base) {
                assert!(
                    (x.from, x.to) == (y.from, y.to),
                    "bulk {name} leg {x:?} against base {y:?}"
                );
                assert!(
                    x.cost_per_t >= 0.0 && x.hours >= 0.0,
                    "negative bulk leg cost: {x:?}"
                );
            }
        }
    }
    if let Some(gl) = &net.good_legs {
        assert!(
            net.route_taus.is_none(),
            "route taus and good legs together (ADR 0279)"
        );
        assert!(
            gl.set.len() == goods.traded && gl.kappa.len() == goods.traded,
            "good legs: {} sets and {} kappas for {} traded goods",
            gl.set.len(),
            gl.kappa.len(),
            goods.traded
        );
        for g in goods.traded_goods() {
            assert!(
                usize::from(gl.set[g]) < gl.sets.len(),
                "good {g}: leg set {} of {}",
                gl.set[g],
                gl.sets.len()
            );
        }
        for (s, set) in gl.sets.iter().enumerate() {
            for (name, legs, base) in [
                ("domestic", &set.domestic, &net.domestic),
                ("access", &set.access, &net.access),
                ("crossing", &set.crossings, &net.crossings),
            ] {
                assert_eq!(
                    legs.len(),
                    base.len(),
                    "good leg set {s}, {name} legs: count"
                );
                for (x, y) in legs.iter().zip(base) {
                    assert!(
                        (x.from, x.to) == (y.from, y.to),
                        "good leg set {s}, {name} leg {x:?} against base {y:?}"
                    );
                    assert!(
                        x.cost_per_t >= 0.0 && x.hours >= 0.0,
                        "negative good leg cost: {x:?}"
                    );
                }
            }
        }
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
                for g in goods.traded_goods() {
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

#[allow(clippy::too_many_arguments)]
fn good_kernel(
    net: &Network,
    np: usize,
    ng: usize,
    nc: usize,
    g: usize,
    w: f64,
    theta: f64,
    wedges: &Wedges,
    p: &TradeParams,
) -> GoodKernel {
    let least = |l: &Leg| 1.0 + l.cost_per_t * w + p.time_cost[g] * l.hours / 24.0;
    let rt = net.route_taus.as_ref();
    let (domestic, access, crossing_legs) = match (&net.good_legs, &net.bulk) {
        (Some(gl), _) => {
            let s = &gl.sets[usize::from(gl.set[g])];
            (&s.domestic, &s.access, &s.crossings)
        }
        (None, Some(b)) if b.classes[usize::from(p.theta_class[g])] => {
            (&b.domestic, &b.access, &b.crossings)
        }
        _ => (&net.domestic, &net.access, &net.crossings),
    };
    let k_of = |total: f64| libm::pow(total, -theta);
    let plain = |taus: Option<&Rows<f64>>, i: usize, l: &Leg| {
        let tau = taus.map_or_else(|| least(l), |t| t[i][g]);
        Entry {
            col: l.to,
            k: k_of(tau),
            inv_tau: 1.0 / tau,
        }
    };
    let group = p.goods.tariff_group(g);
    let border_tau = 1.0 + p.border_nu_of(g);
    let mass = &net.gateway_mass;
    let crossing = |i: usize, l: &Leg| {
        let (tau, hours) = rt.map_or_else(
            || (least(l), l.hours),
            |rt| (rt.crossings[i][g], rt.crossing_hours[i][g]),
        );
        let (a, b) = (l.from as usize, l.to as usize);
        let importer = usize::from(net.gateway_country[b]);
        let exporter = usize::from(net.gateway_country[a]);
        let duty = 1.0 + f64::from(wedges.rate(importer, exporter, group));
        let loss = tau * border_tau;
        Crossing {
            col: l.to,
            k: mass[a] * mass[b] * k_of(loss * duty),
            inv_duty: 1.0 / duty,
            inv_loss: 1.0 / loss,
            lag: (hours / HOURS_PER_MONTH).round().min(MAX_LAG) as u8,
        }
    };
    let crossings = Csr::from_legs(ng, crossing_legs, crossing);
    let mut links = vec![false; ng * nc];
    for a in 0..ng {
        for e in crossings.row(a).iter().filter(|e| e.k > 0.0) {
            links[a * nc + usize::from(net.gateway_country[e.col as usize])] = true;
        }
    }
    GoodKernel {
        domestic: Csr::from_legs(np, domestic, |i, l| plain(rt.map(|r| &r.domestic), i, l)),
        access: Csr::from_legs(np, access, |i, l| plain(rt.map(|r| &r.access), i, l)),
        crossings,
        links,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::goods::Goods;
    use crate::goods::p2::{N as N_GOODS, TRADED};
    use crate::network::{
        BulkLegs, GoodLegs, Leg, LegSet, LegTaus, Network, Wedges, equal_gateway_mass,
    };
    use crate::params::TradeParams;
    use ceteris_core::rows::Rows;

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

    /// tau of a leg by the spec formula, as the kernel computes it.
    fn leg_tau(p: &TradeParams, g: usize, leg: &Leg) -> f64 {
        1.0 + leg.cost_per_t * W[g] + p.time_cost[g] * leg.hours / 24.0
    }

    /// K of a leg by the spec formula, independent of the implementation.
    fn leg_k(p: &TradeParams, g: usize, leg: &Leg, extra: f64) -> f64 {
        f64::powf(leg_tau(p, g, leg) * extra, -p.theta[g])
    }

    /// Provinces 0..5 in country 0, 5..10 in country 1; gateways 0 and 1 in country 0 with
    /// masses 0.3 and 0.7, gateway 2 in country 1. Costs random.
    fn world(rng: &mut Rng) -> Network {
        let province_country: Vec<u16> = (0..10).map(|i| u16::from(i >= 5)).collect();
        let gateway_country = vec![0, 0, 1];
        let mut net = Network {
            province_country: province_country.clone(),
            gateway_country: gateway_country.clone(),
            gateway_mass: vec![0.3, 0.7, 1.0],
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

    /// Access with every country that has a weight at shift 0, so values are unscaled.
    fn origin_access(k: &Kernel, g: usize, weight: &[f64]) -> Access {
        let mut shift = vec![f64::NEG_INFINITY; k.n_countries()];
        for (o, &w) in weight.iter().enumerate() {
            if w > 0.0 {
                shift[usize::from(k.province_country()[o])] = 0.0;
            }
        }
        let mut out = Access::default();
        k.origin_access_into(g, weight, &shift, &mut out);
        out
    }

    /// The entry of a leg, found by (row, col): entries keep leg order within a row.
    fn entry(csr: &Csr<Entry>, l: &Leg) -> Entry {
        *csr.row(l.from as usize)
            .iter()
            .find(|e| e.col == l.to)
            .unwrap()
    }

    fn crossing(csr: &Csr<Crossing>, l: &Leg) -> Crossing {
        *csr.row(l.from as usize)
            .iter()
            .find(|e| e.col == l.to)
            .unwrap()
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
        let p = TradeParams::default().bulk_off();
        let net = two_provinces(150.0, 48.0);
        let k = Kernel::build(&net, &W, &Wedges::none(1), &p);
        let e = k.good(4).domestic.row(0)[0];
        // 1 + 150 * 0.002 + 0.010 * 2 days
        assert!((e.inv_tau - 1.0 / 1.32).abs() < 1e-12);
        let want = f64::powf(1.32, -p.theta[4]);
        assert!((e.k - want).abs() <= want * 1e-12);
        assert_eq!(e.col, 1);
    }

    #[test]
    fn kernel_uses_theta_per_good() {
        let mut p = TradeParams::default().bulk_off();
        p.theta[0] = 2.0;
        p.theta[4] = 8.0;
        let net = world(&mut Rng(5));
        let mut wedges = Wedges::none(2);
        wedges.tariff.iter_mut().for_each(|t| *t = 0.2);
        let k = Kernel::build(&net, &W, &wedges, &p);
        assert_eq!((k.theta(0), k.theta(4)), (2.0, 8.0));
        for g in [0, 4] {
            let nu = 1.0 + p.border_nu_of(g);
            let gk = k.good(g);
            for (csr, legs) in [(&gk.domestic, &net.domestic), (&gk.access, &net.access)] {
                for l in legs {
                    let e = entry(csr, l);
                    let tau = leg_tau(&p, g, l);
                    assert_eq!((e.k, e.inv_tau), (libm::pow(tau, -p.theta[g]), 1.0 / tau));
                }
            }
            for l in &net.crossings {
                let e = crossing(&gk.crossings, l);
                let (loss, duty) = (leg_tau(&p, g, l) * nu, 1.0 + f64::from(0.2f32));
                let m = net.gateway_mass[l.from as usize] * net.gateway_mass[l.to as usize];
                assert_eq!(e.k, m * libm::pow(loss * duty, -p.theta[g]));
                assert_eq!((e.inv_loss, e.inv_duty), (1.0 / loss, 1.0 / duty));
            }
        }
        // same legs, the other theta: the factors differ
        let e = k.good(0).domestic.entries[0];
        assert_ne!(e.k, libm::pow(1.0 / e.inv_tau, -8.0));
    }

    /// ADR 0247: goods of a class with bulk legs take the bulk legs' cost and hours on every leg
    /// list; the other goods keep the base legs. Bulk legs equal to the base legs change nothing.
    #[test]
    fn bulk_legs_price_the_bulk_classes() {
        let p = bulk_params(0.5, 0.25);
        let base = world(&mut Rng(9));
        let mut rng = Rng(21);
        let redraw = |legs: &[Leg], rng: &mut Rng| -> Vec<Leg> {
            legs.iter().map(|l| rng.leg(l.from, l.to)).collect()
        };
        let mut classes = [false; crate::params::N_CLASSES];
        classes[3] = true;
        let net = Network {
            bulk: Some(BulkLegs {
                classes,
                sea: 0.5,
                port: 0.25,
                domestic: redraw(&base.domestic, &mut rng),
                access: redraw(&base.access, &mut rng),
                crossings: redraw(&base.crossings, &mut rng),
            }),
            ..base.clone()
        };
        let bulk = net.bulk.as_ref().unwrap();
        let wedges = Wedges::none(2);
        let k = Kernel::build(&net, &W, &wedges, &p);
        for g in 0..TRADED {
            let on = p.theta_class[g] == 3;
            let pick = |b: &Vec<Leg>, n: &Vec<Leg>| if on { b.clone() } else { n.clone() };
            let gk = k.good(g);
            for (csr, legs) in [
                (&gk.domestic, pick(&bulk.domestic, &net.domestic)),
                (&gk.access, pick(&bulk.access, &net.access)),
            ] {
                for l in &legs {
                    let tau = leg_tau(&p, g, l);
                    assert_eq!(entry(csr, l).inv_tau, 1.0 / tau, "good {g}");
                }
            }
            let nu = 1.0 + p.border_nu_of(g);
            for l in &pick(&bulk.crossings, &net.crossings) {
                let e = crossing(&gk.crossings, l);
                assert_eq!(e.inv_loss, 1.0 / (leg_tau(&p, g, l) * nu), "good {g}");
                let lag = (l.hours / HOURS_PER_MONTH).round().min(MAX_LAG) as u8;
                assert_eq!(e.lag, lag, "good {g}");
            }
        }
        let same = Network {
            bulk: Some(BulkLegs {
                classes,
                sea: 0.5,
                port: 0.25,
                domestic: base.domestic.clone(),
                access: base.access.clone(),
                crossings: base.crossings.clone(),
            }),
            ..base.clone()
        };
        let (a, b) = (
            Kernel::build(&base, &W, &wedges, &TradeParams::default().bulk_off()),
            Kernel::build(&same, &W, &wedges, &p),
        );
        for g in 0..TRADED {
            assert_eq!(a.good(g).crossings.entries, b.good(g).crossings.entries);
            assert_eq!(a.good(g).domestic.entries, b.good(g).domestic.entries);
            assert_eq!(a.good(g).access.entries, b.good(g).access.entries);
        }
    }

    /// Good legs (ADR 0279) for `net`: good g on set g % 2, each set's legs redrawn.
    fn good_legs(net: &Network, p: &TradeParams, rng: &mut Rng) -> GoodLegs {
        let mut redraw =
            |legs: &[Leg]| -> Vec<Leg> { legs.iter().map(|l| rng.leg(l.from, l.to)).collect() };
        let sets = (0..2)
            .map(|_| LegSet {
                domestic: redraw(&net.domestic),
                access: redraw(&net.access),
                crossings: redraw(&net.crossings),
            })
            .collect();
        GoodLegs {
            set: (0..TRADED).map(|g| (g % 2) as u8).collect(),
            kappa: (0..TRADED)
                .map(|g| crate::network::kappa(W[g], p.time_cost[g]))
                .collect(),
            sets,
        }
    }

    #[test]
    fn good_legs_price_each_good_on_its_set() {
        let p = TradeParams::default();
        let base = world(&mut Rng(9));
        let mut classes = [false; crate::params::N_CLASSES];
        classes[3] = true;
        let net = Network {
            bulk: Some(BulkLegs {
                classes,
                sea: p.bulk_sea[3],
                port: p.bulk_port[3],
                domestic: base.domestic.clone(),
                access: base.access.clone(),
                crossings: base.crossings.clone(),
            }),
            good_legs: Some(good_legs(&base, &p, &mut Rng(33))),
            ..base.clone()
        };
        let gl = net.good_legs.as_ref().unwrap();
        let k = Kernel::build(&net, &W, &Wedges::none(2), &p);
        for g in 0..TRADED {
            let (s, gk) = (&gl.sets[g % 2], k.good(g));
            for (csr, legs) in [(&gk.domestic, &s.domestic), (&gk.access, &s.access)] {
                for l in legs {
                    assert_eq!(entry(csr, l).inv_tau, 1.0 / leg_tau(&p, g, l), "good {g}");
                }
            }
            let nu = 1.0 + p.border_nu_of(g);
            for l in &s.crossings {
                let e = crossing(&gk.crossings, l);
                assert_eq!(e.inv_loss, 1.0 / (leg_tau(&p, g, l) * nu), "good {g}");
                let lag = (l.hours / HOURS_PER_MONTH).round().min(MAX_LAG) as u8;
                assert_eq!(e.lag, lag, "good {g}");
            }
        }
    }

    #[test]
    #[should_panic(expected = "kappa")]
    fn good_legs_must_match_the_time_costs() {
        let mut p = TradeParams::default().bulk_off();
        let base = world(&mut Rng(9));
        let net = Network {
            good_legs: Some(good_legs(&base, &p, &mut Rng(33))),
            ..base
        };
        p.time_cost[4] *= 2.0;
        Kernel::build(&net, &W, &Wedges::none(2), &p);
    }

    #[test]
    #[should_panic(expected = "good leg set 1, crossing leg")]
    fn good_legs_must_match_the_base_legs() {
        let p = TradeParams::default().bulk_off();
        let base = world(&mut Rng(9));
        let mut gl = good_legs(&base, &p, &mut Rng(33));
        gl.sets[1].crossings.swap(0, 1);
        let net = Network {
            good_legs: Some(gl),
            ..base
        };
        Kernel::build(&net, &W, &Wedges::none(2), &p);
    }

    #[test]
    #[should_panic(expected = "bulk crossing leg")]
    fn bulk_legs_must_match_the_base_legs() {
        let base = world(&mut Rng(9));
        let mut crossings = base.crossings.clone();
        crossings.swap(0, 1);
        let net = Network {
            bulk: Some(BulkLegs {
                classes: [false, false, false, true],
                sea: 0.5,
                port: 0.25,
                domestic: base.domestic.clone(),
                access: base.access.clone(),
                crossings,
            }),
            ..base
        };
        Kernel::build(&net, &W, &Wedges::none(2), &bulk_params(0.5, 0.25));
    }

    fn bulk_network(sea: f64, port: f64) -> Network {
        let base = world(&mut Rng(9));
        Network {
            bulk: Some(BulkLegs {
                classes: [false, false, false, true],
                sea,
                port,
                domestic: base.domestic.clone(),
                access: base.access.clone(),
                crossings: base.crossings.clone(),
            }),
            ..base
        }
    }

    fn bulk_params(sea: f64, port: f64) -> TradeParams {
        TradeParams {
            bulk_sea: [1.0, 1.0, 1.0, sea],
            bulk_port: [1.0, 1.0, 1.0, port],
            ..TradeParams::default().bulk_off()
        }
    }

    /// ADR 0247: P154 and P155 reach the kernel only through `Network::bulk`, so a run whose
    /// multipliers differ from the ones the network was built with is refused.
    #[test]
    fn bulk_rate_must_match_the_network() {
        let (net, wedges) = (bulk_network(0.5, 0.25), Wedges::none(2));
        Kernel::build(&net, &W, &wedges, &bulk_params(0.5, 0.25));
        let refused = |net: &Network, p: &TradeParams| {
            std::panic::catch_unwind(|| Kernel::build(net, &W, &wedges, p)).is_err()
        };
        assert!(refused(&net, &TradeParams::default().bulk_off()));
        assert!(refused(&net, &bulk_params(0.5, 0.5)));
        assert!(refused(&world(&mut Rng(9)), &bulk_params(0.5, 0.25)));
    }

    /// ADRs 0092, 0276: each good's crossings carry its own border friction, so goods of one
    /// theta class may differ.
    #[test]
    fn border_nu_follows_the_good() {
        let p = TradeParams {
            border_nu: vec![0.1, 0.25, 0.5, 2.0, 0.75, 1.5, 0.05, 0.3, 0.3, 0.3],
            ..TradeParams::default().bulk_off()
        };
        let net = world(&mut Rng(9));
        let k = Kernel::build(&net, &W, &Wedges::none(2), &p);
        for g in 0..TRADED {
            let nu = 1.0 + p.border_nu[g];
            for l in &net.crossings {
                let e = crossing(&k.good(g).crossings, l);
                let loss = leg_tau(&p, g, l) * nu;
                let m = net.gateway_mass[l.from as usize] * net.gateway_mass[l.to as usize];
                assert_eq!(e.inv_loss, 1.0 / loss, "good {g}");
                assert_eq!(e.k, m * libm::pow(loss, -p.theta[g]), "good {g}");
            }
        }
    }

    #[test]
    fn theta_scale_multiplies_every_class() {
        let net = world(&mut Rng(9));
        let mut wedges = Wedges::none(2);
        wedges.tariff.iter_mut().for_each(|t| *t = 0.1);
        let scaled = TradeParams {
            theta_scale: 1.4,
            ..TradeParams::default().bulk_off()
        };
        let mut multiplied = TradeParams::default().bulk_off();
        multiplied.theta.iter_mut().for_each(|t| *t *= 1.4);
        let (a, b) = (
            Kernel::build(&net, &W, &wedges, &scaled),
            Kernel::build(&net, &W, &wedges, &multiplied),
        );
        for g in 0..TRADED {
            assert_eq!(a.theta(g).to_bits(), b.theta(g).to_bits(), "good {g}");
            let (ga, gb) = (a.good(g), b.good(g));
            assert_eq!(ga.domestic.entries, gb.domestic.entries, "good {g}");
            assert_eq!(ga.access.entries, gb.access.entries, "good {g}");
            assert_eq!(ga.crossings.entries, gb.crossings.entries, "good {g}");
        }
        assert_ne!(
            a.good(0).domestic.entries,
            Kernel::build(&net, &W, &wedges, &TradeParams::default().bulk_off())
                .good(0)
                .domestic
                .entries
        );
    }

    #[test]
    fn local_goods_have_no_kernel() {
        let k = Kernel::build(
            &two_provinces(1.0, 1.0),
            &W,
            &Wedges::none(1),
            &TradeParams::default().bulk_off(),
        );
        assert!((0..N_GOODS).all(|g| k.try_good(g).is_some() == (g < 7)));
    }

    #[test]
    fn crossing_includes_border_and_tariff() {
        let p = TradeParams::default().bulk_off();
        let net = Network {
            province_country: vec![0, 1],
            gateway_country: vec![0, 1],
            gateway_mass: vec![1.0, 1.0],
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
        let close = |a: f64, b: f64| (a - b).abs() <= 1e-12 * b.abs();
        let e = k.good(4).crossings.row(0)[0];
        let tau = 1.0 + 100.0 * W[4] + p.time_cost[4] * 1500.0 / 24.0;
        assert!(close(e.inv_loss, 1.0 / (tau * 1.3)));
        assert!(close(e.inv_duty, 1.0 / (1.0 + f64::from(0.25f32))));
        assert_eq!(e.lag, 2);
        assert!(close(
            e.k,
            f64::powf(tau * 1.3 * (1.0 + f64::from(0.25f32)), -p.theta[4])
        ));

        // same leg, good 0 (primary, group 0)
        let e = k.good(0).crossings.row(0)[0];
        assert!(close(e.inv_duty, 1.0 / (1.0 + f64::from(0.10f32))));
        let tau = 1.0 + 100.0 * W[0] + p.time_cost[0] * 1500.0 / 24.0;
        assert!(close(
            e.k,
            f64::powf(tau * 1.3 * (1.0 + f64::from(0.10f32)), -p.theta[0])
        ));

        // 1 -> 0, good 0: lag round(5000 / 730) = 7 capped at 3; importer is country 0
        let e = k.good(0).crossings.row(1)[0];
        assert_eq!(e.lag, 3);
        assert!(close(e.inv_duty, 1.0 / (1.0 + f64::from(0.40f32))));
        // good 4 in the same direction carries no tariff
        assert_eq!(k.good(4).crossings.row(1)[0].inv_duty, 1.0);
        assert_eq!(Goods::p2().tariff_group(4), 1);

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
        let p = TradeParams::default().bulk_off();
        let mut net = world(&mut Rng(11));
        let least = Kernel::build(&net, &W, &Wedges::none(2), &p);
        // per-good factors unrelated to the least-cost legs; hours give lags 0 to 3
        let taus = |n: usize, base: f64| -> Rows<f64> {
            Rows::from_rows(
                N_GOODS,
                (0..n).map(|i| {
                    (0..N_GOODS)
                        .map(|g| base + 0.01 * i as f64 + 0.1 * g as f64)
                        .collect::<Vec<_>>()
                }),
            )
        };
        let hours = |n: usize| -> Rows<f64> {
            Rows::from_rows(
                N_GOODS,
                (0..n).map(|i| {
                    (0..N_GOODS)
                        .map(|g| 730.0 * ((i + g) % 4) as f64)
                        .collect::<Vec<_>>()
                }),
            )
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
        for g in 0..7 {
            let nu = 1.0 + p.border_nu_of(g);
            let gk = k.good(g);
            for (i, l) in net.domestic.iter().enumerate() {
                let e = entry(&gk.domestic, l);
                assert_eq!(e.inv_tau, 1.0 / rt.domestic[i][g]);
                assert_eq!(e.k, libm::pow(rt.domestic[i][g], -p.theta[g]));
            }
            for (i, l) in net.access.iter().enumerate() {
                let e = entry(&gk.access, l);
                assert_eq!(e.inv_tau, 1.0 / rt.access[i][g]);
            }
            for (i, l) in net.crossings.iter().enumerate() {
                let e = crossing(&gk.crossings, l);
                let loss = rt.crossings[i][g] * nu;
                let m = net.gateway_mass[l.from as usize] * net.gateway_mass[l.to as usize];
                assert_eq!(e.inv_loss, 1.0 / loss);
                assert_eq!(e.k, m * libm::pow(loss * 1.0, -p.theta[g]));
                let lag = (rt.crossing_hours[i][g] / 730.0).round().min(3.0) as u8;
                assert_eq!(e.lag, lag);
            }
            // the least-cost kernel differs: the route taus were used
            assert_ne!(
                least.good(g).domestic.entries[0].inv_tau,
                gk.domestic.entries[0].inv_tau
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
            domestic: Rows::filled(2, N_GOODS, 0.9),
            domestic_hours: Rows::filled(2, N_GOODS, 1.0),
            ..LegTaus::default()
        });
        Kernel::build(
            &net,
            &W,
            &Wedges::none(1),
            &TradeParams::default().bulk_off(),
        );
    }

    #[test]
    fn home_kernel_is_one() {
        let p = TradeParams::default().bulk_off();
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
                let acc = origin_access(&k, g, &w);
                assert_eq!((acc.home[d], acc.abroad[d]), (2.5, 0.0));
            }
        }
    }

    #[test]
    fn phi_matches_dense_kernel() {
        let p = TradeParams::default().bulk_off();
        let mut rng = Rng(0x9e3779b97f4a7c15);
        let net = world(&mut rng);
        let mut wedges = Wedges::none(2);
        for t in &mut wedges.tariff {
            *t = (0.3 * rng.next()) as f32;
        }
        let kernel = Kernel::build(&net, &W, &wedges, &p);

        for g in [0usize, 3, 4, 6] {
            let nu = 1.0 + p.border_nu_of(g);
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
                                        let t = f64::from(wedges.rate(
                                            cb,
                                            ca,
                                            Goods::p2().tariff_group(g),
                                        ));
                                        let m = net.gateway_mass[a] * net.gateway_mass[b];
                                        s += leg_k(&p, g, oa, 1.0)
                                            * m
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

            // weights relative to each country's own scale, the scales far apart for some
            // goods: the kernel's log access is that of the dense access
            for shift in [[0.0, 0.0], [-250.0, 0.0], [0.0, -250.0]] {
                let mut acc = Access::default();
                kernel.origin_access_into(g, &pt, &shift, &mut acc);
                assert_eq!((acc.home.len(), acc.abroad.len()), (10, 10));
                assert_eq!(acc.v.len(), 3);
                assert_eq!(acc.u.len(), 3);
                // each country measures what it imports in its supplier's scale
                assert_eq!(acc.dest_shift, [shift[1], shift[0]]);
                let country = &net.province_country;
                for (d, &cd) in country.iter().enumerate() {
                    let want: f64 = (0..10)
                        .map(|o| dense[o][d] * pt[o] * f64::exp(shift[usize::from(country[o])]))
                        .sum();
                    let got = f64::exp(acc.ln_phi(d, usize::from(cd)));
                    let rel = (got - want).abs() / want;
                    assert!(rel < 1e-12, "g {g} d {d}: {got} vs {want}");
                }
            }
        }
    }

    #[test]
    fn kernel_round_trip_at_extreme_tau() {
        // tau 1e4 at theta 64.85: tau^-theta is about 4e-260, below f32 and far below what an
        // f32 ln K resolves to 1e-7 (1.5e-5)
        let p = TradeParams {
            theta: vec![64.85; N_GOODS],
            ..TradeParams::default().bulk_off()
        };
        let mut net = world(&mut Rng(3));
        let taus = |n: usize| Rows::filled(n, N_GOODS, 1e4);
        net.route_taus = Some(LegTaus {
            domestic: taus(net.domestic.len()),
            access: taus(net.access.len()),
            crossings: taus(net.crossings.len()),
            domestic_hours: taus(net.domestic.len()),
            access_hours: taus(net.access.len()),
            crossing_hours: taus(net.crossings.len()),
        });
        let k = Kernel::build(&net, &W, &Wedges::none(2), &p);
        let want = libm::pow(1e4, -64.85);
        assert!(want > 0.0 && want < 1e-259);
        for g in 0..7 {
            let gk = k.good(g);
            for e in gk.domestic.entries.iter().chain(&gk.access.entries) {
                assert!(libm::log(e.k).is_finite());
                assert!((e.k - want).abs() <= 1e-7 * want, "{} vs {want}", e.k);
                assert_eq!(e.inv_tau, 1e-4);
            }
            let want = libm::pow(1e4 * (1.0 + p.border_nu_of(g)), -64.85);
            for a in 0..3 {
                for e in gk.crossings.row(a) {
                    let m = net.gateway_mass[a] * net.gateway_mass[e.col as usize];
                    assert!(libm::log(e.k).is_finite());
                    assert!((e.k - m * want).abs() <= 1e-7 * m * want);
                }
            }
        }
    }

    #[test]
    fn closed_crossings_carry_nothing_across_far_scales() {
        // no route from country 0 into country 1, whose best origin is e^800 dearer: its access
        // stays in its own scale, and the closed crossings add nothing, not 0 times infinity
        let mut net = world(&mut Rng(9));
        let n = |legs: &Vec<Leg>| legs.len();
        let closed = Rows::from_rows(
            N_GOODS,
            net.crossings.iter().map(|l| {
                [if net.gateway_country[l.from as usize] == 0 {
                    f64::INFINITY
                } else {
                    1.5
                }; N_GOODS]
            }),
        );
        net.route_taus = Some(LegTaus {
            domestic: Rows::filled(n(&net.domestic), N_GOODS, 1.2),
            access: Rows::filled(n(&net.access), N_GOODS, 1.1),
            crossings: closed,
            domestic_hours: Rows::filled(n(&net.domestic), N_GOODS, 0.0),
            access_hours: Rows::filled(n(&net.access), N_GOODS, 0.0),
            crossing_hours: Rows::filled(n(&net.crossings), N_GOODS, 0.0),
        });
        let k = Kernel::build(
            &net,
            &W,
            &Wedges::none(2),
            &TradeParams::default().bulk_off(),
        );
        let mut acc = Access::default();
        k.origin_access_into(0, &[1.0; 10], &[0.0, -800.0], &mut acc);
        // country 0 imports from country 1 in country 1's scale; nothing reaches country 1
        assert_eq!(acc.dest_shift, [-800.0, 0.0]);
        assert!(acc.abroad[5..].iter().all(|&a| a == 0.0));
        for d in 0..10 {
            let ln_phi = acc.ln_phi(d, usize::from(net.province_country[d]));
            assert!(ln_phi.is_finite() && ln_phi >= if d < 5 { 0.0 } else { -800.0 });
        }
    }

    #[test]
    #[should_panic(expected = "gateway masses of country 0 sum to")]
    fn gateway_masses_must_sum_to_one_per_country() {
        let mut net = world(&mut Rng(2));
        assert_eq!(equal_gateway_mass(&net.gateway_country), [0.5, 0.5, 1.0]);
        net.gateway_mass = vec![0.5, 0.6, 1.0];
        Kernel::build(
            &net,
            &W,
            &Wedges::none(2),
            &TradeParams::default().bulk_off(),
        );
    }
}
