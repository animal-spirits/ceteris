//! The admin-1 shapefile and the geoBoundaries files read as pyogrio reads them: the digests of
//! `scripts/parity/geo_readers.py`.

mod common;

use ceteris_pack::read::{geojson, natural_earth};
use ceteris_pack::{fetch, sources};
use common::cached;
use geo::Geometry;
use sha2::{Digest, Sha256};

fn feed(h: &mut Sha256, g: Option<&Geometry>) {
    let count = |h: &mut Sha256, n: usize| h.update(u32::try_from(n).unwrap().to_le_bytes());
    let parts = match g {
        None => return h.update([0]),
        Some(Geometry::Polygon(p)) => {
            h.update([3]);
            vec![p]
        }
        Some(Geometry::MultiPolygon(m)) => {
            h.update([6]);
            m.0.iter().collect()
        }
        Some(g) => panic!("{g:?}"),
    };
    count(h, parts.len());
    for p in parts {
        count(h, 1 + p.interiors().len());
        for r in std::iter::once(p.exterior()).chain(p.interiors()) {
            count(h, r.0.len());
            for c in &r.0 {
                h.update(c.x.to_le_bytes());
                h.update(c.y.to_le_bytes());
            }
        }
    }
}

#[test]
fn admin1_shapes_read_as_pyogrio_reads_them() {
    let Some(path) = cached("ne_10m_admin_1_states_provinces.zip") else {
        return;
    };
    let shapes = natural_earth::shapes_from_zip(&path).unwrap();
    let mut h = Sha256::new();
    for g in &shapes {
        feed(&mut h, g.as_ref());
    }
    assert_eq!(
        (shapes.len(), format!("{:x}", h.finalize()).as_str()),
        (
            4596,
            "d16ba00b3666f4c145a1f47e05e9c27a76f310f7f61783d0133db3afa4f011ec"
        )
    );
}

#[test]
fn geoboundaries_read_as_pyogrio_reads_them() {
    let entries = fetch::entries(&sources::workspace_root()).unwrap();
    let files: Vec<_> = entries
        .iter()
        .filter(|e| e.source == "geoboundaries")
        .collect();
    let Some(paths) = files
        .iter()
        .map(|e| cached(&e.name))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let mut h = Sha256::new();
    let mut n = 0;
    for path in paths {
        for (name, g) in geojson::features(&path, "shapeName").unwrap() {
            h.update(name.unwrap_or_default().as_bytes());
            h.update([0]);
            feed(&mut h, g.as_ref());
            n += 1;
        }
    }
    assert_eq!(
        (n, format!("{:x}", h.finalize()).as_str()),
        (
            53441,
            "266a98042eb84a5b65817497e6fc4fe0cecc132f52c7643516bc48a76a438c2b"
        )
    );
}

#[cfg(feature = "geos")]
#[test]
fn goias_is_repaired_as_shapely_on_geos_3_15_repairs_it() {
    use ceteris_pack::geo::provinces;
    let (Some(admin1), Some(admin0)) = (
        cached("ne_10m_admin_1_states_provinces.zip"),
        cached("ne_10m_admin_0_countries.zip"),
    ) else {
        return;
    };
    let repaired = provinces::load(&admin1, &admin0).unwrap();
    let goias = repaired.iter().find(|p| p.id == "BRA-1294").unwrap();
    let mut h = Sha256::new();
    feed(&mut h, Some(&goias.geom));
    assert_eq!(
        format!("{:x}", h.finalize()),
        "121530c223e0383469f00aedaa9e84819de85744a3131dc1c33b11359a8179c2"
    );
}
