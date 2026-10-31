//! The price-gain scale of coupled blocks of commodity markets (ADR 0267; trade SPEC, Stability
//! region, Block gain): each country's traded cells, joined through the price step by substitution
//! between origins of a good, by the input orders of the country's enterprises and by their input
//! costs, are linearized together and their gains scaled so that the block keeps the margin m
//! (P03d) of its stability limit that ADR 0263 gives each market alone.

use rayon::prelude::*;

use ceteris_reference::io::Mat;
use ceteris_reference::model::{Factors, Model, Sourcing, good_major};
use ceteris_trade::TradeParams;

use crate::steady::{COVER, Steady};

/// Ticks of the linear map over which the spectral radius is measured; the growth rate is read
/// over the second half, after the transient of the start vector.
pub const ITERATIONS: usize = 4000;
/// A block counts as stable while its measured spectral radius is at most 1 + TOLERANCE: a
/// market capped by ADR 0263 sits at its own limit (radius 1) in the test map at gains / m, and
/// the measurement reads a radius to about 1e-3 at ITERATIONS.
pub const TOLERANCE: f64 = 2e-3;
/// Halvings of the scale interval [0, 1]: a scale to 2^-12.
pub const BISECTIONS: usize = 12;

/// An enterprise whose output enters a block: through its own market, its input orders or both.
#[derive(Clone, Debug, PartialEq)]
pub struct Enterprise {
    /// Block index of the market it sells in and its share of that market's output; `None` when
    /// its market is outside the block (a local good).
    pub own: Option<(usize, f64)>,
    /// Elasticity of its output to its own price at given wage and input prices.
    pub eps: f64,
}

/// The linearized price step of one block of markets (trade SPEC, Block gain). With x the log
/// price deviations of the block's markets, q the output deviations of its enterprises and a
/// slot s one traded input good at one buyer province:
///
/// ```text
/// y_s     = sum_(o -> s) a_os x_o,t-1                                buyer price, last tick
/// q_k     = (1 - eta) q_k,t-1 + eta ( eps_k x_own(k),t - sum_(s of k) c_ks y_s )
/// z_s     = sum_(k at s) v_ks ( q_k + H_s (q_k - q_k,t-1) )         input orders, order-up-to
/// e_i     = - sum_j E_ij x_j,t + sum_(i -> s) u_is z_s - sum_(k own i) w_k q_k
/// x_i,t+1 = x_i,t + kappa_i e_i
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// kappa_cell of each market (ADR 0263).
    pub gain: Vec<f64>,
    /// (i, j, E_ij): elasticity of the units ordered at market i to the price of j.
    pub demand: Vec<(usize, usize, f64)>,
    pub enterprises: Vec<Enterprise>,
    /// H_s of each slot: the months of use the order-up-to position holds, 1 + cover + lead.
    pub hold: Vec<f64>,
    /// (k, s, v_ks, c_ks): enterprise k buys at slot s; v its spending there over Phi of the
    /// slot's province, c the elasticity of its output to the slot's buyer price (eps_k times the
    /// input's cost share).
    pub inputs: Vec<(usize, usize, f64, f64)>,
    /// (s, i, a_is, u_is): market i supplies slot s; a its share of the slot's buyer price
    /// index, u the units it ships per unit of z over its orders.
    pub routes: Vec<(usize, usize, f64, f64)>,
}

/// Start vector of the measurement: fixed, and with no symmetry a block could keep it off an
/// unstable mode (splitmix64 of the state index, in [-1, 1)).
fn start(i: usize) -> f64 {
    let mut z = (i as u64)
        .wrapping_add(1)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

impl Block {
    /// Spectral radius of the linear map at gains `scale` kappa_i and employment speed `eta`
    /// (P41), as the growth rate of the map's iterates over the second half of ITERATIONS ticks.
    pub fn spectral_radius(&self, scale: f64, eta: f64) -> f64 {
        let (n, n_e, n_s) = (self.gain.len(), self.enterprises.len(), self.hold.len());
        let mut x: Vec<f64> = (0..n).map(start).collect();
        let mut xp: Vec<f64> = (0..n).map(|i| start(n + i)).collect();
        let mut qp: Vec<f64> = (0..n_e).map(|k| start(2 * n + k)).collect();
        let (mut q, mut e) = (vec![0.0; n_e], vec![0.0; n]);
        let (mut y, mut z) = (vec![0.0; n_s], vec![0.0; n_s]);
        let mut log_growth = 0.0;
        for t in 0..ITERATIONS {
            y.iter_mut().for_each(|v| *v = 0.0);
            for &(s, o, a, _) in &self.routes {
                y[s] += a * xp[o];
            }
            for (k, ent) in self.enterprises.iter().enumerate() {
                q[k] = ent.own.map_or(0.0, |(i, _)| ent.eps * x[i]);
            }
            for &(k, s, _, c) in &self.inputs {
                q[k] -= c * y[s];
            }
            for k in 0..n_e {
                q[k] = (1.0 - eta) * qp[k] + eta * q[k];
            }
            z.iter_mut().for_each(|v| *v = 0.0);
            for &(k, s, v, _) in &self.inputs {
                z[s] += v * (q[k] + self.hold[s] * (q[k] - qp[k]));
            }
            e.iter_mut().for_each(|v| *v = 0.0);
            for &(i, j, d) in &self.demand {
                e[i] -= d * x[j];
            }
            for &(s, i, _, u) in &self.routes {
                e[i] += u * z[s];
            }
            for (k, ent) in self.enterprises.iter().enumerate() {
                if let Some((i, w)) = ent.own {
                    e[i] -= w * q[k];
                }
            }
            xp.copy_from_slice(&x);
            for ((x, g), e) in x.iter_mut().zip(&self.gain).zip(&e) {
                *x += scale * g * e;
            }
            qp.copy_from_slice(&q);
            let norm = x
                .iter()
                .chain(&xp)
                .chain(&qp)
                .fold(0.0f64, |m, v| m.max(v.abs()));
            if norm == 0.0 {
                return 0.0;
            }
            if t >= ITERATIONS / 2 {
                log_growth += norm.ln();
            }
            for v in x.iter_mut().chain(xp.iter_mut()).chain(qp.iter_mut()) {
                *v /= norm;
            }
        }
        (log_growth / (ITERATIONS - ITERATIONS / 2) as f64).exp()
    }

    /// The block's gain scale b in (0, 1] at employment speed `eta` and margin `share` (P03d):
    /// the largest b on the bisection grid at which the map is stable at gains b kappa_i / m,
    /// 1 where it already is. A single market reduces to ADR 0263: at kappa_i / m its loop gain
    /// is at most the J2 limit 2. A block unstable at every scale of the grid keeps the
    /// smallest, 2^-BISECTIONS.
    pub fn scale(&self, eta: f64, share: f64) -> f64 {
        let stable = |b: f64| self.spectral_radius(b / share, eta) <= 1.0 + TOLERANCE;
        if stable(1.0) {
            return 1.0;
        }
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..BISECTIONS {
            let mid = 0.5 * (lo + hi);
            if stable(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if lo > 0.0 { lo } else { hi }
    }
}

/// Union-find root with path halving.
fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn join(parent: &mut [usize], a: usize, b: usize) {
    let (ra, rb) = (root(parent, a), root(parent, b));
    if ra != rb {
        parent[ra.max(rb)] = ra.min(rb);
    }
}

/// One country's linearization before it is split into connected blocks; indices are the
/// country's own (cells, slots, enterprises).
#[derive(Default)]
struct Country {
    cells: Vec<usize>,
    demand: Vec<(usize, usize, f64)>,
    enterprises: Vec<Enterprise>,
    hold: Vec<f64>,
    inputs: Vec<(usize, usize, f64, f64)>,
    routes: Vec<(usize, usize, f64, f64)>,
}

impl Country {
    /// The connected blocks: cells joined by cross-price demand, by an enterprise (its market
    /// and its input slots) and by a slot (the markets that supply it). Slots no market of the
    /// country supplies and enterprises that touch no market are left out.
    fn split(self, gain: impl Fn(usize) -> f64) -> Vec<(Vec<usize>, Block)> {
        let (n, n_s, n_e) = (self.cells.len(), self.hold.len(), self.enterprises.len());
        let (slot, ent) = (|s: usize| n + s, |k: usize| n + n_s + k);
        let mut parent: Vec<usize> = (0..n + n_s + n_e).collect();
        for &(i, j, _) in &self.demand {
            join(&mut parent, i, j);
        }
        for (k, e) in self.enterprises.iter().enumerate() {
            if let Some((i, _)) = e.own {
                join(&mut parent, ent(k), i);
            }
        }
        for &(k, s, _, _) in &self.inputs {
            join(&mut parent, ent(k), slot(s));
        }
        for &(s, i, _, _) in &self.routes {
            join(&mut parent, slot(s), i);
        }
        let roots: Vec<usize> = (0..parent.len()).map(|i| root(&mut parent, i)).collect();
        let mut sets: Vec<usize> = roots[..n].to_vec();
        sets.sort_unstable();
        sets.dedup();
        let local = |of: &dyn Fn(usize) -> usize, count: usize, r: usize| {
            let mut at = vec![usize::MAX; count];
            let mut next = 0;
            for (i, a) in at.iter_mut().enumerate() {
                if roots[of(i)] == r {
                    *a = next;
                    next += 1;
                }
            }
            at
        };
        sets.iter()
            .map(|&r| {
                let at = local(&|i| i, n, r);
                let s_at = local(&slot, n_s, r);
                let e_at = local(&ent, n_e, r);
                let cells: Vec<usize> = (0..n)
                    .filter(|&i| at[i] != usize::MAX)
                    .map(|i| self.cells[i])
                    .collect();
                let block = Block {
                    gain: cells.iter().map(|&j| gain(j)).collect(),
                    demand: self
                        .demand
                        .iter()
                        .filter(|d| at[d.0] != usize::MAX)
                        .map(|&(i, j, v)| (at[i], at[j], v))
                        .collect(),
                    enterprises: (0..n_e)
                        .filter(|&k| e_at[k] != usize::MAX)
                        .map(|k| {
                            let e = &self.enterprises[k];
                            Enterprise {
                                own: e.own.map(|(i, w)| (at[i], w)),
                                eps: e.eps,
                            }
                        })
                        .collect(),
                    hold: (0..n_s)
                        .filter(|&s| s_at[s] != usize::MAX)
                        .map(|s| self.hold[s])
                        .collect(),
                    inputs: self
                        .inputs
                        .iter()
                        .filter(|x| e_at[x.0] != usize::MAX)
                        .map(|&(k, s, v, c)| (e_at[k], s_at[s], v, c))
                        .collect(),
                    routes: self
                        .routes
                        .iter()
                        .filter(|x| s_at[x.0] != usize::MAX)
                        .map(|&(s, i, a, u)| (s_at[s], at[i], a, u))
                        .collect(),
                };
                (cells, block)
            })
            .collect()
    }
}

/// The blocks of a calibrated steady state at employment speed `eta` and the gains of
/// `trade`, each with its cells (good-major indices, G * P): a country's active traded cells
/// that the coupling joins, one block per connected set. `s_l` per enterprise.
pub fn blocks(
    m: &Model,
    src: &Sourcing,
    s: &Steady,
    s_l: &[f64],
    eta: f64,
    trade: &TradeParams,
) -> Vec<(Vec<usize>, Block)> {
    let inp = m.inp;
    let (n_p, n_g, n_n) = (inp.n_provinces(), inp.n_goods(), inp.n_gateways());
    let n_pg = n_p * n_g;
    let country = &inp.province_country;
    let traded_cell = |j: usize| inp.traded[j / n_p] && inp.active[j % n_p][j / n_p];
    let spend = good_major(&Mat::from_fn(n_p, n_g, |p, g| {
        if inp.traded[g] {
            s.spend_int[(p, g)] + s.spend_hh[(p, g)]
        } else {
            0.0
        }
    }));
    let reach: Vec<f64> = (0..n_pg)
        .map(|j| {
            if inp.traded[j / n_p] && src.phi[j] > 0.0 {
                spend[j] / src.phi[j]
            } else {
                0.0
            }
        })
        .collect();
    let rp: Vec<f64> = (0..n_pg)
        .map(|j| {
            if reach[j] > 0.0 {
                reach[j] / src.phi[j]
            } else {
                0.0
            }
        })
        .collect();
    let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
    let cro_net: Vec<f64> = cro
        .k
        .iter()
        .zip(&cro.inv_duty)
        .map(|(k, d)| k * d)
        .collect();
    let t = m.destination_sums(
        &reach,
        Factors {
            dom: &dom.k,
            acc: &acc.k,
            cro: &cro_net,
        },
    );
    // Squared routes beyond an origin's gateway (as in `elasticity::demand`): z2_a sums over
    // crossings a -> b and access legs b -> d of the squared factors times rp_d.
    let mut y2 = vec![0.0; n_g * n_n];
    for l in 0..acc.k.len() {
        let (a, d) = (
            acc.good[l] * n_n + acc.col[l],
            acc.good[l] * n_p + acc.row[l],
        );
        y2[a] += acc.k[l] * acc.k[l] * rp[d];
    }
    let mut z2 = vec![0.0; n_g * n_n];
    for l in 0..cro.k.len() {
        let g = cro.good[l];
        z2[g * n_n + cro.row[l]] += cro_net[l] * cro.k[l] * y2[g * n_n + cro.col[l]];
    }
    // Origins of the destination's country reaching it at home or over one domestic leg.
    let mut incoming: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_pg];
    for j in (0..n_pg).filter(|&j| traded_cell(j)) {
        incoming[j].push((j, 1.0));
    }
    for l in 0..dom.k.len() {
        let (from, to) = (
            dom.good[l] * n_p + dom.row[l],
            dom.good[l] * n_p + dom.col[l],
        );
        if traded_cell(from) && country[dom.row[l]] == country[dom.col[l]] {
            incoming[to].push((from, dom.k[l]));
        }
    }
    let mut via_gate: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n_g * n_n];
    for l in 0..acc.k.len() {
        let j = acc.good[l] * n_p + acc.row[l];
        if traded_cell(j) {
            via_gate[acc.good[l] * n_n + acc.col[l]].push((j, acc.k[l]));
        }
    }
    // T2_ij of pairs of one country's origins of a good, i != j, over shared last legs.
    let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
    let mut add_pairs = |list: &[(usize, f64)], weight: f64| {
        for &(i, ki) in list {
            for &(j, kj) in list {
                if i != j && country[i % n_p] == country[j % n_p] {
                    pairs.push((i, j, ki * kj * weight));
                }
            }
        }
    };
    for d in (0..n_pg).filter(|&d| rp[d] > 0.0) {
        add_pairs(&incoming[d], rp[d]);
    }
    for a in (0..n_g * n_n).filter(|&a| z2[a] > 0.0) {
        add_pairs(&via_gate[a], z2[a]);
    }
    pairs.sort_by_key(|&(i, j, _)| (i, j));

    // the storage order's factor on each stored good's order terms (ADR 0091; trade SPEC, Block
    // gain), by good
    let response: Vec<f64> = (0..n_g).map(|g| trade.order_response(g)).collect();
    let mut countries: Vec<Country> = (0..inp.n_countries()).map(|_| Country::default()).collect();
    let mut local = vec![usize::MAX; n_pg];
    for j in (0..n_pg).filter(|&j| traded_cell(j)) {
        let c = &mut countries[country[j % n_p]];
        local[j] = c.cells.len();
        c.cells.push(j);
        c.demand.push((
            local[j],
            local[j],
            s.demand_elasticity[(j % n_p, j / n_p)] * response[j / n_p],
        ));
    }
    let mut k = 0;
    while k < pairs.len() {
        let (i, j) = (pairs[k].0, pairs[k].1);
        let mut t2 = 0.0;
        while k < pairs.len() && (pairs[k].0, pairs[k].1) == (i, j) {
            t2 += pairs[k].2;
            k += 1;
        }
        if t[i] > 0.0 {
            let v = -(1.0 + inp.theta[i / n_p]) * src.w[j] * t2 / t[i] * response[i / n_p];
            countries[country[i % n_p]]
                .demand
                .push((local[i], local[j], v));
        }
    }
    // Slots: each country's (province, traded good) where its enterprises buy.
    let mut slot_of = vec![usize::MAX; n_pg];
    for (e, &s_le) in s_l.iter().enumerate() {
        let p = inp.ent_province[e];
        let c = &mut countries[country[p]];
        let eps = s_le * inp.sigma_fl / ((1.0 - s_le) * s.a_va[e]);
        let own_cell = inp.ent_good[e] * n_p + p;
        let own =
            (local[own_cell] != usize::MAX && s.supply[(p, inp.ent_good[e])] > 0.0).then(|| {
                (
                    local[own_cell],
                    s.output[e] / s.supply[(p, inp.ent_good[e])],
                )
            });
        let k = c.enterprises.len();
        c.enterprises.push(Enterprise { own, eps });
        for h in (0..n_g).filter(|&h| inp.traded[h] && s.ent_spend[(e, h)] > 0.0) {
            let d = h * n_p + p;
            if slot_of[d] == usize::MAX {
                slot_of[d] = c.hold.len();
                c.hold.push(1.0 + COVER + s.lead[(p, h)]);
                for &(i, kid) in &incoming[d] {
                    let u = if t[i] > 0.0 {
                        kid / t[i] * response[i / n_p]
                    } else {
                        0.0
                    };
                    c.routes
                        .push((slot_of[d], local[i], src.w[i] * kid / src.phi[d], u));
                }
            }
            let v = s.ent_spend[(e, h)] / src.phi[d];
            let cost = eps * s.ent_spend[(e, h)] / s.value[e];
            c.inputs
                .push((k, slot_of[d], v, if cost.is_finite() { cost } else { 0.0 }));
        }
    }
    let gain = |j: usize| {
        let (p, g) = (j % n_p, j / n_p);
        trade.price_gain(
            s.demand_elasticity[(p, g)] * response[g]
                + eta * s.supply_elasticity[(p, g)] / (2.0 - eta),
        )
    };
    countries.into_iter().flat_map(|c| c.split(gain)).collect()
}

/// The block scale of every cell (P, G) at employment speed `eta`: b of its block (Block::scale),
/// 1 for a cell in no block (local goods, inactive cells).
pub fn scales(
    m: &Model,
    src: &Sourcing,
    s: &Steady,
    s_l: &[f64],
    eta: f64,
    trade: &TradeParams,
) -> Mat {
    let n_p = m.inp.n_provinces();
    let blocks = blocks(m, src, s, s_l, eta, trade);
    let b: Vec<f64> = blocks
        .par_iter()
        .map(|(_, block)| block.scale(eta, trade.gain_share))
        .collect();
    let mut out = Mat::filled(n_p, m.inp.n_goods(), 1.0);
    for ((cells, _), &b) in blocks.iter().zip(&b) {
        for &j in cells {
            out[(j % n_p, j / n_p)] = b;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const KAPPA1: f64 = 0.05;
    const SHARE: f64 = 0.5;

    fn market(e: f64, eps: f64) -> Block {
        Block {
            gain: vec![KAPPA1],
            demand: vec![(0, 0, e)],
            enterprises: vec![Enterprise {
                own: Some((0, 1.0)),
                eps,
            }],
            hold: vec![],
            inputs: vec![],
            routes: vec![],
        }
    }

    /// Two markets, each with one enterprise: A buys B's good as an input (one slot, all its
    /// supply from B), which is most of B's orders, and B's price is half of A's cost.
    fn coupled() -> Block {
        let (eps_a, eps_b) = (12.0, 4.0);
        Block {
            gain: vec![KAPPA1, KAPPA1],
            demand: vec![(0, 0, 6.0), (1, 1, 12.0)],
            enterprises: vec![
                Enterprise {
                    own: Some((0, 1.0)),
                    eps: eps_a,
                },
                Enterprise {
                    own: Some((1, 1.0)),
                    eps: eps_b,
                },
            ],
            hold: vec![3.0],
            inputs: vec![(0, 0, 1.0, 0.5 * eps_a)],
            routes: vec![(0, 1, 1.0, 0.8)],
        }
    }

    fn alone(b: &Block, i: usize) -> Block {
        let e = b.demand.iter().find(|d| d.0 == i && d.1 == i).unwrap().2;
        market(e, b.enterprises[i].eps)
    }

    /// One market with one enterprise is the SPEC's single-market map (Stability region, kappa2
    /// 0): frictionless its root is 1 - kappa1 (E + eps), and at any eta its scale is 1 at the
    /// ADR 0263 gain, which puts it at the share m of its J2 limit.
    #[test]
    fn single_market_is_the_j2_map() {
        let b = market(10.0, 6.0);
        for s in [0.5, 1.0, 2.0, 2.4] {
            let want = (1.0 - s * KAPPA1 * 16.0).abs();
            let got = b.spectral_radius(s, 1.0);
            assert!((got - want).abs() < 1e-3, "scale {s}: {got} vs {want}");
        }
        for eta in [0.3, 1.0] {
            for (e, eps) in [(10.0, 6.0), (17.0, 30.0), (36.0, 25.0)] {
                let j = e + eta * eps / (2.0 - eta);
                let gain = KAPPA1 * (2.0 * SHARE / (KAPPA1 * j)).min(1.0);
                let b = Block {
                    gain: vec![gain],
                    ..market(e, eps)
                };
                assert_eq!(b.scale(eta, SHARE), 1.0, "eta {eta}, E {e}, eps {eps}");
                assert!(b.spectral_radius(1.0, eta) < 1.0);
            }
        }
    }

    /// Each market of the pair is stable alone and inside its margin, the pair is unstable at
    /// the same gains; its scale makes it stable with the margin m, and the scaled map decays.
    #[test]
    fn coupled_pair_is_scaled_into_its_margin() {
        let b = coupled();
        for eta in [0.3, 1.0] {
            for i in 0..2 {
                let a = alone(&b, i);
                assert_eq!(a.scale(eta, SHARE), 1.0, "market {i} alone, eta {eta}");
                assert!(a.spectral_radius(1.0 / SHARE, eta) < 1.0);
            }
        }
        assert!(
            b.spectral_radius(1.0, 1.0) > 1.01,
            "{}",
            b.spectral_radius(1.0, 1.0)
        );
        let s = b.scale(1.0, SHARE);
        assert!(s < 1.0, "scale {s}");
        assert!(b.spectral_radius(s / SHARE, 1.0) <= 1.0 + TOLERANCE);
        assert!(b.spectral_radius(s, 1.0) < 1.0);
        assert!(b.spectral_radius((s + 2.0 / 4096.0) / SHARE, 1.0) > 1.0 + TOLERANCE);
    }

    /// Without coupling a pair keeps its gains.
    #[test]
    fn uncoupled_pair_keeps_its_gains() {
        let mut b = coupled();
        b.hold.clear();
        b.inputs.clear();
        b.routes.clear();
        for eta in [0.3, 1.0] {
            assert_eq!(b.scale(eta, SHARE), 1.0);
        }
    }
}
