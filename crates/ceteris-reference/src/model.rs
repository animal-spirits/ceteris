//! Steady state of the P2 tick pipeline with frictions off (`specs/econ/SPEC.md`, GE reference
//! model). Mirrors `ceteris-trade` `source` and `dispatch` (ADRs 0015 and 0016) and `ceteris-econ`
//! E1 to E4 at a fixed point: frictionless wage (employment at the natural rate, (1 - u*) L^s,
//! ADR 0089), every origin market selling its output (orders = Q; at the origins of the
//! competitively stored goods, orders (1 + delta s) = Q, the rest of output replacing the stock's
//! spoilage, ADR 0091), every local market clearing,
//! input stocks never binding (orders replace use), pops spending their whole income. Expected
//! inflation (ADR 0019) is zero at a steady state, since the world price index is constant there,
//! so the price and wage rules' pi^e term leaves this solution unchanged.
//!
//! Destination d spends E_d on traded good g, households (each pop by PIGL across categories and
//! CES within them, ADR 0021) at the buyer price q_d = Gamma_d P_d, P_d the CES index (ADR 0200,
//! selection on trade links; `Inputs::gamma`), and enterprises a Y q_d; every unit of spending
//! delivers 1 / q_d units, so arrivals replace use. Without Gamma ([`Model::physical`], the
//! calibration's model) q_d is the delivered unit price of the physical arrivals, 1 / alpha_d;
//! X_od = E_d K_od w_o / Phi_d with w_o = (p_o / p_min)^-theta_g (theta per good, ADR 0018), K
//! carrying the tariff on crossings. Origin o ships X_od / (p_o (1 + t)) units and receives p_o
//! per unit, the importer's rebate account t p_o. Weights are relative to one p_min per good: Rust
//! holds them per country in log scale (ADR 0093), which gives the same shares wherever the global
//! scale does not underflow and E_d / Phi_d does not overflow. A destination is reachable where
//! Phi_d > 0. Losses and duties multiply by the kernel's reciprocals.
//!
//! Unknowns x: ln p of every active (province, good) cell in row-major order (origin markets of
//! traded goods, province markets of local goods), then ln w per province. Equations: excess
//! demand of every active market, ln(D / Q) (with a floor far below market size), D the orders
//! placed at an origin or the demand at a local market, and of every labour market,
//! (sum L* - N) / N with N = (1 - u*) L^s employed at the natural rate. Market cash flow is zero at
//! a fixed point and left out, except the spoilage of stored goods, p Q delta s / (1 + delta s),
//! which its country's profits carry (at a solution it is the market's cash flow -p delta S),
//! and households are paid w N, so at a solution Walras' law reads
//! sum p (D - Q) + sum w (L* - N) = 0 identically; the labour equation of the province with the
//! largest base wage bill is replaced by ln(world price index) = 0, the index weighting consumer
//! prices (q_d for traded goods, the local price for local goods) and leaving out cells with no
//! reachable origin (q_d infinite), as `EconWorld::world_price_index`.
//!
//! Sums run in a fixed order (sequential per index, pairwise over contiguous runs), so an
//! evaluation is reproducible to the last bit.
//!
//! Layouts: (P, G) matrices are [`Mat`]s; routing works on flat good-major vectors, index
//! `g * P + i` for provinces and `g * N + a` for gateways; country pairs are flat (C, C, G),
//! index `(exporter * C + importer) * G + g`.

use crate::demand::{self, Weights};
use crate::io::{Inputs, Mat, pairwise_sum};

/// Cobb-Douglas threshold on |rho|, as `production.rs`.
pub const RHO_CD: f64 = 1e-9;
/// Relative change of every country's rebate that ends the income iteration.
pub const REBATE_TOL: f64 = 1e-15;
pub const MAX_REBATE_ITERATIONS: usize = 200;
/// Added to D and Q, relative to the cell's base size: ln(D / Q) stays finite.
pub const MARKET_FLOOR: f64 = 1e-12;

/// Flat indices of every kernel leg: provinces as `g * P + i`, gateways as `g * N + a`.
struct LegIndex {
    d_from: Vec<usize>,
    d_to: Vec<usize>,
    a_prov: Vec<usize>,
    a_gate: Vec<usize>,
    c_from: Vec<usize>,
    c_to: Vec<usize>,
}

/// Per-leg factors of one routing pass: domestic, access and crossing legs, in the order of the
/// kernel tables.
#[derive(Clone, Copy)]
pub struct Factors<'f> {
    pub dom: &'f [f64],
    pub acc: &'f [f64],
    pub cro: &'f [f64],
}

/// The inputs with every derived quantity an evaluation uses, computed once.
pub struct Model<'a> {
    pub inp: &'a Inputs,
    /// Selection constants of the buyer price (ADR 0200); `None` for [`Model::physical`].
    gamma: Option<&'a Mat>,
    /// (E, G) physical input coefficients.
    pub ent_inputs: Mat,
    pub ent_a_va: Vec<f64>,
    pub ent_s_l: Vec<f64>,
    pub weights: Weights,
    /// (P, G) weights of the world price index.
    pub index_weights: Mat,
    pub active_cells: Vec<usize>,
    pub employed_supply: Vec<f64>,
    pub rho: f64,
    /// MARKET_FLOOR (base demand + base output) of every active cell.
    market_floor: Vec<f64>,
    /// (G,) delta_g s_g: spoilage of the stock per unit of orders at a steady state, which
    /// origins of competitively stored goods produce on top of their orders (ADR 0091); 0
    /// elsewhere and for inputs without coverage.
    pub spoilage: Vec<f64>,
    legs: LegIndex,
    ent_country: Vec<usize>,
    pop_country: Vec<usize>,
    /// e_k LF_k / L^s of the pop's province.
    pop_labour_share: Vec<f64>,
    /// The pop's share of its country's people.
    pop_country_share: Vec<f64>,
    pop_sigma: Vec<f64>,
    dom_zero: Vec<f64>,
    dom_arrive: Vec<f64>,
    acc_arrive: Vec<f64>,
    /// K / (1 + t) of each crossing: what origins receive.
    cro_net: Vec<f64>,
    cro_arrive: Vec<f64>,
    cro_duty: Vec<f64>,
}

/// Origin weights, Phi, the CES index and the buyer prices of every traded cell, as
/// `EconWorld::refresh_price_index`.
pub struct Sourcing {
    /// Good-major (G * P): (p / p_min)^-theta_g of active traded origins, 0 elsewhere.
    pub w: Vec<f64>,
    /// Good-major (G * P).
    pub phi: Vec<f64>,
    /// (P, G) CES index P_d = p_min Phi_d^(-1/theta), infinite where no active origin is
    /// reachable.
    pub ces: Mat,
    /// (P, G) buyer price q: Gamma P_d with selection (ADR 0200), else 1 / alpha_d of the
    /// physical arrivals; infinite where no active origin is reachable.
    pub index: Mat,
    /// (P, G) units arriving per unit of spending, 1 / q.
    pub alpha: Mat,
}

/// Sourcing of a spending matrix, fully filled (`source` and `dispatch`). (P, G) unless noted.
pub struct Flows {
    /// Origin units ordered.
    pub orders: Mat,
    pub home: Mat,
    /// Delivered units, 1 / q per unit paid.
    pub arrive: Mat,
    /// What each destination pays, its spending where it reaches an origin.
    pub paid: Mat,
    /// What each origin receives, p_o times its orders.
    pub received: Mat,
    /// The tariff t / (1 + t) of crossings, by destination.
    pub tariff: Mat,
    /// (C, C, G) origin value of shipped units, exporter x importer.
    pub pair_value: Vec<f64>,
    /// (C, C, G) origin units at dispatch.
    pub pair_units: Vec<f64>,
}

/// Every quantity of the steady state at prices and wages x.
pub struct Evaluation {
    /// (P, G) posted price of active cells, 0 elsewhere.
    pub price: Mat,
    pub wage: Vec<f64>,
    /// (E,)
    pub employment: Vec<f64>,
    /// (E,)
    pub output: Vec<f64>,
    /// (E,) nominal: sales less inputs at buyer prices.
    pub value_added: Vec<f64>,
    /// (P, G) traded output of each origin cell.
    pub supply: Mat,
    /// (P, G) q_d of traded goods, the local price of local goods.
    pub consumer_price: Mat,
    /// (P, G) destination spending on traded goods, tariff included.
    pub spend: Mat,
    pub flows: Flows,
    /// (P, G) orders at origins of traded goods, demand at local markets.
    pub demand: Mat,
    /// (P, G) output of each cell.
    pub q: Mat,
    pub labour_demand: Vec<f64>,
    /// (P,) household income.
    pub income: Vec<f64>,
    /// World price index.
    pub index: f64,
}

fn bincount(index: &[usize], weights: impl IntoIterator<Item = f64>, n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (&i, w) in index.iter().zip(weights) {
        out[i] += w;
    }
    out
}

/// (P, G) to flat good-major.
pub fn good_major(m: &Mat) -> Vec<f64> {
    let mut out = Vec::with_capacity(m.data.len());
    for g in 0..m.cols {
        out.extend((0..m.rows).map(|p| m[(p, g)]));
    }
    out
}

/// Flat good-major to (P, G).
pub fn cells(v: &[f64], n_p: usize, n_g: usize) -> Mat {
    Mat::from_fn(n_p, n_g, |p, g| v[g * n_p + p])
}

/// ln of each country pair's trade-cost kernel net of tariffs, flat (C, C, G) exporter x importer
/// (ADR 0264): the sum over crossings a -> b of u_a K_ab (1 + t_ab)^theta v_b, where u_a is the
/// exporter's access to gateway a averaged over its provinces by `origin` weight and v_b the
/// importer's by `destination` weight, both (P, G). With the sourcing weights w_o and E_d / Phi_d
/// as the weights, a pair's flow is its exporter's total weight times its importer's times
/// (1 + t)^-theta times this kernel. -inf where no crossing links the pair, for domestic pairs
/// and for local goods.
pub fn ln_country_kernel(inp: &Inputs, origin: &Mat, destination: &Mat) -> Vec<f64> {
    let (n_c, n_g, n_n) = (inp.n_countries(), inp.n_goods(), inp.n_gateways());
    let pc = &inp.province_country;
    let mut origin_c = vec![0.0; n_c * n_g];
    let mut destination_c = vec![0.0; n_c * n_g];
    for p in 0..inp.n_provinces() {
        for g in (0..n_g).filter(|&g| inp.traded[g]) {
            origin_c[pc[p] * n_g + g] += origin[(p, g)];
            destination_c[pc[p] * n_g + g] += destination[(p, g)];
        }
    }
    let (mut u, mut v) = (vec![0.0; n_n * n_g], vec![0.0; n_n * n_g]);
    let acc = &inp.access;
    for j in 0..acc.k.len() {
        let (p, a, g) = (acc.row[j], acc.col[j], acc.good[j]);
        let c = pc[p] * n_g + g;
        if origin_c[c] > 0.0 {
            u[a * n_g + g] += origin[(p, g)] / origin_c[c] * acc.k[j];
        }
        if destination_c[c] > 0.0 {
            v[a * n_g + g] += destination[(p, g)] / destination_c[c] * acc.k[j];
        }
    }
    let mut k = vec![0.0; n_c * n_c * n_g];
    let (cro, gc) = (&inp.crossing, &inp.gateway_country);
    for j in 0..cro.k.len() {
        let (a, b, g) = (cro.row[j], cro.col[j], cro.good[j]);
        let net = cro.k[j] * cro.inv_duty[j].powf(-inp.theta[g]);
        k[(gc[a] * n_c + gc[b]) * n_g + g] += u[a * n_g + g] * net * v[b * n_g + g];
    }
    k.iter().map(|x| x.ln()).collect()
}

/// numpy's `max` of |v|: NaN wins.
pub fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m: f64, &x| {
        if m.is_nan() || x.is_nan() {
            f64::NAN
        } else {
            m.max(x.abs())
        }
    })
}

impl<'a> Model<'a> {
    /// The model of `inp`, buyers at Gamma P_d where the inputs carry Gamma (ADR 0200).
    pub fn new(inp: &'a Inputs) -> Result<Model<'a>, String> {
        Model::with_gamma(inp, inp.gamma.as_ref())
    }

    /// The model of `inp` with buyers at the delivered unit price of the physical arrivals,
    /// 1 / alpha_d, whatever Gamma the inputs carry: the calibration's, which fixes Gamma where
    /// it settles.
    pub fn physical(inp: &'a Inputs) -> Result<Model<'a>, String> {
        Model::with_gamma(inp, None)
    }

    fn with_gamma(inp: &'a Inputs, gamma: Option<&'a Mat>) -> Result<Model<'a>, String> {
        let (n_p, n_n) = (inp.n_provinces(), inp.n_gateways());
        let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
        let flat = |good: &[usize], at: &[usize], n: usize| -> Vec<usize> {
            good.iter().zip(at).map(|(&g, &i)| g * n + i).collect()
        };
        let legs = LegIndex {
            d_from: flat(&dom.good, &dom.row, n_p),
            d_to: flat(&dom.good, &dom.col, n_p),
            a_prov: flat(&acc.good, &acc.row, n_p),
            a_gate: flat(&acc.good, &acc.col, n_n),
            c_from: flat(&cro.good, &cro.row, n_n),
            c_to: flat(&cro.good, &cro.col, n_n),
        };
        let times =
            |a: &[f64], b: &[f64]| -> Vec<f64> { a.iter().zip(b).map(|(x, y)| x * y).collect() };
        let cro_net = times(&cro.k, &cro.inv_duty);
        let cro_arrive = times(&cro_net, &cro.inv_loss);
        let cro_duty = cro.k.iter().zip(&cro_net).map(|(k, n)| k - n).collect();

        let c = &inp.province_country;
        let pop_country: Vec<usize> = inp.pop_province.iter().map(|&p| c[p]).collect();
        let country_people = bincount(
            &pop_country,
            inp.pop_population.iter().copied(),
            inp.n_countries(),
        );
        let pop_labour_share = inp
            .pop_effective
            .iter()
            .zip(&inp.pop_province)
            .map(|(e, &p)| e / inp.labour_supply[p])
            .collect();
        let pop_country_share = inp
            .pop_population
            .iter()
            .zip(&pop_country)
            .map(|(n, &k)| n / country_people[k])
            .collect();
        let weights = inp.pigl_weights()?;
        let pop_sigma = pop_country.iter().map(|&k| weights.sigma[k]).collect();

        let active_cells = inp.active_cells();
        let (base_demand, base_output) = (inp.base_demand(), inp.base_output());
        let market_floor = active_cells
            .iter()
            .map(|&i| MARKET_FLOOR * (base_demand.data[i] + base_output.data[i]))
            .collect();
        let spoilage = (0..inp.n_goods())
            .map(|g| match &inp.coverage {
                Some(cover) if inp.traded[g] => inp.storage_loss[g] * cover[g],
                _ => 0.0,
            })
            .collect();
        Ok(Model {
            spoilage,
            ent_inputs: inp.ent_inputs(),
            ent_a_va: inp.ent_a_va(),
            ent_s_l: inp.ent_s_l(),
            weights,
            index_weights: inp.index_weights(),
            employed_supply: inp.employed_supply(),
            rho: (inp.sigma_fl - 1.0) / inp.sigma_fl,
            market_floor,
            active_cells,
            legs,
            ent_country: inp.ent_country(),
            pop_country,
            pop_labour_share,
            pop_country_share,
            pop_sigma,
            dom_zero: dom.k.iter().map(|k| 0.0 * k).collect(),
            dom_arrive: times(&dom.k, &dom.inv_tau),
            acc_arrive: times(&acc.k, &acc.inv_tau),
            cro_net,
            cro_arrive,
            cro_duty,
            inp,
            gamma,
        })
    }

    /// Number of unknowns: active cells, then provinces.
    pub fn n_unknowns(&self) -> usize {
        self.active_cells.len() + self.inp.n_provinces()
    }

    /// Prices 1 and base wages.
    pub fn data_state(&self) -> Vec<f64> {
        let mut x = vec![0.0; self.active_cells.len()];
        x.extend(self.inp.w0.iter().map(|w| w.ln()));
        x
    }

    /// The province with the largest base wage bill, whose labour equation the price index
    /// replaces.
    pub fn dropped_province(&self) -> usize {
        let inp = self.inp;
        let bill: Vec<f64> = inp
            .w0
            .iter()
            .zip(&inp.labour_supply)
            .map(|(w, l)| w * l)
            .collect();
        // numpy's argmax: the first maximum, or the first NaN.
        let mut best = 0;
        for (i, &b) in bill.iter().enumerate() {
            if b.is_nan() {
                return i;
            }
            if b > bill[best] {
                best = i;
            }
        }
        best
    }

    pub fn index_row(&self) -> usize {
        self.active_cells.len() + self.dropped_province()
    }

    fn split(&self, x: &[f64]) -> (Mat, Vec<f64>) {
        let inp = self.inp;
        let n = self.active_cells.len();
        let mut price = Mat::zeros(inp.n_provinces(), inp.n_goods());
        for (&cell, v) in self.active_cells.iter().zip(&x[..n]) {
            price.data[cell] = v.exp();
        }
        (price, x[n..].iter().map(|v| v.exp()).collect())
    }

    /// sum over routes o -> d of the route's leg factors times w_o, for every destination d, home
    /// route (factor 1) included: the origin-side access sums of `Kernel::origin_access_into`.
    pub fn origin_sums(&self, w: &[f64], f: Factors) -> Vec<f64> {
        let inp = self.inp;
        let l = &self.legs;
        let (n_pg, n_ng) = (w.len(), inp.n_goods() * inp.n_gateways());
        let v = bincount(
            &l.a_gate,
            f.acc.iter().zip(&l.a_prov).map(|(k, &i)| k * w[i]),
            n_ng,
        );
        let u = bincount(
            &l.c_to,
            f.cro.iter().zip(&l.c_from).map(|(k, &a)| k * v[a]),
            n_ng,
        );
        let dom = bincount(
            &l.d_to,
            f.dom.iter().zip(&l.d_from).map(|(k, &i)| k * w[i]),
            n_pg,
        );
        let acc = bincount(
            &l.a_prov,
            f.acc.iter().zip(&l.a_gate).map(|(k, &a)| k * u[a]),
            n_pg,
        );
        (0..n_pg).map(|j| w[j] + dom[j] + acc[j]).collect()
    }

    /// sum over routes o -> d of the route's leg factors times r_d, for every origin o, home
    /// route included: the forward pass of `dispatch`.
    pub fn destination_sums(&self, r: &[f64], f: Factors) -> Vec<f64> {
        let inp = self.inp;
        let l = &self.legs;
        let (n_pg, n_ng) = (r.len(), inp.n_goods() * inp.n_gateways());
        let y = bincount(
            &l.a_gate,
            f.acc.iter().zip(&l.a_prov).map(|(k, &i)| k * r[i]),
            n_ng,
        );
        let z = bincount(
            &l.c_from,
            f.cro.iter().zip(&l.c_to).map(|(k, &b)| k * y[b]),
            n_ng,
        );
        let dom = bincount(
            &l.d_from,
            f.dom.iter().zip(&l.d_to).map(|(k, &i)| k * r[i]),
            n_pg,
        );
        let acc = bincount(
            &l.a_prov,
            f.acc.iter().zip(&l.a_gate).map(|(k, &a)| k * z[a]),
            n_pg,
        );
        (0..n_pg).map(|j| r[j] + dom[j] + acc[j]).collect()
    }

    fn gross(&self) -> Factors<'_> {
        let inp = self.inp;
        Factors {
            dom: &inp.domestic.k,
            acc: &inp.access.k,
            cro: &inp.crossing.k,
        }
    }

    fn arriving(&self) -> Factors<'_> {
        Factors {
            dom: &self.dom_arrive,
            acc: &self.acc_arrive,
            cro: &self.cro_arrive,
        }
    }

    /// Origin weights of active traded origins, Phi, the CES index, the buyer price and alpha,
    /// at posted prices (P, G).
    pub fn sourcing(&self, price: &Mat) -> Sourcing {
        let inp = self.inp;
        let (n_p, n_g) = (inp.n_provinces(), inp.n_goods());
        let p = good_major(price);
        let mut w = vec![0.0; n_g * n_p];
        let mut p_low = vec![f64::INFINITY; n_g];
        for g in 0..n_g {
            let act = |i: usize| inp.traded[g] && inp.active[i][g];
            // numpy's min: NaN wins.
            let p_min = (0..n_p).filter(|&i| act(i)).fold(f64::INFINITY, |m, i| {
                let v = p[g * n_p + i];
                if m.is_nan() || v.is_nan() {
                    f64::NAN
                } else {
                    m.min(v)
                }
            });
            for i in (0..n_p).filter(|&i| act(i)) {
                w[g * n_p + i] = (p[g * n_p + i] / p_min).powf(-inp.theta[g]);
            }
            p_low[g] = p_min;
        }
        let phi = self.origin_sums(&w, self.gross());
        let per_price: Vec<f64> = w
            .iter()
            .zip(&p)
            .map(|(&w, &p)| if w > 0.0 { w / p } else { 0.0 })
            .collect();
        let arrive = self.origin_sums(&per_price, self.arriving());
        let ces: Vec<f64> = (0..n_g * n_p)
            .map(|j| {
                let g = j / n_p;
                if phi[j] > 0.0 {
                    p_low[g] * phi[j].powf(-1.0 / inp.theta[g])
                } else {
                    f64::INFINITY
                }
            })
            .collect();
        let (index, alpha): (Vec<f64>, Vec<f64>) = match self.gamma {
            Some(gamma) => {
                let gamma = good_major(gamma);
                let index: Vec<f64> = ces.iter().zip(&gamma).map(|(c, g)| g * c).collect();
                let alpha = index
                    .iter()
                    .map(|&q| if q.is_finite() { 1.0 / q } else { 0.0 })
                    .collect();
                (index, alpha)
            }
            None => {
                let alpha: Vec<f64> = arrive
                    .iter()
                    .zip(&phi)
                    .map(|(&a, &f)| if f > 0.0 { a / f } else { 0.0 })
                    .collect();
                let index = alpha
                    .iter()
                    .map(|&a| if a > 0.0 { 1.0 / a } else { f64::INFINITY })
                    .collect();
                (index, alpha)
            }
        };
        Sourcing {
            ces: cells(&ces, n_p, n_g),
            index: cells(&index, n_p, n_g),
            alpha: cells(&alpha, n_p, n_g),
            w,
            phi,
        }
    }

    /// Sourcing of `spend` (P, G), spending by destination, fully filled.
    pub fn flows(&self, price: &Mat, spend: &Mat, src: &Sourcing) -> Flows {
        let inp = self.inp;
        let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
        let n_ng = n_g * inp.n_gateways();
        let (dom, acc, cro) = (&inp.domestic, &inp.access, &inp.crossing);
        let l = &self.legs;
        let (pf, wf, phi) = (good_major(price), &src.w, &src.phi);
        let mut ef = good_major(spend);
        for g in (0..n_g).filter(|&g| !inp.traded[g]) {
            ef[g * n_p..(g + 1) * n_p].fill(0.0);
        }
        let reach: Vec<f64> = ef
            .iter()
            .zip(phi)
            .map(|(&e, &f)| if f > 0.0 { e / f } else { 0.0 })
            .collect();
        let per_price: Vec<f64> = wf
            .iter()
            .zip(&pf)
            .map(|(&w, &p)| if w > 0.0 { w / p } else { 0.0 })
            .collect();
        let net = Factors {
            dom: &dom.k,
            acc: &acc.k,
            cro: &self.cro_net,
        };
        let orders: Vec<f64> = self
            .destination_sums(&reach, net)
            .iter()
            .zip(&per_price)
            .map(|(t, p)| p * t)
            .collect();
        let home: Vec<f64> = per_price.iter().zip(&reach).map(|(p, r)| p * r).collect();
        let arrive: Vec<f64> = if self.gamma.is_some() {
            let alpha = good_major(&src.alpha);
            ef.iter()
                .zip(phi)
                .zip(&alpha)
                .map(|((&e, &f), &a)| if f > 0.0 { e * a } else { 0.0 })
                .collect()
        } else {
            self.origin_sums(&per_price, self.arriving())
                .iter()
                .zip(&reach)
                .map(|(a, r)| r * a)
                .collect()
        };
        let duty_factors = Factors {
            dom: &self.dom_zero,
            acc: &acc.k,
            cro: &self.cro_duty,
        };
        let tariff: Vec<f64> = self
            .origin_sums(wf, duty_factors)
            .iter()
            .zip(wf)
            .zip(&reach)
            .map(|((d, w), r)| r * (d - w))
            .collect();
        let paid: Vec<f64> = ef
            .iter()
            .zip(phi)
            .map(|(&e, &f)| if f > 0.0 { e } else { 0.0 })
            .collect();
        let received: Vec<f64> = pf.iter().zip(&orders).map(|(p, o)| p * o).collect();

        // Country pairs: domestic legs by province countries, crossings by gateway countries.
        let (pc, gc) = (&inp.province_country, &inp.gateway_country);
        let n_pairs = n_c * n_c * n_g;
        let gateway_sum = |values: &[f64]| {
            bincount(
                &l.a_gate,
                acc.k.iter().zip(&l.a_prov).map(|(k, &i)| k * values[i]),
                n_ng,
            )
        };
        let y = gateway_sum(&reach);
        let v = gateway_sum(wf);
        let v_units = gateway_sum(&per_price);
        let pair_dom: Vec<usize> = (0..dom.good.len())
            .map(|j| (pc[dom.row[j]] * n_c + pc[dom.col[j]]) * n_g + dom.good[j])
            .collect();
        let pair_int: Vec<usize> = (0..cro.good.len())
            .map(|j| (gc[cro.row[j]] * n_c + gc[cro.col[j]]) * n_g + cro.good[j])
            .collect();
        let pairs = |origin: &[f64], gate: &[f64]| -> Vec<f64> {
            let domestic = bincount(
                &pair_dom,
                (0..dom.good.len()).map(|j| origin[l.d_from[j]] * dom.k[j] * reach[l.d_to[j]]),
                n_pairs,
            );
            let crossing = bincount(
                &pair_int,
                (0..cro.good.len()).map(|j| gate[l.c_from[j]] * self.cro_net[j] * y[l.c_to[j]]),
                n_pairs,
            );
            domestic.iter().zip(&crossing).map(|(a, b)| a + b).collect()
        };
        Flows {
            orders: cells(&orders, n_p, n_g),
            home: cells(&home, n_p, n_g),
            arrive: cells(&arrive, n_p, n_g),
            paid: cells(&paid, n_p, n_g),
            received: cells(&received, n_p, n_g),
            tariff: cells(&tariff, n_p, n_g),
            pair_value: pairs(wf, &v),
            pair_units: pairs(&per_price, &v_units),
        }
    }

    /// Scaled excess demand of every market, all labour markets included.
    pub fn excess_of(&self, s: &Evaluation) -> Vec<f64> {
        let mut r: Vec<f64> = self
            .active_cells
            .iter()
            .zip(&self.market_floor)
            .map(|(&i, &eps)| ((s.demand.data[i] + eps) / (s.q.data[i] + eps)).ln())
            .collect();
        r.extend(
            s.labour_demand
                .iter()
                .zip(&self.employed_supply)
                .map(|(d, n)| (d - n) / n),
        );
        r
    }

    /// [`Model::excess_of`] with the dropped labour equation replaced by ln(world price index).
    pub fn market_residual(&self, s: &Evaluation) -> Vec<f64> {
        let mut r = self.excess_of(s);
        r[self.index_row()] = s.index.ln();
        r
    }
}

/// `production::labour_target_uncapped` of one enterprise: the frictionless wage solves on
/// uncapped targets in Rust too, which keeps labour demand responsive to the wage everywhere.
#[allow(clippy::too_many_arguments)]
fn labour_target(p_va: f64, w: f64, a: f64, va0: f64, l0: f64, s_l: f64, r: f64) -> f64 {
    let q = w * l0 / (p_va * a * va0 * s_l);
    let target = if r.abs() < RHO_CD {
        l0 * q.powf(1.0 / (s_l - 1.0))
    } else {
        let bracket = (q.powf(r / (1.0 - r)) - s_l) / (1.0 - s_l);
        if bracket > 0.0 {
            l0 * bracket.abs().powf(-1.0 / r)
        } else if r < 0.0 {
            0.0
        } else {
            f64::INFINITY
        }
    };
    if p_va <= 0.0 {
        0.0
    } else if w <= 0.0 {
        f64::INFINITY
    } else {
        target
    }
}

/// `production::value_added`.
fn value_added(a: f64, va0: f64, l0: f64, s_l: f64, r: f64, labour: f64) -> f64 {
    let x = labour / l0;
    if r.abs() < RHO_CD {
        a * va0 * x.powf(s_l)
    } else {
        a * va0 * ((1.0 - s_l) + s_l * x.powf(r)).powf(1.0 / r)
    }
}

/// Every quantity of the steady state at prices and wages `x`, with productivity `a` per
/// enterprise (the inputs' A for the reference solve).
pub fn evaluate(x: &[f64], m: &Model, a: &[f64]) -> Evaluation {
    let inp = m.inp;
    let (n_p, n_g, n_c) = (inp.n_provinces(), inp.n_goods(), inp.n_countries());
    let n_e = inp.ent_province.len();
    let traded = &inp.traded;
    let (price, wage) = m.split(x);
    let src = m.sourcing(&price);
    let consumer = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            src.index[(p, g)]
        } else {
            price[(p, g)]
        }
    });

    // E1, E2: frictionless labour target at the price index of inputs, output at the own price.
    let (ep, eg) = (&inp.ent_province, &inp.ent_good);
    let coef = &m.ent_inputs;
    let mut row = vec![0.0; n_g];
    let mut sale = Vec::with_capacity(n_e);
    let mut l_star = Vec::with_capacity(n_e);
    let mut y = Vec::with_capacity(n_e);
    let mut out = Mat::zeros(n_p, n_g);
    for e in 0..n_e {
        for (k, v) in row.iter_mut().enumerate() {
            let c = coef[(e, k)];
            *v = if c > 0.0 {
                c * consumer[(ep[e], k)]
            } else {
                0.0
            };
        }
        let input_index = pairwise_sum(&row);
        let s = price[(ep[e], eg[e])];
        let l = labour_target(
            (s - input_index) / m.ent_a_va[e],
            wage[ep[e]],
            a[e],
            inp.ent_va0[e],
            inp.ent_l0[e],
            m.ent_s_l[e],
            m.rho,
        );
        let v = value_added(a[e], inp.ent_va0[e], inp.ent_l0[e], m.ent_s_l[e], m.rho, l)
            / m.ent_a_va[e];
        out[(ep[e], eg[e])] += v;
        sale.push(s);
        l_star.push(l);
        y.push(v);
    }

    // E3, enterprises: traded inputs bought at q_d, so arrivals replace use, local inputs at the
    // province price.
    let mut cost = Vec::with_capacity(n_e);
    let mut spend_int = Mat::zeros(n_p, n_g);
    let mut local_use = Mat::zeros(n_p, n_g);
    let mut local_row = vec![0.0; n_g];
    for e in 0..n_e {
        let p = ep[e];
        for k in 0..n_g {
            let c = coef[(e, k)];
            let u = c * y[e];
            let q = consumer[(p, k)];
            row[k] = if c > 0.0 && traded[k] && q.is_finite() {
                u * q
            } else {
                0.0
            };
            local_row[k] = if traded[k] { 0.0 } else { price[(p, k)] * u };
        }
        cost.push(pairwise_sum(&row) + pairwise_sum(&local_row));
        for k in 0..n_g {
            spend_int[(p, k)] += row[k];
            local_use[(p, k)] += if traded[k] { 0.0 } else { coef[(e, k)] * y[e] };
        }
    }
    let profit: Vec<f64> = (0..n_e)
        .map(|e| sale[e] * y[e] - wage[ep[e]] * l_star[e] - cost[e])
        .collect();

    // E3, households: PIGL per pop (ADR 0021) at its province's consumer prices and its mean
    // income per person. E4: a pop earns its effective-labour share e_k LF_k / L^s of its
    // province's wages w (1 - u*) L^s, and its population share of its country's profits
    // (national ownership) and of its country's rebate, the tariff share of spending; rebates
    // depend on household spending, so income is a fixed point.
    let ones = Mat::filled(n_p, n_g, 1.0);
    let duty_share = m.flows(&price, &ones, &src).tariff;
    let c = &inp.province_country;
    let kp = &inp.pop_province;
    let kc = &m.pop_country;
    let people = &inp.pop_population;
    let n_k = kp.len();
    // ADR 0091: each origin of a stored good buys its output and sells Q / (1 + delta s), the
    // rest replacing spoilage, a cash loss its country's profit pool carries (merchant rule)
    let mut country_profit = bincount(&m.ent_country, profit.iter().copied(), n_c);
    for p in 0..n_p {
        for g in (0..n_g).filter(|&g| m.spoilage[g] > 0.0) {
            let share = m.spoilage[g] / (1.0 + m.spoilage[g]);
            country_profit[c[p]] -= price[(p, g)] * out[(p, g)] * share;
        }
    }
    let earned: Vec<f64> = (0..n_k)
        .map(|k| {
            m.pop_labour_share[k] * (wage[kp[k]] * m.employed_supply[kp[k]])
                + m.pop_country_share[k] * country_profit[kc[k]]
        })
        .collect();
    let duty_bill = |spend: &Mat| -> Vec<f64> {
        let rows = Mat::from_fn(n_p, n_g, |p, g| duty_share[(p, g)] * spend[(p, g)]).row_sums();
        bincount(c, rows, n_c)
    };
    let from_inputs = duty_bill(&spend_int);
    let prices_k = consumer.select_rows(kp);
    let pop_income_at = |rebate: &[f64]| -> Vec<f64> {
        (0..n_k)
            .map(|k| earned[k] + m.pop_country_share[k] * rebate[kc[k]])
            .collect()
    };
    let mut rebate = vec![0.0; n_c];
    let mut spend_hh = Mat::zeros(n_p, n_g);
    for _ in 0..MAX_REBATE_ITERATIONS {
        let pop_income = pop_income_at(&rebate);
        let per_person: Vec<f64> = (0..n_k)
            .map(|k| {
                if people[k] > 0.0 {
                    pop_income[k] / people[k]
                } else {
                    0.0
                }
            })
            .collect();
        let share = demand::shares(
            &m.weights,
            kc,
            &prices_k,
            &per_person,
            &m.pop_sigma,
            &inp.category,
            inp.sigma_demand,
            &inp.pigl,
        );
        spend_hh = Mat::zeros(n_p, n_g);
        for k in 0..n_k {
            for g in 0..n_g {
                spend_hh[(kp[k], g)] += if pop_income[k] > 0.0 {
                    share[(k, g)] * pop_income[k]
                } else {
                    0.0
                };
            }
        }
        let nxt: Vec<f64> = from_inputs
            .iter()
            .zip(duty_bill(&spend_hh))
            .map(|(a, b)| a + b)
            .collect();
        let done = nxt
            .iter()
            .zip(&rebate)
            .all(|(n, r)| (n - r).abs() <= REBATE_TOL * n.abs().max(1e-300));
        rebate = nxt;
        if done {
            break;
        }
    }
    let income = bincount(kp, pop_income_at(&rebate), n_p);

    let spend = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            spend_hh[(p, g)] + spend_int[(p, g)]
        } else {
            0.0
        }
    });
    let flows = m.flows(&price, &spend, &src);
    let demand = Mat::from_fn(n_p, n_g, |p, g| {
        if traded[g] {
            flows.orders[(p, g)] * (1.0 + m.spoilage[g])
        } else {
            let pr = price[(p, g)];
            let hh = if pr > 0.0 { spend_hh[(p, g)] / pr } else { 0.0 };
            hh + local_use[(p, g)]
        }
    });
    // As `EconWorld::world_price_index`: cells with no reachable origin are left out.
    let reached = |i: usize| consumer.data[i].is_finite();
    let weights: Vec<f64> = (0..n_p * n_g)
        .map(|i| {
            if reached(i) {
                m.index_weights.data[i]
            } else {
                0.0
            }
        })
        .collect();
    let weighted: Vec<f64> = (0..n_p * n_g)
        .map(|i| weights[i] * if reached(i) { consumer.data[i] } else { 0.0 })
        .collect();
    let index = pairwise_sum(&weighted) / pairwise_sum(&weights);

    Evaluation {
        value_added: (0..n_e).map(|e| sale[e] * y[e] - cost[e]).collect(),
        supply: Mat::from_fn(n_p, n_g, |p, g| if traded[g] { out[(p, g)] } else { 0.0 }),
        labour_demand: bincount(ep, l_star.iter().copied(), n_p),
        q: out,
        price,
        wage,
        employment: l_star,
        output: y,
        consumer_price: consumer,
        spend,
        flows,
        demand,
        income,
        index,
    }
}

/// Scaled excess demand of every market at `x`, all labour markets included, at the inputs' A.
pub fn excess(x: &[f64], m: &Model) -> Vec<f64> {
    m.excess_of(&evaluate(x, m, &m.inp.ent_a))
}

/// [`excess`] with the dropped labour equation replaced by ln(world price index).
pub fn residual(x: &[f64], m: &Model) -> Vec<f64> {
    m.market_residual(&evaluate(x, m, &m.inp.ent_a))
}
