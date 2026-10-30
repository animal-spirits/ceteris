//! S05 least-cost costs on the pack's transport graph: edge prices, least-cost trees with path
//! quantities and the mean costs between countries the trade kernel builds.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};
use std::path::Path;

use ceteris_reference::io::pairwise_sum;
use rayon::prelude::*;

use crate::write::Table;

pub const N_TRADED: usize = 7;

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

/// Mean S05 path quantities between countries per traded good, `q[((a * C + b) * N_TRADED + g)]`
/// for exporter a and importer b, weighted by exporter province VA of the good times importer
/// province VA; `reach[a * C + b]` is false where no weighted province pair is connected.
#[derive(Clone, Debug, PartialEq)]
pub struct CountryCosts {
    pub n: usize,
    pub q: Vec<Quantities>,
    pub reach: Vec<bool>,
}

impl CountryCosts {
    pub fn at(&self, a: usize, b: usize, g: usize) -> &Quantities {
        &self.q[(a * self.n + b) * N_TRADED + g]
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
    wo: &[[f64; N_TRADED]],
    win: &[f64],
    ok: &[Vec<bool>],
    q: &[Vec<Quantities>],
) -> Option<[Quantities; N_TRADED]> {
    let mut num = [[0.0; N_Q]; N_TRADED];
    let mut den = [0.0; N_TRADED];
    let mut any = false;
    for (o, wo) in wo.iter().enumerate() {
        for (d, &win) in win.iter().enumerate() {
            let inner = win * f64::from(u8::from(ok[o][d]));
            for g in 0..N_TRADED {
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
        let mut out = [[0.0; N_Q]; N_TRADED];
        for g in 0..N_TRADED {
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

/// S05 costs between and within the pack's countries as the trade kernel builds them
/// (`EconInputs::network_from_graph`): domestic pairs by the least-cost path between provinces,
/// and pairs of different countries by the cheapest chain of an access leg, a crossing leg
/// between gateways and an access leg, over every gateway pair. Inside a province trade is
/// costless (A2). These are the base legs: the bulk legs of a bulk freight rate (P154, P155,
/// ADR 0247) are not built here, so for a bulk class the costs match the kernel only while the
/// rate is off (the default).
pub fn country_costs(pack: &Path, p: &Transport) -> Result<CountryCosts, String> {
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
    let country: Vec<usize> = read("provinces.arrow")?
        .strings("country")?
        .iter()
        .map(|c| {
            index
                .get(c.as_str())
                .copied()
                .ok_or_else(|| format!("province country {c} is not a pack country"))
        })
        .collect::<Result<_, _>>()?;
    let (np, nc) = (country.len(), countries.len());
    let va_cols = (0..10)
        .map(|g| prov.f64(&format!("va_{g}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mode: Vec<u8> = edges.usize("mode")?.iter().map(|&m| m as u8).collect();
    let q_edge = edge_quantities(&mode, &edges.f64("length_km")?, &edges.f64("tri_m")?, p);
    let graph = Graph::new(
        nodes.rows(),
        &edges.usize("a")?,
        &edges.usize("b")?,
        &q_edge,
        p,
    );
    let gnode = gw.usize("node")?;
    let gcountry = gw.usize("country")?;
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
    let weight_out: Vec<[f64; N_TRADED]> = (0..np)
        .map(|s| std::array::from_fn(|g| va_cols[g][s]))
        .collect();
    let weight_in: Vec<f64> = (0..np)
        .map(|s| pairwise_sum(&(0..10).map(|g| va_cols[g][s]).collect::<Vec<_>>()))
        .collect();
    let mut out = vec![[0.0; N_Q]; nc * nc * N_TRADED];
    let mut reach = vec![false; nc * nc];
    let mut put = |a: usize, b: usize, mean: [Quantities; N_TRADED]| {
        for (g, q) in mean.into_iter().enumerate() {
            out[(a * nc + b) * N_TRADED + g] = q;
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
        let makes: Vec<bool> = (0..N_TRADED)
            .map(|g| pa.iter().any(|&s| weight_out[s][g] > 0.0))
            .collect();
        let wo: Vec<[f64; N_TRADED]> = pa
            .iter()
            .map(|&s| {
                std::array::from_fn(|g| {
                    if makes[g] {
                        weight_out[s][g]
                    } else {
                        weight_in[s]
                    }
                })
            })
            .collect();
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
            if b == a || pb.is_empty() || gb.is_empty() || !pb.iter().any(|&s| weight_in[s] > 0.0) {
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
    Ok(CountryCosts {
        n: nc,
        q: out,
        reach,
    })
}

/// S05 iceberg factor 1 + c w_g + lambda_g T (trade SPEC, Leg factors) of path quantities.
pub fn tau(q: &Quantities, weight: f64, time_cost: f64, p: &Transport) -> f64 {
    1.0 + edge_cost(q, p) * weight + time_cost * q[Q_HOURS] / 24.0
}
