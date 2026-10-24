//! Route-choice trade costs (ADR 0017; Allen and Arkolakis 2022). With Frechet route shocks of
//! dispersion theta_r, the cost of reaching d from s aggregates every walk on the graph:
//! `tau_sd^-theta_r = z_sd = [(I - A)^-1]_sd` with `a_kl = t_kl^-theta_r` edge by edge. The
//! inverse is never formed. See `specs/geo/SPEC.md`, Route choice.
//!
//! z spans hundreds of decades across the globe (`t^-theta_r` per edge, compounded over
//! hundreds of edges), so a residual norm, as conjugate gradient minimizes, says nothing about
//! far entries. Each solve instead scales z by the least-cost walk, `z_k = exp(-D_k) u_k` with D
//! the Dijkstra distance under edge weights `theta_r ln t`, and runs symmetric Gauss-Seidel on u
//! in Dijkstra order from `u = 1` (the least-cost walk alone, a lower bound), to a relative
//! change of 1e-12 in every entry. The scaled weights `exp(-(w_kl + D_l - D_k))` lie in [0, 1]
//! (triangle inequality), and `tau = exp((D - ln u) / theta_r)` never underflows.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::OnceLock;

use rayon::prelude::*;

use crate::graph::TransportGraph;

const RADIUS_MAX_ITER: usize = 10_000;
const RADIUS_TOL: f64 = 1e-10;
const SOLVE_TOL: f64 = 1e-12;
const SOLVE_MAX_SWEEPS: usize = 100_000;

/// Edge factors of one good as arcs in both directions: `w_kl = theta_r ln t_kl = -ln a_kl`
/// (infinite on closed edges) and hours.
#[derive(Debug)]
pub struct EdgeFactors {
    pub theta_r: f64,
    offsets: Vec<u32>,
    arc_to: Vec<u32>,
    arc_w: Vec<f64>,
    arc_h: Vec<f64>,
    radius: OnceLock<f64>,
}

/// One row of `(I - A)^-1`, its route-choice factors and the route-probability-weighted hours
/// to each node, indexed by node, or like `nodes` for a subgraph solve. Unreachable nodes have
/// `z = 0` and infinite `tau` and `hours`; `z` may also underflow to 0 where `tau` is finite.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteTau {
    pub z: Vec<f64>,
    /// `tau_sd = z_d^(-1/theta_r)`, from logs.
    pub tau: Vec<f64>,
    pub hours: Vec<f64>,
}

impl EdgeFactors {
    /// `t_kl = 1 + c_kl weight + time_cost_per_day hours_kl / 24` from the graph's edge prices
    /// (`graph::price`), `a_kl = t_kl^-theta_r`; a closed edge has `a = 0`.
    pub fn new(g: &TransportGraph, weight: f64, time_cost_per_day: f64, theta_r: f64) -> Self {
        assert!(theta_r > 0.0, "theta_r must be positive, got {theta_r}");
        let factor = |e: usize| {
            let (c, h) = (g.cost[e], g.hours[e]);
            if !(c.is_finite() && h.is_finite()) {
                return (f64::INFINITY, 0.0);
            }
            let t = 1.0 + c * weight + time_cost_per_day * h / 24.0;
            (theta_r * libm::log(t), h)
        };
        let edge: Vec<(f64, f64)> = (0..g.edges()).map(factor).collect();
        let n = g.nodes();
        let mut offsets = Vec::with_capacity(n + 1);
        let (mut arc_to, mut arc_w, mut arc_h) = (Vec::new(), Vec::new(), Vec::new());
        offsets.push(0);
        for k in 0..n as u32 {
            for (l, e) in g.arcs(k) {
                let (w, h) = edge[e as usize];
                arc_to.push(l);
                arc_w.push(w);
                arc_h.push(h);
            }
            offsets.push(arc_to.len() as u32);
        }
        Self {
            theta_r,
            offsets,
            arc_to,
            arc_w,
            arc_h,
            radius: OnceLock::new(),
        }
    }

    pub fn nodes(&self) -> usize {
        self.offsets.len() - 1
    }

    fn arcs(&self, k: usize) -> std::ops::Range<usize> {
        self.offsets[k] as usize..self.offsets[k + 1] as usize
    }

    /// Spectral radius of A by power iteration from the all-ones vector: the norm ratio
    /// `||A x|| / ||x||`, which for a symmetric non-negative A rises to the radius even when
    /// `-rho` is also an eigenvalue. At most 10,000 iterations, stopping at a relative change of
    /// 1e-10. Computed once; rows in parallel, each summed in arc order.
    pub fn spectral_radius(&self) -> f64 {
        *self.radius.get_or_init(|| {
            let n = self.nodes();
            let a: Vec<f64> = self.arc_w.iter().map(|&w| libm::exp(-w)).collect();
            let mut x = vec![1.0 / (n as f64).sqrt(); n];
            let mut y = vec![0.0; n];
            let mut rho = 0.0;
            for _ in 0..RADIUS_MAX_ITER {
                y.par_iter_mut().enumerate().for_each(|(k, yk)| {
                    *yk = self
                        .arcs(k)
                        .map(|j| a[j] * x[self.arc_to[j] as usize])
                        .sum();
                });
                let norm = y.iter().map(|v| v * v).sum::<f64>().sqrt();
                if norm == 0.0 {
                    return 0.0;
                }
                x.iter_mut().zip(&y).for_each(|(xk, yk)| *xk = yk / norm);
                let done = (norm - rho).abs() <= RADIUS_TOL * norm;
                rho = norm;
                if done {
                    break;
                }
            }
            rho
        })
    }
}

/// Arcs of A restricted to a node set, in local indices.
struct Local {
    offsets: Vec<u32>,
    arc_to: Vec<u32>,
    arc_w: Vec<f64>,
    arc_h: Vec<f64>,
}

impl Local {
    fn new(f: &EdgeFactors, nodes: &[u32]) -> Self {
        let mut local = vec![u32::MAX; f.nodes()];
        for (i, &k) in nodes.iter().enumerate() {
            assert_eq!(local[k as usize], u32::MAX, "node {k} listed twice");
            local[k as usize] = i as u32;
        }
        let mut s = Self {
            offsets: vec![0],
            arc_to: Vec::new(),
            arc_w: Vec::new(),
            arc_h: Vec::new(),
        };
        for &k in nodes {
            for j in f.arcs(k as usize) {
                let l = local[f.arc_to[j] as usize];
                if l != u32::MAX {
                    s.arc_to.push(l);
                    s.arc_w.push(f.arc_w[j]);
                    s.arc_h.push(f.arc_h[j]);
                }
            }
            s.offsets.push(s.arc_to.len() as u32);
        }
        s
    }
}

/// CSR arcs as slices, whole graph or local.
struct Arcs<'a> {
    offsets: &'a [u32],
    to: &'a [u32],
    w: &'a [f64],
    h: &'a [f64],
}

impl Arcs<'_> {
    fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    fn of(&self, k: usize) -> std::ops::Range<usize> {
        self.offsets[k] as usize..self.offsets[k + 1] as usize
    }

    /// Distances under the weights w from `s` and the settle order; ties pop the lower node.
    fn dijkstra(&self, s: usize) -> (Vec<f64>, Vec<u32>) {
        let mut dist = vec![f64::INFINITY; self.len()];
        let mut done = vec![false; self.len()];
        let mut order = Vec::new();
        let mut heap = BinaryHeap::new();
        dist[s] = 0.0;
        heap.push(Reverse((0f64.to_bits(), s as u32)));
        while let Some(Reverse((_, k))) = heap.pop() {
            if std::mem::replace(&mut done[k as usize], true) {
                continue;
            }
            order.push(k);
            let dk = dist[k as usize];
            for j in self.of(k as usize) {
                let (l, d) = (self.to[j] as usize, dk + self.w[j]);
                if d < dist[l] {
                    dist[l] = d;
                    heap.push(Reverse((d.to_bits(), l as u32)));
                }
            }
        }
        (dist, order)
    }
}

/// Symmetric Gauss-Seidel on `x_k = rhs_k + sum_j b_j x_l` over `order` until no entry moves by
/// more than 1e-12 relative.
fn gauss_seidel(arcs: &Arcs, b: &[f64], order: &[u32], rhs: &[f64], x: &mut [f64]) {
    for _ in 0..SOLVE_MAX_SWEEPS {
        let mut change: f64 = 0.0;
        for &k in order.iter().chain(order.iter().rev()) {
            let k = k as usize;
            let new = rhs[k]
                + arcs
                    .of(k)
                    .map(|j| b[j] * x[arcs.to[j] as usize])
                    .sum::<f64>();
            if new > 0.0 {
                change = change.max((new - x[k]).abs() / new);
            }
            x[k] = new;
        }
        if change <= SOLVE_TOL {
            return;
        }
    }
    panic!("route-choice Gauss-Seidel did not reach {SOLVE_TOL} in {SOLVE_MAX_SWEEPS} sweeps");
}

/// Row `source` of `(I - A)^-1`, on the whole graph or on the subgraph `nodes` (which must
/// contain `source`; results are then indexed like `nodes`), with `tau_sd = z_d^(-1/theta_r)`
/// and the expected route hours `T_sd = [(I - A)^-1 (A o H) z]_d / z_d`. `warm` is a starting
/// guess for `z`, such as the last solve before an edge change. Panics, reporting the radius,
/// when the spectral radius of A is 1 or more: the series over walks diverges.
pub fn solve_from(
    f: &EdgeFactors,
    source: u32,
    nodes: Option<&[u32]>,
    warm: Option<&[f64]>,
) -> RouteTau {
    let rho = f.spectral_radius();
    assert!(
        rho < 1.0,
        "route-choice spectral radius {rho} >= 1 at theta_r {}: raise theta_r or coarsen edges",
        f.theta_r
    );
    let local = nodes.map(|list| Local::new(f, list));
    let arcs = match &local {
        Some(m) => Arcs {
            offsets: &m.offsets,
            to: &m.arc_to,
            w: &m.arc_w,
            h: &m.arc_h,
        },
        None => Arcs {
            offsets: &f.offsets,
            to: &f.arc_to,
            w: &f.arc_w,
            h: &f.arc_h,
        },
    };
    let s = match nodes {
        Some(list) => list
            .iter()
            .position(|&k| k == source)
            .unwrap_or_else(|| panic!("source {source} not in the subgraph")),
        None => source as usize,
    };
    let n = arcs.len();
    let (dist, order) = arcs.dijkstra(s);
    let mut b = vec![0.0; arcs.to.len()];
    for k in 0..n {
        if dist[k].is_finite() {
            for j in arcs.of(k) {
                let reduced = arcs.w[j] + dist[arcs.to[j] as usize] - dist[k];
                b[j] = if reduced.is_finite() {
                    libm::exp(-reduced.max(0.0))
                } else {
                    0.0
                };
            }
        }
    }
    let mut u: Vec<f64> = (0..n)
        .map(|k| match (dist[k].is_finite(), warm) {
            (false, _) => 0.0,
            (true, None) => 1.0,
            (true, Some(z)) => {
                let x = z[k] * libm::exp(dist[k]);
                if x.is_finite() { x.max(1.0) } else { 1.0 }
            }
        })
        .collect();
    if let Some(z) = warm {
        assert_eq!(z.len(), n, "warm start has the wrong length");
    }
    let mut rhs = vec![0.0; n];
    rhs[s] = 1.0;
    gauss_seidel(&arcs, &b, &order, &rhs, &mut u);
    for &k in &order {
        let k = k as usize;
        rhs[k] = arcs
            .of(k)
            .map(|j| b[j] * arcs.h[j] * u[arcs.to[j] as usize])
            .sum();
    }
    let mut v = vec![0.0; n];
    gauss_seidel(&arcs, &b, &order, &rhs, &mut v);
    let reached = |k: usize| dist[k].is_finite() && u[k] > 0.0;
    RouteTau {
        z: (0..n)
            .map(|k| {
                if reached(k) {
                    u[k] * libm::exp(-dist[k])
                } else {
                    0.0
                }
            })
            .collect(),
        tau: (0..n)
            .map(|k| {
                if reached(k) {
                    libm::exp((dist[k] - libm::log(u[k])) / f.theta_r)
                } else {
                    f64::INFINITY
                }
            })
            .collect(),
        hours: (0..n)
            .map(|k| {
                if reached(k) {
                    v[k] / u[k]
                } else {
                    f64::INFINITY
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeList, Mode, NodeKind, NodeList, TransportParams};
    use crate::paths::shortest;

    /// A graph on `n` nodes with the given (a, b, cost per tonne, hours) edges.
    fn graph(n: usize, edges: &[(u32, u32, f64, f64)]) -> TransportGraph {
        let nodes = NodeList {
            kind: vec![NodeKind::Province; n],
            province: (0..n as u32).collect(),
            lon: vec![0.0; n],
            lat: vec![0.0; n],
        };
        let list = EdgeList {
            a: edges.iter().map(|e| e.0).collect(),
            b: edges.iter().map(|e| e.1).collect(),
            mode: vec![Mode::Sea; edges.len()],
            length_km: vec![1.0; edges.len()],
            tri_m: vec![0.0; edges.len()],
            chokepoint: vec![0; edges.len()],
        };
        let mut g = TransportGraph::new(n, nodes, list, &TransportParams::default());
        g.cost = edges.iter().map(|e| e.2).collect();
        g.hours = edges.iter().map(|e| e.3).collect();
        g
    }

    const W: f64 = 0.01;

    /// Two routes from node 0 to node 5: 0-1-2-5 at 20 USD/t per edge (t = 1.2), the first
    /// edge scaled by `first`, and 0-3-4-5 at 25 (t = 1.25).
    fn two_routes(first: f64) -> TransportGraph {
        graph(
            6,
            &[
                (0, 1, 20.0 * first, 10.0),
                (1, 2, 20.0, 10.0),
                (2, 5, 20.0, 10.0),
                (0, 3, 25.0, 20.0),
                (3, 4, 25.0, 20.0),
                (4, 5, 25.0, 20.0),
            ],
        )
    }

    fn tau_05(g: &TransportGraph, theta_r: f64) -> f64 {
        let f = EdgeFactors::new(g, W, 0.0, theta_r);
        solve_from(&f, 0, None, None).tau[5]
    }

    #[test]
    fn converges_to_least_cost_as_theta_r_grows() {
        let g = two_routes(1.0);
        // the cheaper route by Dijkstra, priced as the product of its edge factors
        assert_eq!(shortest(&g, 0).cost[5], 60.0);
        let least = libm::pow(1.2, 3.0);
        let taus: Vec<f64> = [10.0, 100.0, 1000.0]
            .iter()
            .map(|&th| tau_05(&g, th))
            .collect();
        assert!(taus.windows(2).all(|w| w[0] < w[1]), "{taus:?}");
        assert!(taus.iter().all(|&t| t <= least), "{taus:?} vs {least}");
        let gap = least / taus[2] - 1.0;
        assert!(gap < 1e-3, "gap {gap} at theta_r 1000");
    }

    #[test]
    fn derivative_matches_finite_difference() {
        let theta_r = 8.0;
        let (e, k, l, d) = (1usize, 1u32, 2u32, 5usize);
        let g = two_routes(1.0);
        let f = EdgeFactors::new(&g, W, 0.0, theta_r);
        let zs = solve_from(&f, 0, None, None);
        let zk = solve_from(&f, k, None, None).z;
        let zl = solve_from(&f, l, None, None).z;
        // d z_sd / d a_kl over both directions of the edge, then the chain rule
        let dz_da = zs.z[k as usize] * zl[d] + zs.z[l as usize] * zk[d];
        let t = 1.0 + g.cost[e] * W;
        let da_dc = -theta_r * libm::pow(t, -theta_r - 1.0) * W;
        let analytic = -zs.tau[d] / (theta_r * zs.z[d]) * dz_da * da_dc;

        let h = 1e-6;
        let at = |c: f64| {
            let mut g = g.clone();
            g.cost[e] = c;
            tau_05(&g, theta_r)
        };
        let numeric = (at(g.cost[e] + h) - at(g.cost[e] - h)) / (2.0 * h);
        assert!(analytic > 0.0);
        let rel = (numeric - analytic).abs() / analytic;
        assert!(rel < 1e-6, "{numeric} vs {analytic}: {rel}");
    }

    #[test]
    fn closing_an_edge_is_continuous() {
        // routes of about equal cost, 0-1-2-5 at 5, 20, 20 USD/t and 0-3-4-5 at 15 per edge;
        // edge 0-1 is scaled up to closure
        let theta_r = 20.0;
        let routes = |first: f64| {
            graph(
                6,
                &[
                    (0, 1, 5.0 * first, 10.0),
                    (1, 2, 20.0, 10.0),
                    (2, 5, 20.0, 10.0),
                    (0, 3, 15.0, 20.0),
                    (3, 4, 15.0, 20.0),
                    (4, 5, 15.0, 20.0),
                ],
            )
        };
        let taus: Vec<f64> = (0..20)
            .map(|i| tau_05(&routes(libm::pow(1e6, f64::from(i) / 19.0)), theta_r))
            .collect();
        let closed = tau_05(&routes(f64::INFINITY), theta_r);
        for w in taus.windows(2) {
            assert!(w[1] >= w[0], "{taus:?}");
            assert!(
                w[1] / w[0] - 1.0 <= 0.05,
                "jump {} -> {}: {taus:?}",
                w[0],
                w[1]
            );
        }
        assert!(
            closed >= taus[19] && closed / taus[19] - 1.0 < 1e-6,
            "{closed}"
        );
    }

    #[test]
    fn two_parallel_routes_add_access() {
        // routes 0-1-3 and 0-2-3 with x = t1^-theta_r and y = t2^-theta_r per edge: with
        // q = x^2 + y^2, z_03 = q / (1 - 2 q) over all walks
        let theta_r = 6.0;
        let (c1, c2) = (20.0, 30.0);
        let both = graph(
            4,
            &[
                (0, 1, c1, 1.0),
                (1, 3, c1, 1.0),
                (0, 2, c2, 1.0),
                (2, 3, c2, 1.0),
            ],
        );
        let f = EdgeFactors::new(&both, W, 0.0, theta_r);
        let r = solve_from(&f, 0, None, None);
        let x = libm::pow(1.0 + c1 * W, -theta_r);
        let y = libm::pow(1.0 + c2 * W, -theta_r);
        let q = x * x + y * y;
        let z = q / (1.0 - 2.0 * q);
        assert!((r.z[3] - z).abs() <= 1e-12 * z, "{} vs {z}", r.z[3]);
        assert!((r.tau[3] - libm::pow(z, -1.0 / theta_r)).abs() <= 1e-12);
        let alone = |c: f64| {
            let g = graph(3, &[(0, 1, c, 1.0), (1, 2, c, 1.0)]);
            let f = EdgeFactors::new(&g, W, 0.0, theta_r);
            solve_from(&f, 0, None, None).tau[2]
        };
        assert!(r.tau[3] < alone(c1) && alone(c1) < alone(c2));
    }

    #[test]
    fn expected_hours_count_every_walk() {
        // path 0-1-2, hours h per edge: walks of 2k + 2 edges with weight 2^k x^(2k+2), so
        // the expected hours are 2 h / (1 - 2 x^2)
        let (theta_r, c, h) = (5.0, 30.0, 7.0);
        let g = graph(3, &[(0, 1, c, h), (1, 2, c, h)]);
        let f = EdgeFactors::new(&g, W, 0.0, theta_r);
        let x = libm::pow(1.0 + c * W, -theta_r);
        let got = solve_from(&f, 0, None, None).hours[2];
        let want = 2.0 * h / (1.0 - 2.0 * x * x);
        assert!((got - want).abs() <= 1e-10 * want, "{got} vs {want}");
    }

    #[test]
    fn time_cost_enters_the_edge_factor() {
        let g = graph(2, &[(0, 1, 50.0, 48.0)]);
        let f = EdgeFactors::new(&g, 0.002, 0.01, 4.0);
        // 2 nodes: rho = a and z_01 = a / (1 - a^2)
        let a = libm::pow(1.0 + 50.0 * 0.002 + 0.01 * 2.0, -4.0);
        assert!((f.spectral_radius() - a).abs() < 1e-12);
        let z = solve_from(&f, 0, None, None).z[1];
        assert!((z / (a / (1.0 - a * a)) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn far_entries_keep_their_precision() {
        // a 200-edge path at t = 3 and theta_r 20: z at the far end is about 1e-1908, far
        // below the f64 range; walks that turn back weigh a^2 = 3^-40 each, so tau is the
        // product of the edge factors
        let n = 201;
        let edges: Vec<_> = (0..200).map(|i| (i, i + 1, 200.0, 1.0)).collect();
        let g = graph(n, &edges);
        let f = EdgeFactors::new(&g, W, 0.0, 20.0);
        let r = solve_from(&f, 0, None, None);
        assert_eq!(r.z[200], 0.0);
        let want = libm::pow(3.0, 200.0);
        assert!(
            (r.tau[200] / want - 1.0).abs() < 1e-10,
            "{} vs {want}",
            r.tau[200]
        );
        assert!((r.hours[200] - 200.0).abs() < 1e-6);
    }

    #[test]
    fn subgraph_solve_ignores_outside_nodes() {
        let g = two_routes(1.0);
        let f = EdgeFactors::new(&g, W, 0.0, 6.0);
        let nodes = [5, 0, 1, 2];
        let sub = solve_from(&f, 0, Some(&nodes), None);
        let path = graph(
            4,
            &[(0, 1, 20.0, 10.0), (1, 2, 20.0, 10.0), (2, 3, 20.0, 10.0)],
        );
        let fp = EdgeFactors::new(&path, W, 0.0, 6.0);
        let full = solve_from(&fp, 0, None, None);
        assert!((sub.z[0] - full.z[3]).abs() <= 1e-12 * full.z[3]);
        assert!((sub.hours[0] - full.hours[3]).abs() <= 1e-10 * full.hours[3]);
        // a warm start from the answer gives the same values
        let warm = solve_from(&f, 0, Some(&nodes), Some(&sub.z));
        assert!((warm.z[0] - sub.z[0]).abs() <= 1e-12 * sub.z[0]);
    }

    #[test]
    #[should_panic(expected = "spectral radius")]
    fn radius_above_one_panics() {
        // t close to 1 at a small theta_r: a about 0.99 on a cycle of degree 2
        let g = two_routes(1.0);
        let f = EdgeFactors::new(&g, 0.0002, 0.0, 2.0);
        assert!(f.spectral_radius() > 1.0);
        solve_from(&f, 0, None, None);
    }
}
