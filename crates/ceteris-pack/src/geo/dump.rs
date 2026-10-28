//! `ceteris pack dump-geo DIR [--build]`: what the geo modules compute from the provinces and
//! counties of `DIR/provinces.geom.arrow` and `DIR/counties.geom.arrow` (one row per vertex:
//! geom, part, ring, x, y), written to `DIR/rust` as the tables and fills of
//! `scripts/parity/geo_inputs.py`. With `--build`, the provinces and clipped counties are built
//! here instead, and their vertex tables, attributes and clipping misfit are written to
//! `DIR/rust` too.

use std::path::Path;
use std::sync::Arc;

use arrow_array::types::{Float64Type, UInt32Type};
use arrow_array::{ArrayRef, Float64Array, UInt32Array, UInt64Array};
use geo::{Coord, LineString, MultiPolygon, Polygon};
use h3o::{CellIndex, LatLng, Resolution};

use super::raster_terrain::{
    Elevation, FINEST, SAMPLE_DEPTH, Terrain, assign_cells, cell_area_km2, province_means, terrain,
};
use super::{fills, resources, sphere};
use crate::econ::build::values;
use crate::sources::{CONFIG_DIR, read_table, workspace_root};
use crate::{fetch, table};

const ETOPO: &str = "ETOPO_2022_v1_60s_N90W180_surface.tif";

/// The geometries of a vertex table, in `geom` order.
pub fn read_geoms(path: &Path) -> Result<Vec<MultiPolygon>, String> {
    let geom = values::<UInt32Type>(path, "geom")?;
    let part = values::<UInt32Type>(path, "part")?;
    let ring = values::<UInt32Type>(path, "ring")?;
    let x = values::<Float64Type>(path, "x")?;
    let y = values::<Float64Type>(path, "y")?;
    let mut rows: Vec<(u32, u32, u32, Coord)> = Vec::with_capacity(geom.len());
    for i in 0..geom.len() {
        rows.push((geom[i], part[i], ring[i], Coord { x: x[i], y: y[i] }));
    }
    let mut out: Vec<Vec<Vec<LineString>>> = Vec::new();
    for (g, p, r, c) in rows {
        let (g, p, r) = (g as usize, p as usize, r as usize);
        if out.len() <= g {
            out.resize(g + 1, Vec::new());
        }
        if out[g].len() <= p {
            out[g].resize(p + 1, Vec::new());
        }
        if out[g][p].len() <= r {
            out[g][p].resize(r + 1, LineString(Vec::new()));
        }
        out[g][p][r].0.push(c);
    }
    Ok(out
        .into_iter()
        .map(|parts| {
            MultiPolygon(
                parts
                    .into_iter()
                    .map(|mut rings| {
                        let exterior = rings.remove(0);
                        Polygon::new(exterior, rings)
                    })
                    .collect(),
            )
        })
        .collect())
}

fn resolution(config: &toml::Table, key: &str) -> Result<Resolution, String> {
    let r = config
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("pack.toml: no {key}"))?;
    u8::try_from(r)
        .ok()
        .and_then(|r| Resolution::try_from(r).ok())
        .ok_or_else(|| format!("pack.toml: {key} = {r}"))
}

fn f64s(v: Vec<f64>) -> ArrayRef {
    Arc::new(Float64Array::from(v))
}

fn write(path: &Path, columns: Vec<(String, ArrayRef)>) -> Result<(), String> {
    let batch = table::batch(columns)?;
    table::write(path, &batch, &Default::default())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(())
}

fn named(columns: Vec<(&str, ArrayRef)>) -> Vec<(String, ArrayRef)> {
    columns
        .into_iter()
        .map(|(n, a)| (n.to_owned(), a))
        .collect()
}

fn cell_columns(
    cells: &[CellIndex],
    (owner_name, owner): (&str, Vec<u32>),
    area: Vec<f64>,
    t: Terrain,
) -> Vec<(String, ArrayRef)> {
    let cells = UInt64Array::from_iter_values(cells.iter().map(|&c| u64::from(c)));
    named(vec![
        ("cell", Arc::new(cells)),
        (owner_name, Arc::new(UInt32Array::from(owner))),
        ("area_km2", f64s(area)),
        ("elev_mean_m", f64s(t.elev_mean_m)),
        ("elev_std_m", f64s(t.elev_std_m)),
        ("tri_m", f64s(t.tri_m)),
    ])
}

/// Area-weighted terrain means per owner, as `build.build` computes them.
fn means(owner: &[u32], area: &[f64], t: &Terrain, n: usize) -> Terrain {
    Terrain {
        elev_mean_m: province_means(owner, area, &t.elev_mean_m, n),
        elev_std_m: province_means(owner, area, &t.elev_std_m, n),
        tri_m: province_means(owner, area, &t.tri_m, n),
    }
}

/// A vertex table of the geometries: one row per vertex, rings closed, in their stored order.
pub fn write_geoms(path: &Path, geoms: &[MultiPolygon]) -> Result<(), String> {
    let (mut g, mut p, mut r, mut x, mut y) = (vec![], vec![], vec![], vec![], vec![]);
    let index = |i: usize| u32::try_from(i).map_err(|e| e.to_string());
    for (i, geom) in geoms.iter().enumerate() {
        for (j, part) in geom.iter().enumerate() {
            for (k, ring) in std::iter::once(part.exterior())
                .chain(part.interiors())
                .enumerate()
            {
                for c in &ring.0 {
                    g.push(index(i)?);
                    p.push(index(j)?);
                    r.push(index(k)?);
                    x.push(c.x);
                    y.push(c.y);
                }
            }
        }
    }
    write(
        path,
        named(vec![
            ("geom", Arc::new(UInt32Array::from(g))),
            ("part", Arc::new(UInt32Array::from(p))),
            ("ring", Arc::new(UInt32Array::from(r))),
            ("x", f64s(x)),
            ("y", f64s(y)),
        ]),
    )
}

pub fn dump(dir: &Path) -> Result<(), String> {
    let provinces = read_geoms(&dir.join("provinces.geom.arrow"))?;
    let counties = read_geoms(&dir.join("counties.geom.arrow"))?;
    dump_into(&dir.join("rust"), &provinces, &counties)
}

/// Builds the provinces and clipped counties from the fetch cache as the pack build does, and
/// writes them with what the geo modules compute from them to `DIR/rust`.
#[cfg(feature = "geos")]
pub fn build(dir: &Path) -> Result<(), String> {
    use arrow_array::StringArray;

    use super::{counties, overlay, provinces};

    let root = workspace_root();
    let config = read_table(&root.join(CONFIG_DIR).join("pack.toml"))?;
    let split_area = config
        .get("split_area_km2")
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .ok_or("pack.toml: no split_area_km2")?;
    let admin1 = fetch::path_of("ne_10m_admin_1_states_provinces.zip");
    let admin0 = fetch::path_of("ne_10m_admin_0_countries.zip");
    let units = counties::load(&root)?;
    let provs = provinces::build(
        &admin1,
        &admin0,
        split_area,
        resolution(&config, "land_res")?,
        &units,
    )?;
    let clipped = counties::clip(&units, &provs, split_area)?;
    let out = dir.join("rust");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let text = |v: Vec<&str>| -> ArrayRef { Arc::new(StringArray::from(v)) };
    write(
        &out.join("province_ids.arrow"),
        named(vec![
            ("id", text(provs.iter().map(|p| p.id.as_str()).collect())),
            (
                "name",
                text(provs.iter().map(|p| p.name.as_str()).collect()),
            ),
            (
                "country",
                text(provs.iter().map(|p| p.country.as_str()).collect()),
            ),
            (
                "sovereign",
                text(provs.iter().map(|p| p.sovereign.as_str()).collect()),
            ),
            (
                "country_now",
                text(provs.iter().map(|p| p.country_now.as_str()).collect()),
            ),
            (
                "split_rule",
                text(provs.iter().map(|p| p.split_rule.as_str()).collect()),
            ),
        ]),
    )?;
    let cty = &clipped.counties;
    write(
        &out.join("county_ids.arrow"),
        named(vec![
            ("name", text(cty.iter().map(|c| c.name.as_str()).collect())),
            (
                "country",
                text(cty.iter().map(|c| c.iso.as_str()).collect()),
            ),
            (
                "level",
                text(cty.iter().map(|c| c.level.as_str()).collect()),
            ),
            (
                "province",
                Arc::new(UInt32Array::from(clipped.province.clone())),
            ),
        ]),
    )?;
    write(
        &out.join("clip_stats.arrow"),
        named(vec![
            ("fits", f64s(vec![clipped.fits])),
            ("fits_inside", f64s(vec![clipped.fits_inside])),
        ]),
    )?;
    let provinces: Vec<MultiPolygon> = provs.iter().map(|p| provinces::parts(&p.geom)).collect();
    let counties = cty
        .iter()
        .map(|c| overlay::to_multipolygon(&c.geom))
        .collect::<Result<Vec<_>, _>>()?;
    write_geoms(&out.join("provinces.geom.arrow"), &provinces)?;
    write_geoms(&out.join("counties.geom.arrow"), &counties)?;
    dump_into(&out, &provinces, &counties)
}

fn dump_into(
    out: &Path,
    provinces: &[MultiPolygon],
    counties: &[MultiPolygon],
) -> Result<(), String> {
    let config = read_table(&workspace_root().join(CONFIG_DIR).join("pack.toml"))?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let z = Elevation::open(&fetch::path_of(ETOPO))?;
    let anchors = |geoms: &[MultiPolygon]| -> Result<Vec<(f64, f64)>, String> {
        geoms
            .iter()
            .map(|g| sphere::anchor(g).ok_or("a geometry without parts".to_owned()))
            .collect()
    };

    let canchor = anchors(counties)?;
    let (ccells, cowner) = assign_cells(counties, &canchor, resolution(&config, "county_res")?)?;
    let cterr = terrain(&ccells, &z, 0)?;
    let carea = cell_area_km2(&ccells);
    let mut cstat = means(&cowner, &carea, &cterr, counties.len());
    // Slivers whose only cell went to a neighbour are sampled at their anchor.
    let bare: Vec<usize> = (0..counties.len())
        .filter(|&i| cstat.elev_mean_m[i].is_nan())
        .collect();
    let at: Vec<CellIndex> = bare
        .iter()
        .map(|&i| {
            LatLng::new(canchor[i].1, canchor[i].0)
                .map(|ll| ll.to_cell(FINEST))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let point = terrain(&at, &z, 0)?;
    for (k, &i) in bare.iter().enumerate() {
        cstat.elev_mean_m[i] = point.elev_mean_m[k];
        cstat.elev_std_m[i] = point.elev_std_m[k];
        cstat.tri_m[i] = point.tri_m[k];
    }
    write(
        &out.join("county_cells.arrow"),
        cell_columns(&ccells, ("county", cowner), carea, cterr),
    )?;
    write(
        &out.join("counties.arrow"),
        named(vec![
            ("lon", f64s(canchor.iter().map(|a| a.0).collect())),
            ("lat", f64s(canchor.iter().map(|a| a.1).collect())),
            (
                "area_km2",
                f64s(counties.iter().map(sphere::area_km2).collect()),
            ),
            ("elev_mean_m", f64s(cstat.elev_mean_m)),
            ("elev_std_m", f64s(cstat.elev_std_m)),
            ("tri_m", f64s(cstat.tri_m)),
        ]),
    )?;

    let n = provinces.len();
    let anchor = anchors(provinces)?;
    let (cells, owner) = assign_cells(provinces, &anchor, resolution(&config, "land_res")?)?;
    let terr = terrain(&cells, &z, SAMPLE_DEPTH)?;
    let area = cell_area_km2(&cells);
    let stat = means(&owner, &area, &terr, n);
    let mut count = vec![0u32; n];
    for &o in &owner {
        count[o as usize] += 1;
    }
    let res = resources::counts(&fetch::path_of(resources::MRDS), provinces)?;
    write(
        &out.join("cells.arrow"),
        cell_columns(&cells, ("province", owner), area, terr),
    )?;
    let mut columns = named(vec![
        ("lon", f64s(anchor.iter().map(|a| a.0).collect())),
        ("lat", f64s(anchor.iter().map(|a| a.1).collect())),
        (
            "area_km2",
            f64s(provinces.iter().map(sphere::area_km2).collect()),
        ),
        ("elev_mean_m", f64s(stat.elev_mean_m)),
        ("elev_std_m", f64s(stat.elev_std_m)),
        ("tri_m", f64s(stat.tri_m)),
        ("cells", Arc::new(UInt32Array::from(count))),
    ]);
    for ((group, _), v) in resources::GROUPS.iter().zip(res) {
        columns.push((format!("res_{group}"), Arc::new(UInt32Array::from(v))));
    }
    write(&out.join("provinces.arrow"), columns)?;
    fills::write(&out.join("provinces.fill"), provinces)?;
    fills::write(&out.join("counties.fill"), counties)?;
    Ok(())
}
