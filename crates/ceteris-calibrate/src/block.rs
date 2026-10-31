//! The price-gain cap of coupled blocks of markets (ADR 0267; trade SPEC, Stability region, Block
//! gain): each country's traded and local cells, joined through the tick by substitution between
//! origins of a good, by its enterprises' input orders, input costs and wages, and by its
//! households' income and demand, are linearized together; a block beyond the margin m (P03d) of
//! its stability limit that ADR 0263 gives each market alone gets its limit gain stored, and runs
//! cap each of its cells at the share m of it.

use rayon::prelude::*;

use ceteris_econ::EconParams;
use ceteris_reference::demand;
use ceteris_reference::io::Mat;
use ceteris_reference::model::{Factors, Model, Sourcing, good_major};
use ceteris_trade::TradeParams;

use crate::steady::{COVER, Steady};

/// Ticks of the linear map over which the spectral radius is measured; the growth rate is read
/// over the second half, after the transient of the start vector.
pub const ITERATIONS: usize = 4000;
/// A block counts as stable while its measured spectral radius is at most 1 + TOLERANCE (P157):
/// a market capped by ADR 0263 sits at its own limit (radius 1) in the test map at gains / m, and
/// the measurement reads a radius to about 1e-3 at ITERATIONS.
pub const TOLERANCE: f64 = 2e-3;
/// Halvings of the scale interval [0, 1]: a scale to 2^-12.
pub const BISECTIONS: usize = 12;
/// Relative step of the central differences of household demand.
const STEP: f64 = 1e-5;

/// How a block's wages move (econ SPEC, Wages).
#[derive(Clone, Debug, PartialEq)]
pub enum Wage {
    /// Held at the steady state: the map at given wages, as J2 is.
    Given,
    /// Frictionless: each province's wage clears its labour market within the tick.
    Clearing,
    /// The wage rule dlnw = kappa_w x (P39) on last tick's wage, at the gain of a rise and of a
    /// cut, (1 - delta_w) kappa_w (P40): the block is stable when the map is under both.
    Rule { rise: f64, cut: f64 },
}

/// An enterprise of the block's country.
#[derive(Clone, Debug, PartialEq)]
pub struct Enterprise {
    /// Block index of the cell it sells in and its share of that cell's output; `None` when it
    /// sells in no cell of the block.
    pub own: Option<(usize, f64)>,
    /// Elasticity of its output to its own price at given wage and input prices.
    pub eps: f64,
    /// Block index of its province (labour market).
    pub province: usize,
    /// beta = eps a_VA, the elasticity of its output to the wage.
    pub wage: f64,
    /// L0 / (L^s s_L): its weight in its province's excess labour demand per unit of output.
    pub labour: f64,
}

/// An input of an enterprise at a place: a traded slot (orders through the order-up-to position)
/// or a local cell (drawn from this tick's output).
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    pub enterprise: usize,
    /// Place index: a cell below the block's cell count, else a slot.
    pub place: usize,
    /// At a slot spend / Phi_d, so that sum_s u_is v is the share of i's orders it places; at a
    /// local cell its draw over the cell's output.
    pub v: f64,
    /// c: eps times the input's cost share, the elasticity of its output to the place's price.
    pub cost: f64,
    /// Its spending there over the country's profit pool.
    pub spend: f64,
}

/// A route of a cell's good to a slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub slot: usize,
    pub cell: usize,
    /// The cell's share of the slot's buyer price index.
    pub a: f64,
    /// Units the cell ships per unit of the slot's z over its orders.
    pub u: f64,
    /// d L_s / d x_cell, the move of the slot's expected months in transit with the cell's price:
    /// -theta a (lag - L_s), lag 0 at home and 1 over a domestic leg.
    pub lead: f64,
}

/// A province's labour market and households.
#[derive(Clone, Debug, PartialEq)]
pub struct Province {
    /// sum L0 / L^s = 1 - u*: employment over labour supply.
    pub employed: f64,
    /// Its wage bill over the country's profit pool.
    pub bill: f64,
}

/// Households' demand of one province at one place (a slot or a local cell), each term over the
/// slot's Phi_d (z units) or the cell's output (e units).
#[derive(Clone, Debug, PartialEq)]
pub struct Household {
    pub province: usize,
    pub place: usize,
    /// Spending change per relative change of the province's wage bill.
    pub wage: f64,
    /// Spending change per relative change of the country's profit pool, paid last tick.
    pub pool: f64,
    /// Spending: the units term, units = spending / price.
    pub spend: f64,
    /// (place, spending change per log price of the place at this tick's E3).
    pub prices: Vec<(usize, f64)>,
}

/// The linearized tick of one country's cells (trade SPEC, Block gain). With x the log price
/// deviations of the cells, q the output deviations of the enterprises, omega the provinces'
/// log wage deviations, W the wage bills', P the profit pool's, y the buyer prices of the slots
/// and a place's price that of its cell or slot:
///
/// ```text
/// y_s,t   = sum_(o -> s) a_os x_o,t
/// q*_k    = eps_k x_own(k),t - sum_(places of k) c_kr price_r,t-1
/// omega_t = sum_k L_k q*_k / sum_k L_k beta_k            (Clearing; omega_(t-1) under the rule)
/// omega_t = omega_t-1 + kappa_w sum_k L_k (q*_k - beta_k omega_t-1)               (Rule)
/// q_k     = (1 - eta) q_k,t-1 + eta (q*_k - beta_k omega)
/// W_p     = omega_p,t + sum_(k at p) L_k q_k / employed_p
/// L_s,t   = sum_(o -> s) l_os x_o,t                              expected months in transit
/// z_s     = sum_(k at s) v_ks (q_k + H_s (q_k - q_k,t-1) + L_s,t - L_s,t-1) + households
/// e_i     = - sum_j E_ij x_j + sum_s u_is z_s + draws + households - sum_(k own i) w_k q_k
/// P_t     = sum_i R_i (x_i + q_i + e_i) - sum_p B_p W_p - sum_inputs spend (price + units)
/// x_i,t+1 = x_i,t + kappa_i e_i
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// kappa_cell of each cell (ADR 0263).
    pub gain: Vec<f64>,
    /// (i, j, E_ij): elasticity of the units ordered at traded cell i to the price of j at given
    /// units of each buyer.
    pub demand: Vec<(usize, usize, f64)>,
    pub enterprises: Vec<Enterprise>,
    /// H_s of each slot: the months of use the order-up-to position holds, 1 + cover + lead.
    pub hold: Vec<f64>,
    pub inputs: Vec<Input>,
    pub routes: Vec<Route>,
    pub provinces: Vec<Province>,
    pub households: Vec<Household>,
    /// R_i of each cell, its sales over the country's profit pool.
    pub revenue: Vec<f64>,
    pub wage: Wage,
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

/// The work vectors of one step of the map.
struct Work {
    now: Vec<f64>,
    then: Vec<f64>,
    target: Vec<f64>,
    q: Vec<f64>,
    num: Vec<f64>,
    den: Vec<f64>,
    set_at: Vec<f64>,
    omega: Vec<f64>,
    bill: Vec<f64>,
    z: Vec<f64>,
    lead: Vec<f64>,
    e: Vec<f64>,
    made: Vec<f64>,
}

impl Block {
    fn cells(&self) -> usize {
        self.gain.len()
    }

    /// Length of the state: x, x_t-1, q_t-1, omega_t-1 and P_t-1.
    pub fn dimension(&self) -> usize {
        2 * self.cells() + self.enterprises.len() + self.provinces.len() + 1
    }

    /// The gains of the wage rule the map is measured at: one, or a rise and a cut.
    fn wage_gains(&self) -> Vec<f64> {
        match self.wage {
            Wage::Given | Wage::Clearing => vec![0.0],
            Wage::Rule { rise, cut } => vec![rise, cut],
        }
    }

    fn work(&self) -> Work {
        let (n, n_e, n_p) = (self.cells(), self.enterprises.len(), self.provinces.len());
        let places = n + self.hold.len();
        Work {
            now: vec![0.0; places],
            then: vec![0.0; places],
            target: vec![0.0; n_e],
            q: vec![0.0; n_e],
            num: vec![0.0; n_p],
            den: vec![0.0; n_p],
            set_at: vec![0.0; n_p],
            omega: vec![0.0; n_p],
            bill: vec![0.0; n_p],
            z: vec![0.0; self.hold.len()],
            lead: vec![0.0; self.hold.len()],
            e: vec![0.0; n],
            made: vec![0.0; n],
        }
    }

    /// One tick of the map at gains `scale` kappa_i, employment speed `eta` and wage gain
    /// `wage_gain`, from `state` to `next` (layout of [`Block::dimension`]).
    fn step(
        &self,
        scale: f64,
        eta: f64,
        wage_gain: f64,
        state: &[f64],
        next: &mut [f64],
        w: &mut Work,
    ) {
        let (n, n_e, n_p) = (self.cells(), self.enterprises.len(), self.provinces.len());
        let (x, rest) = state.split_at(n);
        let (xp, rest) = rest.split_at(n);
        let (qp, rest) = rest.split_at(n_e);
        let (wp, pool) = (&rest[..n_p], rest[n_p]);
        w.now[..n].copy_from_slice(x);
        w.then[..n].copy_from_slice(xp);
        w.now[n..].iter_mut().for_each(|v| *v = 0.0);
        w.then[n..].iter_mut().for_each(|v| *v = 0.0);
        w.lead.iter_mut().for_each(|v| *v = 0.0);
        for r in &self.routes {
            w.now[n + r.slot] += r.a * x[r.cell];
            w.then[n + r.slot] += r.a * xp[r.cell];
            w.lead[r.slot] += r.lead * (x[r.cell] - xp[r.cell]);
        }
        for (k, ent) in self.enterprises.iter().enumerate() {
            w.target[k] = ent.own.map_or(0.0, |(i, _)| ent.eps * x[i]);
        }
        for input in &self.inputs {
            w.target[input.enterprise] -= input.cost * w.then[input.place];
        }
        // w.set_at: the wage the targets are set at; w.omega: the wage paid this tick
        w.num.iter_mut().for_each(|v| *v = 0.0);
        w.den.iter_mut().for_each(|v| *v = 0.0);
        for (k, ent) in self.enterprises.iter().enumerate() {
            w.num[ent.province] += ent.labour * w.target[k];
            w.den[ent.province] += ent.labour * ent.wage;
        }
        for (p, &last) in wp.iter().enumerate() {
            let (num, den) = (w.num[p], w.den[p]);
            (w.set_at[p], w.omega[p]) = match self.wage {
                Wage::Given => (0.0, 0.0),
                Wage::Clearing if den > 0.0 => (num / den, num / den),
                Wage::Clearing => (0.0, 0.0),
                Wage::Rule { .. } => (last, last + wage_gain * (num - den * last)),
            };
            w.bill[p] = w.omega[p];
        }
        for (k, ent) in self.enterprises.iter().enumerate() {
            let p = ent.province;
            w.q[k] = (1.0 - eta) * qp[k] + eta * (w.target[k] - ent.wage * w.set_at[p]);
            let employed = self.provinces[p].employed;
            if employed > 0.0 {
                w.bill[p] += ent.labour * w.q[k] / employed;
            }
        }
        w.z.iter_mut().for_each(|v| *v = 0.0);
        w.e.iter_mut().for_each(|v| *v = 0.0);
        let mut new_pool = 0.0;
        for input in &self.inputs {
            let k = input.enterprise;
            let units = if input.place < n {
                w.e[input.place] += input.v * w.q[k];
                w.q[k]
            } else {
                let s = input.place - n;
                let units = w.q[k] + self.hold[s] * (w.q[k] - qp[k]) + w.lead[s];
                w.z[s] += input.v * units;
                units
            };
            new_pool -= input.spend * (w.now[input.place] + units);
        }
        for h in &self.households {
            let mut d = h.wage * w.bill[h.province] + h.pool * pool - h.spend * w.now[h.place];
            for &(r, c) in &h.prices {
                d += c * w.now[r];
            }
            if h.place < n {
                w.e[h.place] += d;
            } else {
                w.z[h.place - n] += d;
            }
        }
        for &(i, j, d) in &self.demand {
            w.e[i] -= d * x[j];
        }
        for r in &self.routes {
            w.e[r.cell] += r.u * w.z[r.slot];
        }
        w.made.iter_mut().for_each(|v| *v = 0.0);
        for (k, ent) in self.enterprises.iter().enumerate() {
            if let Some((i, share)) = ent.own {
                w.made[i] += share * w.q[k];
            }
        }
        for (((e, &made), &r), &x) in w.e.iter_mut().zip(&w.made).zip(&self.revenue).zip(x) {
            *e -= made;
            new_pool += r * (x + made + *e);
        }
        for (p, prov) in self.provinces.iter().enumerate() {
            new_pool -= prov.bill * w.bill[p];
        }
        let (nx, rest) = next.split_at_mut(n);
        let (nxp, rest) = rest.split_at_mut(n);
        let (nqp, rest) = rest.split_at_mut(n_e);
        for (((nx, &x), &g), &e) in nx.iter_mut().zip(x).zip(&self.gain).zip(&w.e) {
            *nx = x + scale * g * e;
        }
        nxp.copy_from_slice(x);
        nqp.copy_from_slice(&w.q);
        rest[..n_p].copy_from_slice(&w.omega);
        rest[n_p] = new_pool;
    }

    /// One tick of the map from `state` (for tests of the measurement).
    pub fn apply(&self, scale: f64, eta: f64, wage_gain: f64, state: &[f64]) -> Vec<f64> {
        let mut next = vec![0.0; self.dimension()];
        self.step(scale, eta, wage_gain, state, &mut next, &mut self.work());
        next
    }

    /// Spectral radius at gains `scale` kappa_i and wage gain `wage_gain`, as the growth rate of
    /// the map's iterates over the second half of ITERATIONS ticks.
    pub fn radius_at(&self, scale: f64, eta: f64, wage_gain: f64) -> f64 {
        let dim = self.dimension();
        let mut state: Vec<f64> = (0..dim).map(start).collect();
        let mut next = vec![0.0; dim];
        let mut work = self.work();
        let mut log_growth = 0.0;
        for t in 0..ITERATIONS {
            self.step(scale, eta, wage_gain, &state, &mut next, &mut work);
            std::mem::swap(&mut state, &mut next);
            let norm = state.iter().fold(0.0f64, |m, v| m.max(v.abs()));
            if norm == 0.0 {
                return 0.0;
            }
            if t >= ITERATIONS / 2 {
                log_growth += norm.ln();
            }
            state.iter_mut().for_each(|v| *v /= norm);
        }
        (log_growth / (ITERATIONS - ITERATIONS / 2) as f64).exp()
    }

    /// Spectral radius at gains `scale` kappa_i and employment speed `eta` (P41): the largest
    /// over the wage gains of the block's regime.
    pub fn spectral_radius(&self, scale: f64, eta: f64) -> f64 {
        self.wage_gains()
            .into_iter()
            .map(|g| self.radius_at(scale, eta, g))
            .fold(0.0, f64::max)
    }

    /// The block's gain scale b in (0, 1] at employment speed `eta` and margin `share` (P03d):
    /// 1 where the map is stable at gains kappa_i / m, else the bisection's lower end on a grid
    /// of 2^-BISECTIONS, a scale at which it is stable at gains b kappa_i / m and unstable one
    /// grid step above. A block unstable at every scale of the grid keeps the smallest,
    /// 2^-BISECTIONS.
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

/// The derivatives of each pop's spending at the calibrated point (`demand::shares` by central
/// differences): (K, G) spending S, (K, G) d ln S / d ln money, and (K, G * G) d ln S_g / d ln q_h
/// at `[g * G + h]`.
fn pop_derivatives(m: &Model, s: &Steady) -> (Mat, Mat, Mat) {
    let inp = m.inp;
    let n_g = inp.n_goods();
    let kc = inp.pop_country();
    let sigma_c = inp.within_sigma();
    let sigma: Vec<f64> = kc.iter().map(|&c| sigma_c[c]).collect();
    let price = s.consumer.select_rows(&inp.pop_province);
    let per_person: Vec<f64> = s
        .pop_income
        .iter()
        .zip(&inp.pop_population)
        .map(|(&i, &n)| if n > 0.0 { i / n } else { 0.0 })
        .collect();
    let shares = |price: &Mat, income: &[f64]| {
        demand::shares(
            &s.weights,
            &kc,
            price,
            income,
            &sigma,
            &inp.category,
            inp.sigma_demand,
            &inp.pigl,
        )
    };
    let log_diff = |up: &Mat, down: &Mat, k: usize, g: usize| {
        let (u, d) = (up[(k, g)], down[(k, g)]);
        if u > 0.0 && d > 0.0 {
            (u / d).ln() / (2.0 * STEP)
        } else {
            0.0
        }
    };
    let n_k = per_person.len();
    let scaled = |f: f64| per_person.iter().map(|x| x * f).collect::<Vec<f64>>();
    let (up, down) = (
        shares(&price, &scaled(STEP.exp())),
        shares(&price, &scaled((-STEP).exp())),
    );
    let money = Mat::from_fn(n_k, n_g, |k, g| 1.0 + log_diff(&up, &down, k, g));
    let mut cross = Mat::zeros(n_k, n_g * n_g);
    for h in 0..n_g {
        let at = |f: f64| {
            let mut p = price.clone();
            for k in 0..n_k {
                p[(k, h)] *= f;
            }
            shares(&p, &per_person)
        };
        let (up, down) = (at(STEP.exp()), at((-STEP).exp()));
        for k in 0..n_k {
            for g in 0..n_g {
                cross[(k, g * n_g + h)] = log_diff(&up, &down, k, g);
            }
        }
    }
    let spend = Mat::from_fn(n_k, n_g, |k, g| s.pop_spend[(k, g)]);
    (spend, money, cross)
}

/// One country's linearization; indices are the country's own.
#[derive(Default)]
struct Country {
    cells: Vec<usize>,
    demand: Vec<(usize, usize, f64)>,
    enterprises: Vec<Enterprise>,
    hold: Vec<f64>,
    inputs: Vec<Input>,
    routes: Vec<Route>,
    provinces: Vec<Province>,
    households: Vec<Household>,
    revenue: Vec<f64>,
}

/// The blocks of a calibrated steady state under the employment speed and wage regime of
/// `params` and the gains of `trade`, each with its cells (good-major indices, G * P): one per
/// country, its active traded cells and its local cells with output, which the profit pool and
/// the households' income join into one block. `s_l` per enterprise.
pub fn blocks(
    m: &Model,
    src: &Sourcing,
    s: &Steady,
    s_l: &[f64],
    params: &EconParams,
    trade: &TradeParams,
) -> Vec<(Vec<usize>, Block)> {
    let inp = m.inp;
    let (n_p, n_g, n_n) = (inp.n_provinces(), inp.n_goods(), inp.n_gateways());
    let n_pg = n_p * n_g;
    let eta = params.eta;
    let country = &inp.province_country;
    let traded_cell = |j: usize| inp.traded[j / n_p] && inp.active[j % n_p][j / n_p];
    let local_cell = |j: usize| {
        let (p, g) = (j % n_p, j / n_p);
        !inp.traded[g] && inp.active[p][g] && s.supply[(p, g)] > 0.0
    };
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

    // The country's profit pool, its denominator: sum (1 - s_L) a_VA V.
    let n_c = inp.n_countries();
    let mut pool0 = vec![0.0; n_c];
    for (e, &p) in inp.ent_province.iter().enumerate() {
        pool0[country[p]] += (1.0 - s_l[e]) * s.a_va[e] * s.value[e];
    }
    let per_pool = |c: usize, x: f64| if pool0[c] > 0.0 { x / pool0[c] } else { 0.0 };

    let mut countries: Vec<Country> = (0..n_c).map(|_| Country::default()).collect();
    let mut local = vec![usize::MAX; n_pg];
    for j in (0..n_pg).filter(|&j| traded_cell(j) || local_cell(j)) {
        let (p, g) = (j % n_p, j / n_p);
        let c = &mut countries[country[p]];
        local[j] = c.cells.len();
        c.cells.push(j);
        c.revenue
            .push(per_pool(country[p], s.price[(p, g)] * s.supply[(p, g)]));
        if traded_cell(j) {
            c.demand
                .push((local[j], local[j], s.demand_elasticity[(p, g)]));
        }
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
            let v = -(1.0 + inp.theta[i / n_p]) * src.w[j] * t2 / t[i];
            countries[country[i % n_p]]
                .demand
                .push((local[i], local[j], v));
        }
    }
    // Provinces: every province of a country with an enterprise or a pop.
    let mut province_of = vec![usize::MAX; n_p];
    let mut add_province = |p: usize, countries: &mut [Country]| {
        if province_of[p] == usize::MAX {
            let c = &mut countries[country[p]];
            province_of[p] = c.provinces.len();
            c.provinces.push(Province {
                employed: 0.0,
                bill: 0.0,
            });
        }
        province_of[p]
    };
    for &p in inp.ent_province.iter().chain(&inp.pop_province) {
        add_province(p, &mut countries);
    }
    // Slots: each country's (province, traded good) where its enterprises or households buy.
    let mut slot_of = vec![usize::MAX; n_pg];
    let mut slot = |d: usize, c: &mut Country| -> usize {
        if slot_of[d] == usize::MAX {
            let (p, h) = (d % n_p, d / n_p);
            slot_of[d] = c.hold.len();
            c.hold.push(1.0 + COVER + s.lead[(p, h)]);
            for &(i, kid) in &incoming[d] {
                let u = if t[i] > 0.0 { kid / t[i] } else { 0.0 };
                let a = src.w[i] * kid / src.phi[d];
                let lag = if i % n_p == p { 0.0 } else { 1.0 };
                c.routes.push(Route {
                    slot: slot_of[d],
                    cell: local[i],
                    a,
                    u,
                    lead: -inp.theta[h] * a * (lag - s.lead[(p, h)]),
                });
            }
        }
        c.cells.len() + slot_of[d]
    };
    // Cells and slots are numbered per country before places are named: every cell exists now.
    for (e, &s_le) in s_l.iter().enumerate() {
        let p = inp.ent_province[e];
        let ci = country[p];
        let c = &mut countries[ci];
        let a_va = s.a_va[e];
        let eps = s_le * inp.sigma_fl / ((1.0 - s_le) * a_va);
        let own_cell = inp.ent_good[e] * n_p + p;
        let own =
            (local[own_cell] != usize::MAX && s.supply[(p, inp.ent_good[e])] > 0.0).then(|| {
                (
                    local[own_cell],
                    s.output[e] / s.supply[(p, inp.ent_good[e])],
                )
            });
        let labour = s.l0[e] / (inp.labour_supply[p] * s_le);
        let prov = province_of[p];
        c.provinces[prov].employed += s.l0[e] / inp.labour_supply[p];
        c.provinces[prov].bill += per_pool(ci, s_le * a_va * s.value[e]);
        let k = c.enterprises.len();
        c.enterprises.push(Enterprise {
            own,
            eps,
            province: prov,
            wage: eps * a_va,
            labour,
        });
        for h in 0..n_g {
            let d = h * n_p + p;
            let (place, spend, v) = if inp.traded[h] {
                let spend = s.ent_spend[(e, h)];
                if spend <= 0.0 {
                    continue;
                }
                (slot(d, c), spend, spend / src.phi[d])
            } else {
                let units = s.coef[(e, h)] * s.output[e];
                if units <= 0.0 || local[d] == usize::MAX {
                    continue;
                }
                let spend = units * s.price[(p, h)];
                (local[d], spend, units / s.supply[(p, h)])
            };
            let cost = eps * spend / s.value[e];
            c.inputs.push(Input {
                enterprise: k,
                place,
                v,
                cost: if cost.is_finite() { cost } else { 0.0 },
                spend: per_pool(ci, spend),
            });
        }
    }
    // Households: each province's pops, their spending at each place and its derivatives.
    let (pop_spend, money, cross) = pop_derivatives(m, s);
    let kc = inp.pop_country();
    let mut people = vec![0.0; n_c];
    for (k, &c) in kc.iter().enumerate() {
        people[c] += inp.pop_population[k];
    }
    let mut pops_of: Vec<Vec<usize>> = vec![Vec::new(); n_p];
    for (k, &p) in inp.pop_province.iter().enumerate() {
        pops_of[p].push(k);
    }
    for p in 0..n_p {
        let ci = country[p];
        let c = &mut countries[ci];
        // the place of each good in the province, where its price moves with the block
        let place: Vec<Option<usize>> = (0..n_g)
            .map(|h| {
                let d = h * n_p + p;
                if inp.traded[h] {
                    (!incoming[d].is_empty()).then(|| slot(d, c))
                } else {
                    (local[d] != usize::MAX).then_some(local[d])
                }
            })
            .collect();
        for g in 0..n_g {
            let Some(at) = place[g] else { continue };
            let d = g * n_p + p;
            let norm = if inp.traded[g] {
                src.phi[d]
            } else {
                s.supply[(p, g)]
            };
            let (mut wage, mut pool, mut spend) = (0.0, 0.0, 0.0);
            let mut prices = vec![0.0; n_g];
            for &k in &pops_of[p] {
                let (sk, income) = (pop_spend[(k, g)], s.pop_income[k]);
                if sk <= 0.0 || income <= 0.0 {
                    continue;
                }
                let dividend = inp.pop_population[k] / people[ci] * pool0[ci];
                wage += sk * money[(k, g)] * s.pop_wage[k] / income;
                pool += sk * money[(k, g)] * dividend / income;
                spend += sk;
                for (h, v) in prices.iter_mut().enumerate() {
                    *v += sk * cross[(k, g * n_g + h)];
                }
            }
            if spend <= 0.0 {
                continue;
            }
            c.households.push(Household {
                province: province_of[p],
                place: at,
                wage: wage / norm,
                pool: pool / norm,
                spend: spend / norm,
                prices: (0..n_g)
                    .filter_map(|h| place[h].map(|r| (r, prices[h] / norm)))
                    .filter(|&(_, v)| v != 0.0)
                    .collect(),
            });
        }
    }
    let gain = |j: usize| {
        let (p, g) = (j % n_p, j / n_p);
        trade.price_gain(
            s.demand_elasticity[(p, g)] + eta * s.supply_elasticity[(p, g)] / (2.0 - eta),
        )
    };
    let wage = if params.frictionless {
        Wage::Clearing
    } else {
        Wage::Rule {
            rise: params.kappa_w,
            cut: (1.0 - params.delta_w) * params.kappa_w,
        }
    };
    countries
        .into_iter()
        .filter(|c| !c.cells.is_empty())
        .map(|c| {
            let block = Block {
                gain: c.cells.iter().map(|&j| gain(j)).collect(),
                demand: c.demand,
                enterprises: c.enterprises,
                hold: c.hold,
                inputs: c.inputs,
                routes: c.routes,
                provinces: c.provinces,
                households: c.households,
                revenue: c.revenue,
                wage: wage.clone(),
            };
            (c.cells, block)
        })
        .collect()
}

/// The block limit gain of every cell (P, G) under `params` (ADR 0267): for a cell of a block
/// whose scale b (Block::scale) is below 1, b kappa_cell / m, the gain at which its block is at
/// the edge of stability, with the map checked stable at gains b kappa_cell; infinite for a cell
/// of a block already within its margin and for a cell in no block (inactive cells).
pub fn limits(
    m: &Model,
    src: &Sourcing,
    s: &Steady,
    s_l: &[f64],
    params: &EconParams,
    trade: &TradeParams,
) -> Result<Mat, String> {
    let n_p = m.inp.n_provinces();
    let blocks = blocks(m, src, s, s_l, params, trade);
    let scales: Vec<f64> = blocks
        .par_iter()
        .map(|(_, block)| block.scale(params.eta, trade.gain_share))
        .collect();
    let mut out = Mat::filled(n_p, m.inp.n_goods(), f64::INFINITY);
    for ((cells, block), &b) in blocks.iter().zip(&scales) {
        if b == 1.0 {
            continue;
        }
        let radius = block.spectral_radius(b, params.eta);
        if radius >= 1.0 {
            return Err(format!(
                "the block of cell {} has radius {radius} at its scaled gains (scale {b})",
                cells[0]
            ));
        }
        for (&j, &kappa) in cells.iter().zip(&block.gain) {
            out[(j % n_p, j / n_p)] = b * kappa / trade.gain_share;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KAPPA1: f64 = 0.05;
    const SHARE: f64 = 0.5;

    fn enterprise(own: usize, eps: f64) -> Enterprise {
        Enterprise {
            own: Some((own, 1.0)),
            eps,
            province: 0,
            wage: 0.0,
            labour: 0.0,
        }
    }

    fn market(e: f64, eps: f64) -> Block {
        Block {
            gain: vec![KAPPA1],
            demand: vec![(0, 0, e)],
            enterprises: vec![enterprise(0, eps)],
            hold: vec![],
            inputs: vec![],
            routes: vec![],
            provinces: vec![Province {
                employed: 0.0,
                bill: 0.0,
            }],
            households: vec![],
            revenue: vec![0.0],
            wage: Wage::Given,
        }
    }

    /// Two markets, each with one enterprise: A buys B's good as an input (one slot, all its
    /// supply from B), which is most of B's orders, and B's price is half of A's cost.
    fn coupled() -> Block {
        let (eps_a, eps_b) = (12.0, 4.0);
        Block {
            gain: vec![KAPPA1, KAPPA1],
            demand: vec![(0, 0, 6.0), (1, 1, 12.0)],
            enterprises: vec![enterprise(0, eps_a), enterprise(1, eps_b)],
            hold: vec![3.0],
            inputs: vec![Input {
                enterprise: 0,
                place: 2,
                v: 1.0,
                cost: 0.5 * eps_a,
                spend: 0.0,
            }],
            routes: vec![Route {
                slot: 0,
                cell: 1,
                a: 1.0,
                u: 0.8,
                lead: 0.0,
            }],
            revenue: vec![0.0, 0.0],
            ..market(0.0, 0.0)
        }
    }

    fn alone(b: &Block, i: usize) -> Block {
        let e = b.demand.iter().find(|d| d.0 == i && d.1 == i).unwrap().2;
        market(e, b.enterprises[i].eps)
    }

    /// One market with one enterprise at given wages is the SPEC's single-market map (Stability
    /// region, kappa2 0): frictionless its root is 1 - kappa1 (E + eps), and at any eta its scale
    /// is 1 at the ADR 0263 gain, which puts it at the share m of its J2 limit.
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

    /// A clearing wage absorbs a price move of the province's only enterprise: its output does
    /// not answer its price, and the map is the market's demand alone, root 1 - kappa1 E.
    #[test]
    fn a_clearing_wage_absorbs_the_own_price() {
        let (e, eps, a_va) = (10.0, 6.0, 0.4);
        let mut b = market(e, eps);
        b.wage = Wage::Clearing;
        b.enterprises[0].wage = eps * a_va;
        b.enterprises[0].labour = 1.0;
        b.provinces[0].employed = 0.95;
        for s in [1.0, 2.0, 3.5] {
            let want = (1.0 - s * KAPPA1 * e).abs();
            let got = b.spectral_radius(s, 1.0);
            assert!((got - want).abs() < 1e-3, "scale {s}: {got} vs {want}");
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

    /// Under the wage rule the map is measured at the gain of a rise and of a cut, and the
    /// radius is the larger.
    #[test]
    fn the_rule_takes_the_larger_radius_of_its_branches() {
        let mut b = market(10.0, 6.0);
        b.enterprises[0].wage = 2.4;
        b.enterprises[0].labour = 1.0;
        b.provinces[0].employed = 0.95;
        b.wage = Wage::Rule {
            rise: 0.1,
            cut: 0.05,
        };
        let (rise, cut) = (b.radius_at(1.0, 0.3, 0.1), b.radius_at(1.0, 0.3, 0.05));
        assert_ne!(rise, cut);
        assert_eq!(b.spectral_radius(1.0, 0.3), rise.max(cut));
    }
}
