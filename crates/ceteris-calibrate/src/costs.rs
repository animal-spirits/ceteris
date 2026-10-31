//! S05 least-cost costs on the pack's transport graph: edge prices, least-cost trees with path
//! quantities and the mean costs between countries the trade kernel builds.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::path::Path;

use ceteris_core::rows::Rows;
use ceteris_reference::io::pairwise_sum;
use ceteris_trade::TradeParams;
use rayon::prelude::*;

use crate::write::Table;

pub const ROAD: u8 = 0;
pub const RAIL: u8 = 1;
pub const SEA: u8 = 2;
pub const AIR: u8 = 3;
pub const RAIL_ACCESS: u8 = 4;
pub const PORT_ACCESS: u8 = 5;
pub const MINOR_PORT_ACCESS: u8 = 6;
pub const AIR_ACCESS: u8 = 7;
pub const CANAL: u8 = 8;
/// Path quantities per tonne: cost-relevant km by road, rail, sea and air (length times
/// circuity times the terrain factor, the km that P04a-d price), handling and canal fees (USD),
/// and hours.
pub const N_Q: usize = 6;
pub const Q_FEE: usize = 4;
pub const Q_HOURS: usize = 5;
pub const MODE_NAMES: [&str; 4] = ["road", "rail", "sea", "air"];

/// Path quantities of one edge or path.
pub type Quantities = [f64; N_Q];

/// Mirror of `ceteris_geo::graph::TransportParams::default()` (geo SPEC, P04).
#[derive(Clone, Debug, PartialEq)]
pub struct Transport {
    /// P04a-d.
    pub per_tkm: [f64; 4],
    /// P04e-h.
    pub circuity: [f64; 4],
    /// P04i-l.
    pub speed_kmh: [f64; 4],
    /// P04m: rail terminal, listed port, minor port, airport.
    pub fee: [f64; 4],
    /// P04n.
    pub fee_hours: [f64; 4],
    /// P04o.
    pub canal_fee: f64,
    /// P04p.
    pub terrain_slope: f64,
    /// P04q.
    pub terrain_cap: f64,
}

impl Default for Transport {
    fn default() -> Self {
        Transport {
            per_tkm: [0.10, 0.04, 0.008, 0.80],
            circuity: [1.3, 1.3, 1.0, 1.05],
            speed_kmh: [40.0, 25.0, 25.0, 500.0],
            fee: [5.0, 8.0, 20.0, 60.0],
            fee_hours: [24.0, 48.0, 72.0, 12.0],
            canal_fee: 5.0,
            terrain_slope: 0.5,
            terrain_cap: 3.0,
        }
    }
}

/// Per-edge path quantities as `ceteris_geo::graph::price` prices them.
pub fn edge_quantities(
    mode: &[u8],
    length_km: &[f64],
    tri_m: &[f64],
    p: &Transport,
) -> Vec<Quantities> {
    mode.iter()
        .zip(length_km)
        .zip(tri_m)
        .map(|((&m, &length), &tri)| {
            let mut q = [0.0; N_Q];
            let phi = (1.0 + p.terrain_slope * tri / 100.0).min(p.terrain_cap);
            match m {
                ROAD | RAIL | SEA | AIR => {
                    let i = usize::from(m);
                    let km = length * p.circuity[i];
                    q[i] = km * if m == ROAD || m == RAIL { phi } else { 1.0 };
                    q[Q_HOURS] = km / p.speed_kmh[i];
                }
                RAIL_ACCESS | PORT_ACCESS | MINOR_PORT_ACCESS | AIR_ACCESS => {
                    let fee = usize::from(m - RAIL_ACCESS);
                    let km = length * p.circuity[0];
                    q[0] = km;
                    q[Q_FEE] = p.fee[fee];
                    q[Q_HOURS] = km / p.speed_kmh[0] + p.fee_hours[fee];
                }
                CANAL => {
                    let km = length * p.circuity[2];
                    q[2] = km;
                    q[Q_FEE] = p.canal_fee;
                    q[Q_HOURS] = km / p.speed_kmh[2];
                }
                _ => {}
            }
            q
        })
        .collect()
}

/// Cost per tonne of path quantities: P04a-d times the km by mode, plus fees.
pub fn edge_cost(q: &Quantities, p: &Transport) -> f64 {
    let mut c = 0.0;
    for (q, per) in q.iter().zip(&p.per_tkm) {
        c += q * per;
    }
    c + q[Q_FEE]
}

/// The pack's transport graph priced at a [`Transport`], for least-cost trees with path
/// quantities.
pub struct Graph {
    n: usize,
    /// CSR of the cheapest edge per ordered node pair, columns ascending.
    indptr: Vec<usize>,
    target: Vec<usize>,
    weight: Vec<f64>,
    /// The quantities of each CSR entry's edge.
    q: Vec<Quantities>,
}

/// A heap entry ordered as the pair (-distance, node) under `<`: the smallest distance first,
/// the larger node first among equal distances.
#[derive(PartialEq)]
struct Entry(f64, usize);

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        (-self.0)
            .partial_cmp(&-other.0)
            .expect("distances are not NaN")
            .then(self.1.cmp(&other.1))
    }
}

impl Graph {
    pub fn new(nodes: usize, a: &[usize], b: &[usize], q: &[Quantities], p: &Transport) -> Graph {
        let cost: Vec<f64> = q.iter().map(|q| edge_cost(q, p)).collect();
        let mut arcs: Vec<(usize, usize, usize)> = a
            .iter()
            .zip(b)
            .enumerate()
            .map(|(e, (&a, &b))| (a, b, e))
            .chain(a.iter().zip(b).enumerate().map(|(e, (&a, &b))| (b, a, e)))
            .collect();
        // Stable: of parallel edges at equal cost, the first in that order.
        arcs.sort_by(|x, y| {
            (x.0, x.1)
                .cmp(&(y.0, y.1))
                .then(cost[x.2].total_cmp(&cost[y.2]))
        });
        arcs.dedup_by(|later, first| later.0 == first.0 && later.1 == first.1);
        let mut indptr = vec![0; nodes + 1];
        for &(u, _, _) in &arcs {
            indptr[u + 1] += 1;
        }
        for i in 0..nodes {
            indptr[i + 1] += indptr[i];
        }
        Graph {
            n: nodes,
            indptr,
            target: arcs.iter().map(|x| x.1).collect(),
            // A zero-cost edge keeps a negligible cost, as an explicit zero of a sparse graph
            // would be dropped.
            weight: arcs.iter().map(|x| cost[x.2].max(1e-300)).collect(),
            q: arcs.iter().map(|x| q[x.2]).collect(),
        }
    }

    pub fn nodes(&self) -> usize {
        self.n
    }

    /// Least cost from `source` to every node and the path quantities along each least-cost
    /// path; unreachable nodes have infinite cost and zero quantities. Ties between paths of
    /// equal cost go to the node scanned first, the scan taking the smallest distance and then
    /// the largest node index; the quantities are summed by pointer jumping, each pass doubling
    /// the stretch of path summed.
    pub fn tree(&self, source: usize) -> (Vec<f64>, Vec<Quantities>) {
        let n = self.n;
        let mut dist = vec![f64::INFINITY; n];
        let mut parent: Vec<usize> = (0..n).collect();
        let mut via = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        dist[source] = 0.0;
        heap.push(Entry(0.0, source));
        while let Some(Entry(d, v)) = heap.pop() {
            if dist[v] < d {
                continue;
            }
            for j in self.indptr[v]..self.indptr[v + 1] {
                let w = self.target[j];
                let next = d + self.weight[j];
                if dist[w] > next {
                    dist[w] = next;
                    heap.push(Entry(next, w));
                    parent[w] = v;
                    via[w] = j;
                }
            }
        }
        let mut acc: Vec<Quantities> = via
            .iter()
            .map(|&j| {
                if j == usize::MAX {
                    [0.0; N_Q]
                } else {
                    self.q[j]
                }
            })
            .collect();
        loop {
            let up: Vec<usize> = parent.iter().map(|&p| parent[p]).collect();
            let add: Vec<Quantities> = parent.iter().map(|&p| acc[p]).collect();
            for (a, b) in acc.iter_mut().zip(&add) {
                for k in 0..N_Q {
                    a[k] += b[k];
                }
            }
            if up == parent {
                return (dist, acc);
            }
            parent = up;
        }
    }
}

/// Mean S05 path quantities between countries per traded good, `q[((a * C + b) * T + g)]`
/// for exporter a and importer b, weighted by exporter province VA of the good times importer
/// province VA; `reach[a * C + b]` is false where no weighted province pair is connected.
#[derive(Clone, Debug, PartialEq)]
pub struct CountryCosts {
    pub n: usize,
    /// T, the traded goods.
    pub traded: usize,
    pub q: Vec<Quantities>,
    pub reach: Vec<bool>,
}

impl CountryCosts {
    pub fn at(&self, a: usize, b: usize, g: usize) -> &Quantities {
        &self.q[(a * self.n + b) * self.traded + g]
    }
}

/// The index of the first smallest of `v` (numpy's `argmin` without NaN).
fn argmin(v: impl IntoIterator<Item = f64>) -> (usize, f64) {
    let mut best = (0, f64::INFINITY);
    for (i, x) in v.into_iter().enumerate() {
        if i == 0 || x < best.1 {
            best = (i, x);
        }
    }
    best
}

/// The weighted mean of the pair quantities of one country pair: `w[o][d][g] = wo[o][g] *
/// (win[d] * ok[o][d])`; None when no weight is positive.
fn weighted_mean(
    wo: &Rows<f64>,
    win: &[f64],
    ok: &[Vec<bool>],
    q: &[Vec<Quantities>],
) -> Option<Vec<Quantities>> {
    let nt = wo.width();
    let mut num = vec![[0.0; N_Q]; nt];
    let mut den = vec![0.0; nt];
    let mut any = false;
    for (o, wo) in wo.iter().enumerate() {
        for (d, &win) in win.iter().enumerate() {
            let inner = win * f64::from(u8::from(ok[o][d]));
            for g in 0..nt {
                let w = wo[g] * inner;
                any |= w > 0.0;
                den[g] += w;
                for k in 0..N_Q {
                    num[g][k] += w * q[o][d][k];
                }
            }
        }
    }
    any.then(|| {
        let mut out = vec![[0.0; N_Q]; nt];
        for g in 0..nt {
            for k in 0..N_Q {
                out[g][k] = num[g][k] / den[g];
            }
        }
        out
    })
}

/// A province's least costs and path quantities to the provinces and gateways of its country.
struct Access {
    dom_cost: Vec<f64>,
    dom_q: Vec<Quantities>,
    acc_cost: Vec<f64>,
    acc_q: Vec<Quantities>,
}

/// [`CostInputs::country_costs`] of the pack at `pack`.
pub fn country_costs(
    pack: &Path,
    p: &Transport,
    trade: &TradeParams,
) -> Result<CountryCosts, String> {
    Ok(CostInputs::read(pack)?.country_costs(p, trade))
}

/// Path quantities at a bulk freight rate (P154 `sea`, P155 `port`; ADR 0247) as quantities at
/// the base rate, from the base ones `q` of edges of `mode`: the sea km of sea and canal edges
/// times `sea`, and the fees of port and minor port access times `port`, so that [`edge_cost`]
/// gives `ceteris_geo::graph::price` at `TransportParams::bulk(sea, port)`. Hours are unchanged.
pub fn bulk_quantities(mode: &[u8], q: &[Quantities], sea: f64, port: f64) -> Vec<Quantities> {
    mode.iter()
        .zip(q)
        .map(|(&m, &q)| {
            let mut b = q;
            match m {
                SEA | CANAL => b[usize::from(SEA)] *= sea,
                PORT_ACCESS | MINOR_PORT_ACCESS => b[Q_FEE] *= port,
                _ => {}
            }
            b
        })
        .collect()
}

/// The pack's tables that [`country_costs`] reads: the transport graph's edges, the gateways,
/// each province's country and its value added by good.
#[derive(Clone, Debug)]
pub struct CostInputs {
    pub nodes: usize,
    pub edge_a: Vec<usize>,
    pub edge_b: Vec<usize>,
    pub mode: Vec<u8>,
    pub length_km: Vec<f64>,
    pub tri_m: Vec<f64>,
    pub gateway_node: Vec<usize>,
    pub gateway_country: Vec<usize>,
    pub province_country: Vec<usize>,
    pub n_countries: usize,
    /// Goods `0..traded` are traded (`econ_goods`).
    pub traded: usize,
    /// Value added by good, per province.
    pub va: Rows<f64>,
}

impl CostInputs {
    pub fn read(pack: &Path) -> Result<CostInputs, String> {
        let read = |name: &str| Table::open(&pack.join(name));
        let (nodes, edges, gw, prov) = (
            read("nodes.arrow")?,
            read("edges.arrow")?,
            read("gateways.arrow")?,
            read("econ_provinces.arrow")?,
        );
        let countries = read("countries.arrow")?.strings("code")?;
        let index: BTreeMap<&str, usize> = countries
            .iter()
            .enumerate()
            .map(|(i, c)| (c.as_str(), i))
            .collect();
        let province_country: Vec<usize> = read("provinces.arrow")?
            .strings("country")?
            .iter()
            .map(|c| {
                index
                    .get(c.as_str())
                    .copied()
                    .ok_or_else(|| format!("province country {c} is not a pack country"))
            })
            .collect::<Result<_, _>>()?;
        let is_traded = read("econ_goods.arrow")?.bool("traded")?;
        let traded = is_traded.iter().take_while(|&&t| t).count();
        let va_cols = (0..is_traded.len())
            .map(|g| prov.f64(&format!("va_{g}")))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CostInputs {
            nodes: nodes.rows(),
            edge_a: edges.usize("a")?,
            edge_b: edges.usize("b")?,
            mode: edges.usize("mode")?.iter().map(|&m| m as u8).collect(),
            length_km: edges.f64("length_km")?,
            tri_m: edges.f64("tri_m")?,
            gateway_node: gw.usize("node")?,
            gateway_country: gw.usize("country")?,
            va: Rows::from_flat(
                (0..province_country.len())
                    .flat_map(|s| va_cols.iter().map(move |c| c[s]))
                    .collect(),
                va_cols.len(),
            ),
            province_country,
            n_countries: countries.len(),
            traded,
        })
    }

    /// S05 costs between and within the countries as the trade kernel builds them
    /// (`EconInputs::network_from_graph`): domestic pairs by the least-cost path between
    /// provinces, and pairs of different countries by the cheapest chain of an access leg, a
    /// crossing leg between gateways and an access leg, over every gateway pair. Inside a
    /// province trade is costless (A2). Where `trade` gives theta classes a bulk freight rate
    /// (P154, P155, ADR 0247), the goods of those classes take the same chains on the graph
    /// repriced at that rate, as the kernel's bulk legs; their path quantities are then
    /// [`bulk_quantities`], so that [`edge_cost`] at `p` prices them at the rate. The rate
    /// changes no edge's reachability, so `reach` is the same for every good.
    pub fn country_costs(&self, p: &Transport, trade: &TradeParams) -> CountryCosts {
        let q_edge = edge_quantities(&self.mode, &self.length_km, &self.tri_m, p);
        let (mut q, reach) = self.means(&q_edge, p);
        if let Some((classes, sea, port)) = trade.bulk_rate() {
            let (bulk, _) = self.means(&bulk_quantities(&self.mode, &q_edge, sea, port), p);
            let nt = self.traded;
            for g in (0..nt).filter(|&g| classes[usize::from(trade.theta_class[g])]) {
                for cell in 0..self.n_countries * self.n_countries {
                    q[cell * nt + g] = bulk[cell * nt + g];
                }
            }
        }
        CountryCosts {
            n: self.n_countries,
            traded: self.traded,
            q,
            reach,
        }
    }

    /// The mean path quantities of every country pair and good, and whether the pair is
    /// reached, on the graph whose edges have quantities `q_edge`.
    fn means(&self, q_edge: &[Quantities], p: &Transport) -> (Vec<Quantities>, Vec<bool>) {
        let graph = Graph::new(self.nodes, &self.edge_a, &self.edge_b, q_edge, p);
        let (country, gnode, gcountry, va) = (
            &self.province_country,
            &self.gateway_node,
            &self.gateway_country,
            &self.va,
        );
        let (np, nc, nt) = (country.len(), self.n_countries, self.traded);
        let provinces_of: Vec<Vec<usize>> = (0..nc)
            .map(|c| (0..np).filter(|&s| country[s] == c).collect())
            .collect();
        let gateways_of: Vec<Vec<usize>> = (0..nc)
            .map(|c| (0..gnode.len()).filter(|&i| gcountry[i] == c).collect())
            .collect();

        let access: Vec<Access> = (0..np)
            .into_par_iter()
            .map(|s| {
                let (dist, q) = graph.tree(s);
                let c = country[s];
                let at = |nodes: &mut dyn Iterator<Item = usize>| -> (Vec<f64>, Vec<Quantities>) {
                    nodes.map(|n| (dist[n], q[n])).unzip()
                };
                let (dom_cost, dom_q) = at(&mut provinces_of[c].iter().copied());
                let (acc_cost, acc_q) = at(&mut gateways_of[c].iter().map(|&g| gnode[g]));
                Access {
                    dom_cost,
                    dom_q,
                    acc_cost,
                    acc_q,
                }
            })
            .collect();
        let crossing: Vec<(Vec<f64>, Vec<Quantities>)> = gnode
            .par_iter()
            .map(|&s| {
                let (dist, q) = graph.tree(s);
                gnode.iter().map(|&n| (dist[n], q[n])).unzip()
            })
            .collect();

        // Exporter: the good's VA; importer: all VA.
        let weight_out = Rows::from_rows(nt, va.iter().map(|v| &v[..nt]));
        let weight_in: Vec<f64> = va.iter().map(pairwise_sum).collect();
        let mut out = vec![[0.0; N_Q]; nc * nc * nt];
        let mut reach = vec![false; nc * nc];
        let mut put = |a: usize, b: usize, mean: Vec<Quantities>| {
            for (g, q) in mean.into_iter().enumerate() {
                out[(a * nc + b) * nt + g] = q;
            }
            reach[a * nc + b] = true;
        };
        for a in 0..nc {
            let pa = &provinces_of[a];
            let win_a: Vec<f64> = pa.iter().map(|&s| weight_in[s]).collect();
            if pa.is_empty() || !win_a.iter().any(|&w| w > 0.0) {
                continue;
            }
            // A good the country does not make: all VA.
            let makes: Vec<bool> = (0..nt)
                .map(|g| pa.iter().any(|&s| weight_out[s][g] > 0.0))
                .collect();
            let wo = Rows::from_rows(
                nt,
                pa.iter().map(|&s| {
                    (0..nt)
                        .map(|g| {
                            if makes[g] {
                                weight_out[s][g]
                            } else {
                                weight_in[s]
                            }
                        })
                        .collect::<Vec<_>>()
                }),
            );
            let ok: Vec<Vec<bool>> = pa
                .iter()
                .map(|&s| access[s].dom_cost.iter().map(|c| c.is_finite()).collect())
                .collect();
            let q: Vec<Vec<Quantities>> = pa.iter().map(|&s| access[s].dom_q.clone()).collect();
            if let Some(mean) = weighted_mean(&wo, &win_a, &ok, &q) {
                put(a, a, mean);
            }
            let ga = &gateways_of[a];
            if ga.is_empty() {
                continue;
            }
            // The cheapest first two legs to every gateway j: over a's gateways i, access to i
            // plus the crossing from i to j.
            let (first, u): (Vec<Vec<usize>>, Vec<Vec<f64>>) = pa
                .iter()
                .map(|&s| {
                    (0..gnode.len())
                        .map(|j| {
                            argmin(
                                ga.iter()
                                    .zip(&access[s].acc_cost)
                                    .map(|(&i, c)| c + crossing[i].0[j]),
                            )
                        })
                        .unzip()
                })
                .unzip();
            for b in 0..nc {
                let (pb, gb) = (&provinces_of[b], &gateways_of[b]);
                if b == a
                    || pb.is_empty()
                    || gb.is_empty()
                    || !pb.iter().any(|&s| weight_in[s] > 0.0)
                {
                    continue;
                }
                let mut ok = vec![vec![false; pb.len()]; pa.len()];
                let mut q = vec![vec![[0.0; N_Q]; pb.len()]; pa.len()];
                for o in 0..pa.len() {
                    let qa = &access[pa[o]].acc_q;
                    for (d, &t) in pb.iter().enumerate() {
                        let cb = &access[t].acc_cost;
                        let (last, best) = argmin(gb.iter().zip(cb).map(|(&j, c)| u[o][j] + c));
                        if !best.is_finite() {
                            continue;
                        }
                        ok[o][d] = true;
                        let ai = first[o][gb[last]];
                        let x = &crossing[ga[ai]].1[gb[last]];
                        let qb = &access[t].acc_q[last];
                        q[o][d] = std::array::from_fn(|k| qa[ai][k] + x[k] + qb[k]);
                    }
                }
                if !ok.iter().flatten().any(|&x| x) {
                    continue;
                }
                let win_b: Vec<f64> = pb.iter().map(|&s| weight_in[s]).collect();
                if let Some(mean) = weighted_mean(&wo, &win_b, &ok, &q) {
                    put(a, b, mean);
                }
            }
        }
        (out, reach)
    }
}

/// S05 iceberg factor 1 + c w_g + lambda_g T (trade SPEC, Leg factors) of path quantities.
pub fn tau(q: &Quantities, weight: f64, time_cost: f64, p: &Transport) -> f64 {
    1.0 + edge_cost(q, p) * weight + time_cost * q[Q_HOURS] / 24.0
}
