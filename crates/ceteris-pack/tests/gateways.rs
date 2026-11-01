//! Gateway selection on hand-made graphs and, when the pack and cache are present, the 2000
//! gateways.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type};
use ceteris_pack::econ::build::{ADMIN0, BACI};
use ceteris_pack::econ::gateways::{
    AIRPORT, Edges, Gateway, ITERATIONS, LAND, Nodes, PORT, TOTAL, counts, great_circle_km,
    kmedoids, land_crossings, node_country, select,
};
use ceteris_pack::read::baci::trade_by_country;
use ceteris_pack::read::natural_earth::read_codes;
use common::{Uniform, cached, pack_table, strings};

/// Provinces first (one per country code given), then extra nodes; kind 0 province, 2 port,
/// 3 airport. Returns the graph and the country of each province.
fn graph(points: &[(&str, f64, f64, u8)], links: &[(u32, u32, u8)]) -> (Nodes, Edges, Vec<String>) {
    let mut codes: Vec<String> = Vec::new();
    for (c, ..) in points {
        if !codes.iter().any(|x| x == c) {
            codes.push((*c).to_owned());
        }
    }
    let first = |c: &str| codes.iter().position(|x| x == c).unwrap() as u32;
    let nodes = Nodes {
        kind: points.iter().map(|p| p.3).collect(),
        province: points.iter().map(|p| first(p.0)).collect(),
        lon: points.iter().map(|p| p.1).collect(),
        lat: points.iter().map(|p| p.2).collect(),
    };
    let edges = Edges {
        a: links.iter().map(|l| l.0).collect(),
        b: links.iter().map(|l| l.1).collect(),
        mode: links.iter().map(|l| l.2).collect(),
    };
    (nodes, edges, codes)
}

#[test]
fn land_crossings_merge_within_50_km_per_country_pair() {
    // nodes 0, 1 province centroids of AAA and BBB; 2-5 rail nodes near the border, 6 a CCC centroid
    let pts = [
        ("AAA", 0.0, 0.0, 0),
        ("BBB", 1.0, 0.0, 0),
        ("AAA", 0.4, 0.0, 1),
        ("BBB", 0.6, 0.0, 1),
        ("AAA", 0.4, 3.0, 1),
        ("BBB", 0.6, 3.0, 1),
        ("CCC", 0.5, -1.0, 0),
    ];
    let links = [
        (0, 1, 0),
        (2, 3, 1),
        (4, 5, 1),
        (0, 6, 0),
        (0, 2, 0),
        (1, 6, 2),
    ];
    let (nodes, edges, provinces) = graph(&pts, &links);
    let country = node_country(&nodes.province, &provinces).unwrap();
    // the rail link 2-3 has the road link's midpoint (0.5, 0); 4-5 is 330 km north; the sea edge
    // 1-6 is not land
    assert_eq!(
        land_crossings(&nodes, &edges, &country).unwrap(),
        [(0, 1), (4, 5), (0, 6)]
    );
}

#[test]
fn kmedoids_finds_separate_clusters_deterministically() {
    let mut u = Uniform::new(1);
    let centres = [(0.0, 0.0), (20.0, 10.0), (-30.0, 40.0)];
    let (mut lon, mut lat) = (Vec::new(), Vec::new());
    for (x, y) in centres {
        for _ in 0..30 {
            lon.push(x + u.next(-1.0, 1.0));
            lat.push(y + u.next(-1.0, 1.0));
        }
    }
    let m = kmedoids(&lon, &lat, 3, ITERATIONS);
    let mut cluster: Vec<usize> = m.iter().map(|i| i / 30).collect();
    cluster.sort_unstable();
    assert_eq!(cluster, [0, 1, 2]);
    assert_eq!(m, kmedoids(&lon, &lat, 3, ITERATIONS));
    assert_eq!(kmedoids(&lon[..2], &lat[..2], 5, ITERATIONS), [0, 1]);
}

/// Longitudes, latitudes, k and the expected medoids.
type Case<'a> = (&'a [f64], &'a [f64], usize, &'a [usize]);

/// Points mirrored about the meridian lie at exactly equal distances, so these cases tie; the
/// expected medoids are those of the Python `kmedoids` on the same input.
#[test]
fn kmedoids_ties_follow_numpy() {
    let cases: [Case; 4] = [
        // equal row sums take the first point (the update step's tie would also keep it, so this
        // does not pin the build tie on its own)
        (&[10.0, -10.0], &[0.0, 0.0], 1, &[0]),
        // build: equal gains take the lower index
        (&[0.0, 10.0, -10.0], &[0.0; 3], 2, &[0, 1]),
        // a medoid duplicating an earlier one's position gets no members and is kept
        (&[0.0, 0.0, 0.0, 10.0], &[0.0; 4], 3, &[0, 1, 3]),
        (&[-10.0, 10.0, 0.0, 0.0], &[0.0, 0.0, 5.0, -5.0], 2, &[0, 2]),
    ];
    for (lon, lat, k, want) in cases {
        assert_eq!(
            kmedoids(lon, lat, k, ITERATIONS),
            want,
            "{lon:?} {lat:?} k {k}"
        );
    }
}

/// Distances bit for bit those of the Python `great_circle_km` (numpy) on the same points: two
/// pairs across the antimeridian, and an antipodal pair whose haversine term rounds to
/// 1.0000000000000002 and reaches the `min(1.0)` clamp. No f64 input makes the clamp change the
/// result: the square root of a term above 1 (at most 1 + 2^-52) rounds to 1.
#[test]
fn great_circle_km_matches_numpy_bitwise() {
    let cases = [
        ((179.5, 10.0, -179.5, 10.0), 109.50573519924384),
        ((179.5, 10.0, -179.0, -5.0), 1676.181665643988),
        ((-180.0, 27.0, 0.0, -27.0), 20015.114442035923),
    ];
    for ((lon1, lat1, lon2, lat2), want) in cases {
        let got = great_circle_km(lon1, lat1, lon2, lat2);
        assert_eq!(got.to_bits(), f64::to_bits(want), "{got:?} vs {want:?}");
    }
}

#[test]
fn gateway_counts_follow_the_square_root_of_trade() {
    let trade: BTreeMap<String, f64> = [("A", 1e12), ("B", 2.5e11), ("C", 1e6)]
        .map(|(c, v)| (c.to_owned(), v))
        .into();
    let caps = |a: usize| -> BTreeMap<String, usize> {
        [("A", a), ("B", 100), ("C", 100), ("D", 100)]
            .map(|(c, n)| (c.to_owned(), n))
            .into()
    };
    let k = counts(&trade, &caps(100), 30);
    assert_eq!((k["C"], k["D"]), (1, 1));
    assert_eq!(k.values().sum::<usize>(), 30);
    assert!(k["A"].abs_diff(2 * k["B"]) <= 1, "{k:?}");
    assert_eq!(counts(&trade, &caps(3), 30)["A"], 3);
}

#[test]
fn every_country_gets_a_gateway_from_its_ports_airports_or_land_crossings() {
    let pts = [
        ("AAA", 0.0, 0.0, 0),
        ("BBB", 5.0, 0.0, 0),
        ("CCC", 9.0, 0.0, 0),
        ("AAA", -1.0, 0.0, 2),
        ("AAA", -1.0, 1.0, 3),
        ("AAA", 3.0, 0.0, 2),
        ("CCC", 9.5, 0.0, 3),
    ];
    let (nodes, edges, provinces) = graph(&pts, &[(0, 1, 0), (1, 2, 1)]);
    let trade: BTreeMap<String, f64> = [("AAA", 1e12), ("BBB", 1e9), ("CCC", 1e9)]
        .map(|(c, v)| (c.to_owned(), v))
        .into();
    let rows = select(&nodes, &edges, &provinces, &trade, 5).unwrap();
    let countries: BTreeSet<u32> = rows.iter().map(|r| r.country).collect();
    assert_eq!(countries.into_iter().collect::<Vec<_>>(), [0, 1, 2]);
    assert_eq!(rows.len(), 5);
    assert!(rows.is_sorted_by_key(|r| (r.country, r.node)));
    // no throughput per gateway in the sources: each of a country's k gateways has mass 1/k
    for c in 0..3 {
        let mass: Vec<f64> = rows
            .iter()
            .filter(|r| r.country == c)
            .map(|r| r.mass)
            .collect();
        let k = mass.len() as f64;
        assert!(mass.iter().all(|&m| m == 1.0 / k), "country {c}: {mass:?}");
    }
    // BBB's only candidate is its centroid (node 1), the BBB end of both its land crossings,
    // counted once
    let bbb: Vec<(u32, u8)> = rows
        .iter()
        .filter(|r| r.country == 1)
        .map(|r| (r.node, r.kind))
        .collect();
    assert_eq!(bbb, [(1, LAND)]);
    assert!(
        rows.iter()
            .filter(|r| r.country == 0)
            .all(|r| [PORT, AIRPORT, LAND].contains(&r.kind))
    );
}

#[test]
fn a_country_without_candidates_is_refused() {
    let (nodes, edges, provinces) = graph(&[("AAA", 0.0, 0.0, 2), ("BBB", 5.0, 0.0, 0)], &[]);
    let err = select(&nodes, &edges, &provinces, &BTreeMap::new(), 5).unwrap_err();
    assert!(err.contains("BBB"), "{err}");
}

fn read_gateways(path: &std::path::Path) -> Vec<Gateway> {
    let reader = arrow_ipc::reader::FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    let mut out = Vec::new();
    for b in reader {
        let b = b.unwrap();
        let node = b
            .column_by_name("node")
            .unwrap()
            .as_primitive::<UInt32Type>();
        let country = b
            .column_by_name("country")
            .unwrap()
            .as_primitive::<UInt32Type>();
        let kind = b
            .column_by_name("kind")
            .unwrap()
            .as_primitive::<UInt8Type>();
        let mass = b
            .column_by_name("mass")
            .unwrap_or_else(|| {
                panic!(
                    "{} has no mass column (ADR 0092): rebuild the econ tables",
                    path.display()
                )
            })
            .as_primitive::<Float64Type>();
        for i in 0..b.num_rows() {
            out.push(Gateway {
                node: node.value(i),
                country: country.value(i),
                kind: kind.value(i),
                mass: mass.value(i),
            });
        }
    }
    out
}

#[test]
fn every_country_has_a_gateway() {
    let (Some(g), Some(c)) = (pack_table("gateways.arrow"), pack_table("countries.arrow")) else {
        return;
    };
    let rows = read_gateways(&g);
    let countries: BTreeSet<u32> = rows.iter().map(|r| r.country).collect();
    let n = strings(&c, "code").len() as u32;
    assert_eq!(countries, (0..n).collect());
    let nodes: BTreeSet<u32> = rows.iter().map(|r| r.node).collect();
    assert_eq!(nodes.len(), rows.len());
}

/// ADR 0092: the masses of each country's gateways sum to 1.
#[test]
fn gateway_mass_sums_to_one() {
    let Some(g) = pack_table("gateways.arrow") else {
        return;
    };
    let mut total: BTreeMap<u32, f64> = BTreeMap::new();
    for r in read_gateways(&g) {
        assert!(r.mass > 0.0 && r.mass <= 1.0, "{r:?}");
        *total.entry(r.country).or_default() += r.mass;
    }
    for (c, t) in total {
        assert!((t - 1.0).abs() <= 1e-12, "country {c}: masses sum to {t}");
    }
}

#[test]
fn gateway_count_near_600() {
    let Some(g) = pack_table("gateways.arrow") else {
        return;
    };
    assert!((500..=700).contains(&read_gateways(&g).len()));
}

fn column<T: arrow_array::types::ArrowPrimitiveType>(
    path: &std::path::Path,
    name: &str,
) -> Vec<T::Native> {
    let reader = arrow_ipc::reader::FileReader::try_new(File::open(path).unwrap(), None).unwrap();
    reader
        .flat_map(|b| {
            let b = b.unwrap();
            b.column_by_name(name)
                .unwrap()
                .as_primitive::<T>()
                .values()
                .to_vec()
        })
        .collect()
}

/// The pack's gateways.arrow was written by the Python build.
#[test]
fn select_reproduces_the_pack_gateways() {
    let (Some(g), Some(n), Some(e), Some(p)) = (
        pack_table("gateways.arrow"),
        pack_table("nodes.arrow"),
        pack_table("edges.arrow"),
        pack_table("provinces.arrow"),
    ) else {
        return;
    };
    let (Some(baci), Some(admin0), Some(c)) =
        (cached(BACI), cached(ADMIN0), pack_table("countries.arrow"))
    else {
        return;
    };
    let nodes = Nodes {
        kind: column::<UInt8Type>(&n, "kind"),
        province: column::<UInt32Type>(&n, "province"),
        lon: column::<Float64Type>(&n, "lon"),
        lat: column::<Float64Type>(&n, "lat"),
    };
    let edges = Edges {
        a: column::<UInt32Type>(&e, "a"),
        b: column::<UInt32Type>(&e, "b"),
        mode: column::<UInt8Type>(&e, "mode"),
    };
    let (iso, _) = read_codes(&admin0).unwrap();
    let trade = trade_by_country(&baci, &iso, &strings(&c, "code")).unwrap();
    let rows = select(&nodes, &edges, &strings(&p, "country"), &trade, TOTAL).unwrap();
    assert_eq!(rows, read_gateways(&g));
}
