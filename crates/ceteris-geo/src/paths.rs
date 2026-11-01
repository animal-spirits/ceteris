//! Least-cost paths (Section 5.4, physical part) and a cache recomputed only when edge
//! costs change (Section 19.1).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

use rayon::prelude::*;

use crate::graph::TransportGraph;

/// Least cost per tonne from one source to every node, and the hours along that path.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub cost: Vec<f64>,
    pub hours: Vec<f64>,
}

impl Tree {
    /// Costs then hours for the first `provinces` nodes as little-endian `f32`: the map's wire format.
    pub fn province_bytes(&self, provinces: usize) -> Vec<u8> {
        let values = self.cost[..provinces]
            .iter()
            .chain(&self.hours[..provinces]);
        values.flat_map(|&x| (x as f32).to_le_bytes()).collect()
    }
}

/// Dijkstra from `source`. Heap keys are the bit patterns of non-negative costs, which
/// order like the costs; ties pop the lower node index first, so results are deterministic.
pub fn shortest(g: &TransportGraph, source: u32) -> Tree {
    shortest_by(g, source, 0.0)
}

/// Dijkstra from `source` on the edge weight `c_e + kappa h_e`, a good's iceberg cost per tonne
/// over its weight per value with `kappa = lambda_g / (24 w_g)` (ADR 0279): the tree's costs and
/// hours are those along each least-weight path. Heap keys and ties as in [`shortest`], which is
/// `kappa = 0` bit for bit.
pub fn shortest_by(g: &TransportGraph, source: u32, kappa: f64) -> Tree {
    assert!(
        kappa >= 0.0,
        "kappa {kappa} is not a non-negative time cost"
    );
    let n = g.nodes();
    let mut key = vec![f64::INFINITY; n];
    let mut cost = vec![f64::INFINITY; n];
    let mut hours = vec![f64::INFINITY; n];
    let mut done = vec![false; n];
    let mut heap = BinaryHeap::new();
    key[source as usize] = 0.0;
    cost[source as usize] = 0.0;
    hours[source as usize] = 0.0;
    heap.push(Reverse((0f64.to_bits(), source)));
    while let Some(Reverse((_, u))) = heap.pop() {
        if std::mem::replace(&mut done[u as usize], true) {
            continue;
        }
        let (ku, cu, hu) = (key[u as usize], cost[u as usize], hours[u as usize]);
        for (v, e) in g.arcs(u) {
            let (ce, he) = (g.cost[e as usize], g.hours[e as usize]);
            // kappa 0 keys on the cost alone, so a closed edge (both infinite) stays infinite
            let k = if kappa == 0.0 {
                ku + ce
            } else {
                ku + (ce + kappa * he)
            };
            if k < key[v as usize] {
                key[v as usize] = k;
                cost[v as usize] = cu + ce;
                hours[v as usize] = hu + he;
                heap.push(Reverse((k.to_bits(), v)));
            }
        }
    }
    Tree { cost, hours }
}

/// Province-to-all trees keyed by source node, dropped when the graph version moves.
#[derive(Default)]
pub struct PathCache {
    version: Option<u64>,
    trees: BTreeMap<u32, Tree>,
}

impl PathCache {
    fn sync(&mut self, g: &TransportGraph) {
        if self.version != Some(g.version()) {
            self.trees.clear();
            self.version = Some(g.version());
        }
    }

    pub fn tree(&mut self, g: &TransportGraph, source: u32) -> &Tree {
        self.sync(g);
        self.trees
            .entry(source)
            .or_insert_with(|| shortest(g, source))
    }

    /// Compute missing trees for `sources` in parallel; results do not depend on thread count.
    pub fn warm(&mut self, g: &TransportGraph, sources: &[u32]) {
        self.sync(g);
        let missing: Vec<u32> = sources
            .iter()
            .copied()
            .filter(|s| !self.trees.contains_key(s))
            .collect();
        let trees: Vec<Tree> = missing.par_iter().map(|&s| shortest(g, s)).collect();
        self.trees.extend(missing.into_iter().zip(trees));
    }

    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeList, Mode, NodeKind, NodeList, TransportParams};
    use proptest::prelude::*;

    fn graph(n: usize, edges: &[(u32, u32, f64)]) -> TransportGraph {
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
            length_km: edges.iter().map(|e| e.2).collect(),
            tri_m: vec![0.0; edges.len()],
            chokepoint: vec![0; edges.len()],
        };
        TransportGraph::new(n, nodes, list, &TransportParams::default())
    }

    fn floyd(g: &TransportGraph) -> Vec<Vec<f64>> {
        let n = g.nodes();
        let mut d = vec![vec![f64::INFINITY; n]; n];
        for (i, row) in d.iter_mut().enumerate() {
            row[i] = 0.0;
        }
        for e in 0..g.edges() {
            let (a, b) = (g.edge_a[e] as usize, g.edge_b[e] as usize);
            d[a][b] = d[a][b].min(g.cost[e]);
            d[b][a] = d[b][a].min(g.cost[e]);
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    d[i][j] = d[i][j].min(d[i][k] + d[k][j]);
                }
            }
        }
        d
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn dijkstra_matches_floyd_warshall(
            n in 2usize..24,
            raw in proptest::collection::vec((0u32..24, 0u32..24, 1.0f64..5000.0), 0..80),
        ) {
            let edges: Vec<_> = raw.into_iter().filter(|e| (e.0 as usize) < n && (e.1 as usize) < n && e.0 != e.1).collect();
            let g = graph(n, &edges);
            let d = floyd(&g);
            for (s, row) in d.iter().enumerate() {
                let t = shortest(&g, s as u32);
                for (v, (&x, &y)) in t.cost.iter().zip(row).enumerate() {
                    prop_assert!(x == y || (x - y).abs() <= 1e-9 * y, "{s}->{v}: {x} vs {y}");
                }
            }
        }
    }

    /// Provinces 0 and 1 with an airport (2, 3) and a port (4, 5) each, 5,000 km apart by air
    /// and by sea, and a long road between the provinces.
    fn three_modes() -> TransportGraph {
        let kind = vec![
            NodeKind::Province,
            NodeKind::Province,
            NodeKind::Airport,
            NodeKind::Airport,
            NodeKind::Port,
            NodeKind::Port,
        ];
        let e = [
            (0, 1, Mode::Road, 40000.0),
            (0, 2, Mode::AirAccess, 20.0),
            (2, 3, Mode::Air, 5000.0),
            (3, 1, Mode::AirAccess, 20.0),
            (0, 4, Mode::PortAccess, 20.0),
            (4, 5, Mode::Sea, 5000.0),
            (5, 1, Mode::PortAccess, 20.0),
        ];
        let nodes = NodeList {
            kind,
            province: vec![0, 1, 0, 1, 0, 1],
            lon: vec![0.0; 6],
            lat: vec![0.0; 6],
        };
        let list = EdgeList {
            a: e.iter().map(|x| x.0).collect(),
            b: e.iter().map(|x| x.1).collect(),
            mode: e.iter().map(|x| x.2).collect(),
            length_km: e.iter().map(|x| x.3).collect(),
            tri_m: vec![0.0; e.len()],
            chokepoint: vec![0; e.len()],
        };
        TransportGraph::new(2, nodes, list, &TransportParams::default())
    }

    #[test]
    fn iceberg_flies_light_goods_and_ships_heavy_ones() {
        let g = three_modes();
        let path = |edges: &[usize]| {
            let c: f64 = edges.iter().map(|&e| g.cost[e]).sum();
            let h: f64 = edges.iter().map(|&e| g.hours[e]).sum();
            (c, h)
        };
        let (air, sea) = (path(&[1, 2, 3]), path(&[4, 5, 6]));
        assert!(sea.0 < air.0 && air.1 < sea.1 && sea.0 < path(&[0]).0);
        // electronics-like: 1% a day on 2e-5 t per USD; bulk-like: 0.1% a day on 1 t per USD
        let (light, heavy) = (0.01 / (24.0 * 2e-5), 0.001 / 24.0);
        let fly = shortest_by(&g, 0, light);
        assert_eq!((fly.cost[1], fly.hours[1]), air);
        let ship = shortest_by(&g, 0, heavy);
        assert_eq!((ship.cost[1], ship.hours[1]), sea);
        assert_eq!(shortest(&g, 0), ship);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn iceberg_at_zero_time_cost_is_freight(
            n in 2usize..24,
            raw in proptest::collection::vec((0u32..24, 0u32..24, 1.0f64..5000.0), 0..80),
        ) {
            let edges: Vec<_> = raw.into_iter().filter(|e| (e.0 as usize) < n && (e.1 as usize) < n && e.0 != e.1).collect();
            let g = graph(n, &edges);
            for s in 0..n as u32 {
                let (a, b) = (shortest(&g, s), shortest_by(&g, s, 0.0));
                prop_assert!(a.cost.iter().zip(&b.cost).all(|(x, y)| x.to_bits() == y.to_bits()));
                prop_assert!(a.hours.iter().zip(&b.hours).all(|(x, y)| x.to_bits() == y.to_bits()));
            }
        }

        #[test]
        fn iceberg_tree_is_least_weight(
            n in 2usize..16,
            raw in proptest::collection::vec((0u32..16, 0u32..16, 1.0f64..5000.0), 0..50),
            kappa in 0.0f64..10.0,
        ) {
            let edges: Vec<_> = raw.into_iter().filter(|e| (e.0 as usize) < n && (e.1 as usize) < n && e.0 != e.1).collect();
            let mut g = graph(n, &edges);
            for (e, &(_, _, km)) in edges.iter().enumerate() {
                // vary hours against cost so the weight picks other paths than the cost
                g.hours[e] = 5000.0 / km;
            }
            let mut d = vec![vec![f64::INFINITY; n]; n];
            for (i, row) in d.iter_mut().enumerate() {
                row[i] = 0.0;
            }
            for e in 0..g.edges() {
                let (a, b) = (g.edge_a[e] as usize, g.edge_b[e] as usize);
                let w = g.cost[e] + kappa * g.hours[e];
                d[a][b] = d[a][b].min(w);
                d[b][a] = d[b][a].min(w);
            }
            for k in 0..n {
                for i in 0..n {
                    for j in 0..n {
                        d[i][j] = d[i][j].min(d[i][k] + d[k][j]);
                    }
                }
            }
            for (s, row) in d.iter().enumerate() {
                let t = shortest_by(&g, s as u32, kappa);
                for (v, &y) in row.iter().enumerate() {
                    let x = t.cost[v] + kappa * t.hours[v];
                    prop_assert!(x == y || (x - y).abs() <= 1e-9 * y, "{s}->{v}: {x} vs {y}");
                }
            }
        }
    }

    #[test]
    fn cache_drops_trees_when_costs_change() {
        let mut g = graph(3, &[(0, 1, 100.0), (1, 2, 100.0)]);
        let mut cache = PathCache::default();
        let before = cache.tree(&g, 0).cost[2];
        g.reprice(&TransportParams {
            per_tkm: [0.1, 0.04, 0.016, 0.8],
            ..Default::default()
        });
        let after = cache.tree(&g, 0).cost[2];
        assert_eq!(after, 2.0 * before);
        assert_eq!(cache.len(), 1);
    }
}
