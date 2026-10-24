//! Properties of least-cost paths on the built 2000 pack (Sections 5.3, 5.4). Skipped
//! when the pack has not been built.

mod common;

use ceteris_geo::graph::TransportParams;
use ceteris_geo::paths::{PathCache, shortest};
use ceteris_geo::route_choice::EdgeFactors;
use common::{PROBES, pack, province};

#[test]
fn every_province_is_reachable() {
    let Some(pack) = pack() else { return };
    for id in PROBES {
        let tree = shortest(&pack.graph, province(&pack, id));
        let stranded: Vec<_> = (0..pack.provinces.len())
            .filter(|&i| !tree.cost[i].is_finite())
            .map(|i| &pack.provinces.id[i])
            .collect();
        assert!(stranded.is_empty(), "unreachable from {id}: {stranded:?}");
    }
}

#[test]
fn costs_satisfy_the_triangle_inequality_on_every_arc() {
    let Some(pack) = pack() else { return };
    let g = &pack.graph;
    for id in PROBES {
        let t = shortest(g, province(&pack, id));
        for e in 0..g.edges() {
            let (a, b) = (g.edge_a[e] as usize, g.edge_b[e] as usize);
            assert!(
                t.cost[b] <= t.cost[a] + g.cost[e] + 1e-9
                    && t.cost[a] <= t.cost[b] + g.cost[e] + 1e-9
            );
        }
    }
}

#[test]
fn costs_are_symmetric() {
    let Some(pack) = pack() else { return };
    let ids: Vec<u32> = PROBES.iter().map(|id| province(&pack, id)).collect();
    let trees: Vec<_> = ids.iter().map(|&s| shortest(&pack.graph, s)).collect();
    for (i, ti) in trees.iter().enumerate() {
        for (j, tj) in trees.iter().enumerate() {
            let (x, y) = (ti.cost[ids[j] as usize], tj.cost[ids[i] as usize]);
            assert!(
                (x - y).abs() <= 1e-9 * x.max(1.0),
                "{} <-> {}: {x} vs {y}",
                PROBES[i],
                PROBES[j]
            );
        }
    }
}

#[test]
fn distance_orders_costs_from_rotterdam() {
    let Some(pack) = pack() else { return };
    let t = shortest(&pack.graph, province(&pack, "NLD-3485"));
    let c = |id| t.cost[province(&pack, id) as usize];
    assert!(
        c("DEU-1578") < c("CHN-1819"),
        "Hamburg should be cheaper than Shanghai"
    );
    assert!(
        c("CHN-1819") < c("MNG-3331"),
        "coastal Shanghai should be cheaper than landlocked Ulaanbaatar"
    );
}

#[test]
fn closing_suez_raises_europe_asia_costs_and_reopening_restores_them() {
    let Some(mut pack) = pack() else { return };
    let params = TransportParams::default();
    let suez = pack.chokepoint("Suez Canal").expect("Suez in pack");
    let (rotterdam, mumbai) = (
        province(&pack, "NLD-3485"),
        province(&pack, "IND-2447.1") as usize,
    );
    let mut cache = PathCache::default();
    let open = cache.tree(&pack.graph, rotterdam).cost[mumbai];
    pack.graph.set_chokepoint_closed(suez, true, &params);
    let closed = cache.tree(&pack.graph, rotterdam).cost[mumbai];
    pack.graph.set_chokepoint_closed(suez, false, &params);
    let reopened = cache.tree(&pack.graph, rotterdam).cost[mumbai];
    assert!(
        closed > open * 1.2,
        "closing Suez: {open:.2} -> {closed:.2} USD/t"
    );
    assert_eq!(open.to_bits(), reopened.to_bits());
}

#[test]
fn warm_cache_is_identical_across_thread_counts() {
    let Some(pack) = pack() else { return };
    let sources: Vec<u32> = (0..pack.provinces.len() as u32).step_by(97).collect();
    let warm = |threads| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut cache = PathCache::default();
        pool.install(|| cache.warm(&pack.graph, &sources));
        sources
            .iter()
            .map(|&s| {
                cache
                    .tree(&pack.graph, s)
                    .cost
                    .iter()
                    .map(|c| c.to_bits())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(warm(1), warm(8));
}

/// Traded goods' weight per value (t/USD, BACI 2000) and time cost lambda_g (trade P43).
const TRADED: [(&str, f64, f64); 7] = [
    ("agriculture", 0.0026752, 0.002),
    ("oil and gas", 0.0049491, 0.002),
    ("coal and mining", 0.0190502, 0.002),
    ("food", 0.0016041, 0.005),
    ("light manufacturing", 0.00026893, 0.010),
    ("chemicals, fuels, metals", 0.0015934, 0.010),
    ("machinery", 0.00010144, 0.010),
];
/// P44 point estimate (trade `TradeParams::theta_r`; ceteris-geo does not depend on ceteris-trade).
const THETA_R: f64 = 111.5;

// On the P1 graph the radius at the P44 prior's upper end (182) is 2.96 to 6.04 (MIGRATION
// Task 4 report, ADR 0017 Deferral).
#[test]
#[ignore = "route choice deferred past P2 (ADR 0017)"]
fn route_choice_radius_below_one_at_theta_r() {
    let Some(pack) = pack() else { return };
    let radii: Vec<(&str, f64)> = TRADED
        .iter()
        .map(|&(name, w, lambda)| {
            let f = EdgeFactors::new(&pack.graph, w, lambda, THETA_R);
            (name, f.spectral_radius())
        })
        .collect();
    assert!(
        radii.iter().all(|&(_, rho)| rho < 1.0),
        "spectral radius at theta_r {THETA_R}: {radii:?}"
    );
}
