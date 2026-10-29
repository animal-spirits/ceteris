//! The transport graph of the pack: its invariants, and the owner-side build against it.

mod common;

use std::fs::File;
use std::path::Path;

use arrow_array::cast::AsArray;
use arrow_array::types::{UInt8Type, UInt32Type};
use arrow_array::{ArrowPrimitiveType, RecordBatch};
use arrow_ipc::reader::FileReader;
use ceteris_pack::geo::transport::NodeKind;
use common::pack_table;

fn column<T: ArrowPrimitiveType>(path: &Path, name: &str) -> Vec<T::Native> {
    let reader = FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    let batches: Vec<RecordBatch> = reader.map(Result::unwrap).collect();
    batches
        .iter()
        .flat_map(|b| {
            b.column_by_name(name)
                .unwrap()
                .as_primitive::<T>()
                .values()
                .to_vec()
        })
        .collect()
}

fn rows(path: &Path) -> usize {
    let reader = FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    reader.map(|b| b.unwrap().num_rows()).sum()
}

#[test]
fn provinces_lead_the_nodes_and_edges_run_from_lower_to_higher_nodes() {
    let (Some(provinces), Some(nodes), Some(edges)) = (
        pack_table("provinces.arrow"),
        pack_table("nodes.arrow"),
        pack_table("edges.arrow"),
    ) else {
        return;
    };
    let n = rows(&provinces);
    let kinds = column::<UInt8Type>(&nodes, "kind");
    assert!(kinds[..n].iter().all(|&k| k == NodeKind::Province as u8));
    let (a, b) = (
        column::<UInt32Type>(&edges, "a"),
        column::<UInt32Type>(&edges, "b"),
    );
    assert!(a.iter().zip(&b).all(|(a, b)| a < b));
    assert!((*b.iter().max().unwrap() as usize) < kinds.len());
}

/// The owner-side build on the provinces of the cache and the pack's ruggedness: the pack's
/// graph, every node, edge and zone in its order. Positions and lengths agree to 1e-9: the
/// pack was built on another GEOS, and the anchors here come from h3o's cell centres.
#[cfg(feature = "geos")]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "builds every province and the graph; run with --release"
)]
fn the_transport_build_gives_the_graph_of_the_pack() {
    use arrow_array::types::Float64Type;
    use ceteris_pack::geo::{counties, provinces, transport};
    use ceteris_pack::sources::{CONFIG_DIR, workspace_root};
    use common::{cached, strings};
    use h3o::Resolution;

    let (Some(admin1), Some(admin0), Some(pack_provinces), Some(nodes), Some(edges), Some(zones)) = (
        cached("ne_10m_admin_1_states_provinces.zip"),
        cached("ne_10m_admin_0_countries.zip"),
        pack_table("provinces.arrow"),
        pack_table("nodes.arrow"),
        pack_table("edges.arrow"),
        pack_table("sea_zones.arrow"),
    ) else {
        return;
    };
    let root = workspace_root();
    let config: toml::Table = std::fs::read_to_string(root.join(CONFIG_DIR).join("pack.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let res = |key: &str| {
        Resolution::try_from(u8::try_from(config[key].as_integer().unwrap()).unwrap()).unwrap()
    };
    let split_area = config["split_area_km2"]
        .as_float()
        .or_else(|| config["split_area_km2"].as_integer().map(|i| i as f64))
        .unwrap();
    let units = counties::load(&root).unwrap();
    let centres = provinces::Centres::H3o;
    let provs = provinces::build(
        &admin1,
        &admin0,
        split_area,
        res("land_res"),
        &units,
        &centres,
    )
    .unwrap();
    let tri = column::<Float64Type>(&pack_provinces, "tri_m");
    let cache = root.join(ceteris_pack::fetch::CACHE_DIR);
    let t = transport::build(
        &provs,
        &tri,
        &cache,
        res("sea_res"),
        res("sea_zone_res"),
        &centres,
    )
    .unwrap();
    let g = &t.graph;

    let near = |a: &[f64], b: &[f64], what: &str| {
        assert_eq!(a.len(), b.len(), "{what}");
        let worst = a
            .iter()
            .zip(b)
            .map(|(x, y)| if x == y { 0.0 } else { (x / y - 1.0).abs() })
            .fold(0.0, f64::max);
        assert!(worst <= 1e-9, "{what}: {worst}");
    };
    let none = |v: &[Option<usize>]| -> Vec<u32> {
        v.iter()
            .map(|i| i.map_or(u32::MAX, |i| u32::try_from(i).unwrap()))
            .collect()
    };
    let kinds: Vec<u8> = g.kind.iter().map(|&k| k as u8).collect();
    assert_eq!(kinds, column::<UInt8Type>(&nodes, "kind"));
    assert_eq!(none(&g.province), column::<UInt32Type>(&nodes, "province"));
    assert_eq!(none(&g.zone), column::<UInt32Type>(&nodes, "sea_zone"));
    assert_eq!(g.label, strings(&nodes, "label"));
    near(&g.lon, &column::<Float64Type>(&nodes, "lon"), "node lon");
    near(&g.lat, &column::<Float64Type>(&nodes, "lat"), "node lat");

    let e = &g.edges;
    let u32s = |f: fn(&transport::Edge) -> usize| -> Vec<u32> {
        e.iter().map(|x| u32::try_from(f(x)).unwrap()).collect()
    };
    assert_eq!(u32s(|x| x.a), column::<UInt32Type>(&edges, "a"));
    assert_eq!(u32s(|x| x.b), column::<UInt32Type>(&edges, "b"));
    let modes: Vec<u8> = e.iter().map(|x| x.mode as u8).collect();
    assert_eq!(modes, column::<UInt8Type>(&edges, "mode"));
    let tags: Vec<u8> = e.iter().map(|x| x.chokepoint).collect();
    assert_eq!(tags, column::<UInt8Type>(&edges, "chokepoint"));
    let lengths: Vec<f64> = e.iter().map(|x| x.length_km).collect();
    near(
        &lengths,
        &column::<Float64Type>(&edges, "length_km"),
        "length",
    );
    let tri: Vec<f64> = e.iter().map(|x| x.tri_m).collect();
    assert_eq!(tri, column::<Float64Type>(&edges, "tri_m"));
    assert_eq!(t.zones, strings(&zones, "h3"));
    let reader = FileReader::try_new(File::open(&pack_provinces).unwrap(), None).unwrap();
    let coastal: Vec<bool> = reader
        .flat_map(|b| {
            let b = b.unwrap();
            let c = b.column_by_name("coastal").unwrap().as_boolean().clone();
            c.iter().map(Option::unwrap).collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(t.coastal, coastal);
}
