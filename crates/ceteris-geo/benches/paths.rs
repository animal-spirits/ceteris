use ceteris_geo::graph::{EdgeList, Mode, NodeKind, NodeList, TransportGraph, TransportParams};
use ceteris_geo::paths::shortest;
use criterion::{Criterion, criterion_group, criterion_main};

/// A 200 x 200 grid: 40,000 nodes and about 80,000 edges, the size of the 2000 pack's graph.
fn grid(side: u32) -> TransportGraph {
    let n = (side * side) as usize;
    let nodes = NodeList {
        kind: vec![NodeKind::Province; n],
        province: (0..n as u32).collect(),
        lon: vec![0.0; n],
        lat: vec![0.0; n],
    };
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for r in 0..side {
        for c in 0..side {
            let i = r * side + c;
            if c + 1 < side {
                a.push(i);
                b.push(i + 1);
            }
            if r + 1 < side {
                a.push(i);
                b.push(i + side);
            }
        }
    }
    let m = a.len();
    let length_km = (0..m).map(|e| 50.0 + (e % 7) as f64 * 10.0).collect();
    let edges = EdgeList {
        a,
        b,
        mode: vec![Mode::Road; m],
        length_km,
        tri_m: vec![0.0; m],
        chokepoint: vec![0; m],
    };
    TransportGraph::new(n, nodes, edges, &TransportParams::default())
}

fn bench(c: &mut Criterion) {
    let g = grid(200);
    c.bench_function("dijkstra_grid_40k", |b| b.iter(|| shortest(&g, 0)));
}

criterion_group!(benches, bench);
criterion_main!(benches);
