#![allow(clippy::needless_range_loop)]
//! RFC 0317 D4 option A, stage S3 spike: [`dispatch`](super::dispatch) for a block of `B` goods
//! that share a kernel class, with price, fill, the sourcing scratch and every output stored
//! `[row][B]` so each kernel row is read once and the inner loop runs over the block. Each
//! good's sums keep the order of its scalar call and skip what it skips, so every output is
//! bit-identical to the scalar call. Unlike [`super::dispatch_lanes`] (option B, separate
//! buffers per lane) the block owns interleaved buffers; `gather` and `scatter` move a block
//! in and out of per-good [`Sourced`] values, which stage S4 would replace by storing blocks
//! natively. Spike: no 128 byte alignment, not wired into the tick.

use super::{ARRIVAL_SLOTS, Inflow, Sourced, below_normal, over, times_two_to};
use crate::kernel::Kernel;

type Pair<const B: usize> = [[f64; 2]; B];

#[derive(Debug, Default)]
pub struct Block<const B: usize> {
    price: Vec<[f64; B]>,
    fill: Vec<Pair<B>>,
    weight: Vec<[f64; B]>,
    reach: Vec<Pair<B>>,
    reach_abroad: Vec<Pair<B>>,
    inv_q: Vec<[f64; B]>,
    /// `carry[country_a * n_countries + country_b][lane]`
    carry: Vec<[f64; B]>,
    up: Vec<i32>,
    y: Vec<[f64; B]>,
    inv_y: Vec<[f64; B]>,
    z: Vec<[f64; B]>,
    net_ratio: Vec<[f64; B]>,
    cross_w: Vec<[f64; B]>,
    gross_a: Vec<[f64; B]>,
    units_a: Vec<[f64; B]>,
    origin_units_a: Vec<[f64; B]>,
    at_b: Vec<[Inflow; B]>,
    shipped: Vec<[f64; B]>,
    shipped_rounding: Vec<[f64; B]>,
    arrive: [Vec<Pair<B>>; ARRIVAL_SLOTS],
    home: Vec<Pair<B>>,
    survived: [Vec<[f64; B]>; ARRIVAL_SLOTS],
    paid: Vec<[f64; B]>,
    paid_by_class: Vec<Pair<B>>,
    received: Vec<[f64; B]>,
    tariff: Vec<[f64; B]>,
    domestic_value: Vec<[f64; B]>,
    access_in_value: Vec<[f64; B]>,
    crossing_value: Vec<[f64; B]>,
    crossing_units: Vec<[f64; B]>,
    access_out_value: Vec<[f64; B]>,
    lost: Vec<f64>,
    selected: Vec<f64>,
}

fn zero<T: Copy>(v: &mut Vec<T>, n: usize, z: T) {
    v.clear();
    v.resize(n, z);
}

impl<const B: usize> Block<B> {
    /// Interleaves the inputs of `B` goods `goods` of one kernel class; each `outs[l]` must have been
    /// through `source` for `g`.
    pub fn gather(
        &mut self,
        k: &Kernel,
        goods: [usize; B],
        price: [&[f64]; B],
        fill: [&[[f64; 2]]; B],
        outs: &[Sourced; B],
    ) {
        let np = k.n_provinces();
        let nc = k.n_countries();
        for l in 0..B {
            assert_eq!(
                outs[l].scratch.good,
                Some(goods[l]),
                "dispatch must follow source"
            );
            assert!(price[l].len() == np && fill[l].len() == np);
        }
        zero(&mut self.price, np, [0.0; B]);
        zero(&mut self.fill, np, [[0.0; 2]; B]);
        zero(&mut self.weight, np, [0.0; B]);
        zero(&mut self.reach, np, [[0.0; 2]; B]);
        zero(&mut self.reach_abroad, np, [[0.0; 2]; B]);
        zero(&mut self.inv_q, np, [0.0; B]);
        zero(&mut self.carry, nc * nc, [0.0; B]);
        zero(&mut self.up, B, 0);
        for l in 0..B {
            let s = &outs[l].scratch;
            self.up[l] = s.up;
            for o in 0..np {
                self.price[o][l] = price[l][o];
                self.fill[o][l] = fill[l][o];
                self.weight[o][l] = s.weight[o];
                self.reach[o][l] = s.reach[o];
                self.reach_abroad[o][l] = s.reach_abroad[o];
                self.inv_q[o][l] = s.inv_q[o];
            }
            for (i, &c) in s.access.carry.iter().enumerate() {
                self.carry[i][l] = c;
            }
        }
    }

    /// Writes the outputs of the last [`Self::run`] into `outs`, as `B` scalar calls leave them.
    pub fn scatter(&self, outs: &mut [Sourced; B]) {
        for (l, out) in outs.iter_mut().enumerate() {
            let col = |v: &Vec<[f64; B]>, to: &mut Vec<f64>| {
                to.clear();
                to.extend(v.iter().map(|x| x[l]));
            };
            let col2 = |v: &Vec<Pair<B>>, to: &mut Vec<[f64; 2]>| {
                to.clear();
                to.extend(v.iter().map(|x| x[l]));
            };
            col(&self.shipped, &mut out.shipped);
            col(&self.shipped_rounding, &mut out.shipped_rounding);
            for s in 0..ARRIVAL_SLOTS {
                col2(&self.arrive[s], &mut out.arrive[s]);
                col(&self.survived[s], &mut out.survived[s]);
            }
            col2(&self.home, &mut out.home);
            col(&self.paid, &mut out.paid);
            col2(&self.paid_by_class, &mut out.paid_by_class);
            col(&self.received, &mut out.received);
            col(&self.tariff, &mut out.tariff);
            col(&self.domestic_value, &mut out.domestic_value);
            col(&self.access_in_value, &mut out.access_in_value);
            col(&self.crossing_value, &mut out.crossing_value);
            col(&self.crossing_units, &mut out.crossing_units);
            col(&self.access_out_value, &mut out.access_out_value);
            out.lost = self.lost[l];
            out.selected = self.selected[l];
        }
    }

    /// The block kernel: one pass over the rows of `k.good(g)` for all `B` goods.
    pub fn run(&mut self, k: &Kernel, g: usize) {
        let (np, ng) = (k.n_provinces(), k.n_gateways());
        let gk = k.good(g);
        let (gc, nc) = (k.gateway_country(), k.n_countries());
        zero(&mut self.shipped, np, [0.0; B]);
        zero(&mut self.shipped_rounding, np, [0.0; B]);
        for a in &mut self.arrive {
            zero(a, np, [[0.0; 2]; B]);
        }
        zero(&mut self.home, np, [[0.0; 2]; B]);
        for v in &mut self.survived {
            zero(v, np, [0.0; B]);
        }
        zero(&mut self.paid, np, [0.0; B]);
        zero(&mut self.paid_by_class, np, [[0.0; 2]; B]);
        zero(&mut self.received, np, [0.0; B]);
        zero(&mut self.tariff, np, [0.0; B]);
        zero(
            &mut self.domestic_value,
            gk.domestic.entries.len(),
            [0.0; B],
        );
        zero(&mut self.access_in_value, gk.access.entries.len(), [0.0; B]);
        zero(
            &mut self.crossing_value,
            gk.crossings.entries.len(),
            [0.0; B],
        );
        zero(
            &mut self.crossing_units,
            gk.crossings.entries.len(),
            [0.0; B],
        );
        zero(
            &mut self.access_out_value,
            gk.access.entries.len(),
            [0.0; B],
        );
        zero(&mut self.lost, B, 0.0);
        zero(&mut self.selected, B, 0.0);

        let Self {
            price,
            fill,
            weight,
            reach,
            reach_abroad,
            inv_q,
            carry,
            up,
            y,
            inv_y,
            z,
            net_ratio,
            cross_w,
            gross_a,
            units_a,
            origin_units_a,
            at_b,
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
        } = self;

        for c in 0..2 {
            let live: [bool; B] = std::array::from_fn(|l| {
                (0..np).any(|d| reach[d][l][c] != 0.0 || reach_abroad[d][l][c] != 0.0)
            });
            if !live.contains(&true) {
                continue;
            }
            zero(y, ng, [0.0; B]);
            for d in 0..np {
                if !reach_abroad[d].iter().any(|r| r[c] > 0.0) {
                    continue;
                }
                for e in gk.access.row(d) {
                    let row = &mut y[e.col as usize];
                    for l in 0..B {
                        if reach_abroad[d][l][c] > 0.0 {
                            row[l] += e.k * reach_abroad[d][l][c];
                        }
                    }
                }
            }
            inv_y.clear();
            inv_y.extend(y.iter().map(|y| y.map(|y| 1.0 / y)));
            zero(z, ng, [0.0; B]);
            zero(net_ratio, ng, [0.0; B]);
            zero(cross_w, gk.crossings.entries.len(), [0.0; B]);
            for a in 0..ng {
                let lo = gk.crossings.offsets[a] as usize;
                let (mut gross, mut net) = ([0.0; B], [0.0; B]);
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    let b = e.col as usize;
                    let cr = usize::from(gc[a]) * nc + usize::from(gc[b]);
                    for l in 0..B {
                        let x = e.k * carry[cr][l] * y[b][l];
                        cross_w[lo + i][l] = x;
                        gross[l] += x;
                        net[l] += x * e.inv_duty;
                    }
                }
                for l in 0..B {
                    z[a][l] = gross[l];
                    if gross[l] > 0.0 {
                        net_ratio[a][l] = net[l] / gross[l];
                    }
                }
            }

            zero(gross_a, ng, [0.0; B]);
            zero(units_a, ng, [0.0; B]);
            zero(origin_units_a, ng, [0.0; B]);
            for o in 0..np {
                let on: [bool; B] =
                    std::array::from_fn(|l| live[l] && fill[o][l][c] != 0.0 && weight[o][l] != 0.0);
                if !on.contains(&true) {
                    continue;
                }
                let units_of = |inv_p: f64, value: f64, err: f64| {
                    (err + below_normal(value)) * inv_p + below_normal(value * inv_p)
                };
                let (mut m, mut inv_p, mut dm) = ([0.0; B], [0.0; B], [0.0; B]);
                for l in 0..B {
                    if on[l] {
                        let (f, wo) = (fill[o][l][c], weight[o][l]);
                        m[l] = times_two_to(f * wo, up[l]);
                        inv_p[l] = 1.0 / price[o][l];
                        dm[l] = times_two_to(below_normal(f * wo), up[l]) + below_normal(m[l]);
                    }
                }
                for l in 0..B {
                    if !on[l] {
                        continue;
                    }
                    let x = m[l] * reach[o][l][c];
                    shipped_rounding[o][l] += units_of(inv_p[l], x, dm[l] * reach[o][l][c]);
                    if x > 0.0 {
                        let units = x * inv_p[l];
                        let got = x * inv_q[o][l];
                        arrive[0][o][l][c] += got;
                        home[o][l][c] += units;
                        survived[0][o][l] += units;
                        selected[l] += got - units;
                        shipped[o][l] += units;
                        paid[o][l] += x;
                        paid_by_class[o][l][c] += x;
                        received[o][l] += x;
                    }
                }
                let lo = gk.domestic.offsets[o] as usize;
                for (i, e) in gk.domestic.row(o).iter().enumerate() {
                    let d = e.col as usize;
                    for l in 0..B {
                        if !on[l] {
                            continue;
                        }
                        let x = m[l] * e.k * reach[d][l][c];
                        let mk = (dm[l] * e.k + below_normal(m[l] * e.k)) * reach[d][l][c];
                        shipped_rounding[o][l] += units_of(inv_p[l], x, mk);
                        if x == 0.0 {
                            continue;
                        }
                        let units = x * inv_p[l];
                        let melted = units * e.inv_tau;
                        let got = x * inv_q[d][l];
                        arrive[1][d][l][c] += got;
                        survived[1][d][l] += melted;
                        lost[l] += units - melted;
                        selected[l] += got - melted;
                        shipped[o][l] += units;
                        domestic_value[lo + i][l] += x;
                        paid[d][l] += x;
                        paid_by_class[d][l][c] += x;
                        received[o][l] += x;
                    }
                }
                let lo = gk.access.offsets[o] as usize;
                for (i, e) in gk.access.row(o).iter().enumerate() {
                    let a = e.col as usize;
                    for l in 0..B {
                        if !on[l] {
                            continue;
                        }
                        let x = m[l] * e.k * z[a][l];
                        let mk = ((dm[l] * e.k + below_normal(m[l] * e.k)) * z[a][l]
                            + below_normal(x))
                            * net_ratio[a][l];
                        shipped_rounding[o][l] += units_of(inv_p[l], x * net_ratio[a][l], mk);
                        if x == 0.0 {
                            continue;
                        }
                        let net = x * net_ratio[a][l];
                        let units = net * inv_p[l];
                        access_out_value[lo + i][l] += net;
                        received[o][l] += net;
                        shipped[o][l] += units;
                        lost[l] += units - units * e.inv_tau;
                        let origin_units = x * inv_p[l];
                        gross_a[a][l] += x;
                        units_a[a][l] += origin_units * e.inv_tau;
                        origin_units_a[a][l] += origin_units;
                    }
                }
            }

            at_b.clear();
            at_b.resize(ng, [Inflow::default(); B]);
            for a in 0..ng {
                let on: [bool; B] = std::array::from_fn(|l| gross_a[a][l] > 0.0);
                if !on.contains(&true) {
                    continue;
                }
                let lo = gk.crossings.offsets[a] as usize;
                let inv_z: [f64; B] = std::array::from_fn(|l| 1.0 / z[a][l]);
                for (i, e) in gk.crossings.row(a).iter().enumerate() {
                    let b = e.col as usize;
                    for l in 0..B {
                        if !on[l] {
                            continue;
                        }
                        let weight = cross_w[lo + i][l];
                        if weight == 0.0 {
                            continue;
                        }
                        let share = over(weight, inv_z[l], z[a][l]);
                        let gross = gross_a[a][l] * share;
                        let net = gross * e.inv_duty;
                        let units = units_a[a][l] * share * e.inv_duty;
                        let got = units * e.inv_loss;
                        crossing_value[lo + i][l] += net;
                        crossing_units[lo + i][l] += origin_units_a[a][l] * share * e.inv_duty;
                        lost[l] += units - got;
                        let at = &mut at_b[b][l];
                        at.units[usize::from(e.lag)] += got;
                        at.value[usize::from(e.lag)] += gross;
                        at.gross += gross;
                        at.net += net;
                    }
                }
            }

            for d in 0..np {
                let on: [bool; B] = std::array::from_fn(|l| reach_abroad[d][l][c] > 0.0);
                if !on.contains(&true) {
                    continue;
                }
                let lo = gk.access.offsets[d] as usize;
                for (i, e) in gk.access.row(d).iter().enumerate() {
                    let b = e.col as usize;
                    for l in 0..B {
                        if !on[l] {
                            continue;
                        }
                        let weight = e.k * reach_abroad[d][l][c];
                        if weight == 0.0 {
                            continue;
                        }
                        let share = over(weight, inv_y[b][l], y[b][l]);
                        let at = &at_b[b][l];
                        for (lag, (&units, &value)) in at.units.iter().zip(&at.value).enumerate() {
                            let units = units * share;
                            let melted = units * e.inv_tau;
                            let got = value * share * inv_q[d][l];
                            arrive[1 + lag][d][l][c] += got;
                            survived[1 + lag][d][l] += melted;
                            lost[l] += units - melted;
                            selected[l] += got - melted;
                        }
                        let gross = at.gross * share;
                        let net = at.net * share;
                        access_in_value[lo + i][l] += net;
                        paid[d][l] += gross;
                        paid_by_class[d][l][c] += gross;
                        tariff[d][l] += gross - net;
                    }
                }
            }
        }
    }
}

/// `B` goods of kernel class `g` in one block pass: gather, [`Block::run`], scatter.
pub fn dispatch_block<const B: usize>(
    k: &Kernel,
    g: usize,
    price: [&[f64]; B],
    fill: [&[[f64; 2]]; B],
    outs: &mut [Sourced; B],
) {
    let mut block = Block::<B>::default();
    block.gather(k, [g; B], price, fill, outs);
    block.run(k, g);
    block.scatter(outs);
}
