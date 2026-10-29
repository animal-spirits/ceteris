//! The pack build (Section 16.1): the sources fetched into the cache; the provinces, clipped
//! counties, H3 cells, terrain, mineral sites and transport graph built from them; the P1
//! tables and the map's attribute files written; then the econ tables.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, UInt32Array, UInt64Array};
use geo::MultiPolygon;
use h3o::{CellIndex, LatLng, Resolution};

use crate::geo::raster_terrain::{
    Elevation, FINEST, Terrain, assign_cells, cell_area_km2, province_means, terrain,
};
use crate::read::natural_earth::Dbf;
use crate::sources::{CONFIG_DIR, read_table};

/// Map colours: neighbouring countries differ where a country has fewer neighbours than this.
pub const PALETTE_SIZE: usize = 8;
pub const ETOPO: &str = "ETOPO_2022_v1_60s_N90W180_surface.tif";
pub const ADMIN1: &str = "ne_10m_admin_1_states_provinces.zip";
pub const ADMIN0: &str = "ne_10m_admin_0_countries.zip";

/// Where a build writes by default: `data/packs/<year of pack.toml>` under `root`.
pub fn default_out(root: &Path) -> Result<PathBuf, String> {
    let config = read_table(&root.join(CONFIG_DIR).join("pack.toml"))?;
    let year = config
        .get("year")
        .and_then(toml::Value::as_integer)
        .ok_or("pack.toml: no year")?;
    Ok(root.join("data/packs").join(year.to_string()))
}

/// An H3 resolution of `pack.toml`.
pub fn resolution(config: &toml::Table, key: &str) -> Result<Resolution, String> {
    let r = config
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("pack.toml: no {key}"))?;
    u8::try_from(r)
        .ok()
        .and_then(|r| Resolution::try_from(r).ok())
        .ok_or_else(|| format!("pack.toml: {key} = {r}"))
}

/// The area above which `pack.toml` splits a province.
pub fn split_area_km2(config: &toml::Table) -> Result<f64, String> {
    config
        .get("split_area_km2")
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .ok_or_else(|| "pack.toml: no split_area_km2".to_owned())
}

/// 0 plains, 1 plateau, 2 hills, 3 mountains. TRI is at the 1.85 km ETOPO pixel scale.
pub fn terrain_class(elev_mean_m: f64, tri_m: f64) -> u8 {
    if tri_m >= 150.0 {
        3
    } else if tri_m >= 40.0 {
        2
    } else if elev_mean_m >= 600.0 {
        1
    } else {
        0
    }
}

/// Welsh-Powell greedy colouring so neighbouring countries differ: most neighbours first (then
/// by code), each taking the lowest number its coloured neighbours lack, modulo `PALETTE_SIZE`.
pub fn colour_countries(
    codes: &[String],
    borders: &BTreeSet<(String, String)>,
) -> Result<BTreeMap<String, u8>, String> {
    let mut nbrs: BTreeMap<&str, BTreeSet<&str>> = codes
        .iter()
        .map(|c| (c.as_str(), BTreeSet::new()))
        .collect();
    for (a, b) in borders {
        for (x, y) in [(a, b), (b, a)] {
            nbrs.get_mut(x.as_str())
                .ok_or_else(|| format!("border of {x}, which is not a country"))?
                .insert(y.as_str());
        }
    }
    let mut order: Vec<&str> = codes.iter().map(String::as_str).collect();
    order.sort_by(|a, b| nbrs[b].len().cmp(&nbrs[a].len()).then(a.cmp(b)));
    let mut colour: BTreeMap<String, u8> = BTreeMap::new();
    for c in order {
        let taken: BTreeSet<usize> = nbrs[c]
            .iter()
            .filter_map(|n| colour.get(*n).map(|&k| usize::from(k)))
            .collect();
        let k = (0..codes.len())
            .find(|k| !taken.contains(k))
            .ok_or_else(|| format!("no colour left for {c}"))?;
        colour.insert(c.to_owned(), (k % PALETTE_SIZE) as u8);
    }
    Ok(colour)
}

/// Country names: admin-0 ADMIN by ADM0_A3 (a later row overriding an earlier one), then `extra`
/// (the names `borders_2000.toml` gives codes Natural Earth does not carry). None where admin-0
/// leaves ADMIN blank.
pub fn country_names(
    admin0: &Path,
    extra: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, Option<String>>, String> {
    let dbf = Dbf::from_zip(admin0)?;
    let mut names: BTreeMap<String, Option<String>> = BTreeMap::new();
    for [code, name] in dbf
        .select(["ADM0_A3", "ADMIN"])
        .map_err(|e| format!("{}: {e}", admin0.display()))?
    {
        if let Some(code) = code {
            names.insert(code.to_owned(), name.map(str::to_owned));
        }
    }
    names.extend(extra.iter().map(|(k, v)| (k.clone(), Some(v.clone()))));
    Ok(names)
}

/// Python's `round(x)`: the nearest integer, ties to even; NaN and the infinities fail as in
/// Python.
pub fn round_int(x: f64) -> Result<i64, String> {
    if !x.is_finite() {
        return Err(format!("cannot round {x} to an integer"));
    }
    Ok(x.round_ties_even() as i64)
}

/// The H3 cells of some geometries, ordered by owner then cell, with their areas and their
/// terrain sampled `depth` resolutions finer, and each owner's area-weighted mean terrain.
pub struct Sampled {
    pub cells: Vec<CellIndex>,
    pub owner: Vec<u32>,
    pub area: Vec<f64>,
    pub terrain: Terrain,
    pub means: Terrain,
}

/// The cells of `geoms` at `res` (a geometry without a cell centre takes the cell of its anchor,
/// as `assign_cells` does), sampled on `z`.
pub fn sample(
    geoms: &[MultiPolygon],
    anchors: &[(f64, f64)],
    res: Resolution,
    z: &Elevation,
    depth: u8,
) -> Result<Sampled, String> {
    let (cells, owner) = assign_cells(geoms, anchors, res)?;
    let terrain = terrain(&cells, z, depth)?;
    let area = cell_area_km2(&cells);
    let mean = |v: &[f64]| province_means(&owner, &area, v, geoms.len());
    let means = Terrain {
        elev_mean_m: mean(&terrain.elev_mean_m),
        elev_std_m: mean(&terrain.elev_std_m),
        tri_m: mean(&terrain.tri_m),
    };
    Ok(Sampled {
        cells,
        owner,
        area,
        terrain,
        means,
    })
}

/// The counties' cells sampled on `z` without finer samples; slivers whose only cell went to a
/// neighbour are sampled at their anchor.
pub fn sample_counties(
    geoms: &[MultiPolygon],
    anchors: &[(f64, f64)],
    res: Resolution,
    z: &Elevation,
) -> Result<Sampled, String> {
    let mut s = sample(geoms, anchors, res, z, 0)?;
    let bare: Vec<usize> = (0..geoms.len())
        .filter(|&i| s.means.elev_mean_m[i].is_nan())
        .collect();
    if bare.is_empty() {
        return Ok(s);
    }
    let at: Vec<CellIndex> = bare
        .iter()
        .map(|&i| {
            LatLng::new(anchors[i].1, anchors[i].0)
                .map(|ll| ll.to_cell(FINEST))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let point = terrain(&at, z, 0)?;
    for (k, &i) in bare.iter().enumerate() {
        s.means.elev_mean_m[i] = point.elev_mean_m[k];
        s.means.elev_std_m[i] = point.elev_std_m[k];
        s.means.tri_m[i] = point.tri_m[k];
    }
    Ok(s)
}

/// The columns of a cells table: cell, owner (named `owner_name`), area and terrain.
pub fn cell_columns(owner_name: &str, s: &Sampled) -> Vec<(String, ArrayRef)> {
    let cells = UInt64Array::from_iter_values(s.cells.iter().map(|&c| u64::from(c)));
    let f64s = |v: &[f64]| -> ArrayRef { Arc::new(Float64Array::from(v.to_vec())) };
    vec![
        ("cell".to_owned(), Arc::new(cells) as ArrayRef),
        (
            owner_name.to_owned(),
            Arc::new(UInt32Array::from(s.owner.clone())),
        ),
        ("area_km2".to_owned(), f64s(&s.area)),
        ("elev_mean_m".to_owned(), f64s(&s.terrain.elev_mean_m)),
        ("elev_std_m".to_owned(), f64s(&s.terrain.elev_std_m)),
        ("tri_m".to_owned(), f64s(&s.terrain.tri_m)),
    ]
}

#[cfg(feature = "geos")]
pub use full::{Built, build};

#[cfg(feature = "geos")]
mod full {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;
    use std::sync::Arc;

    use arrow_array::{
        ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt8Array, UInt32Array,
    };
    use geo::MultiPolygon;
    use sha2::{Digest, Sha256};

    use super::{
        ADMIN0, ADMIN1, ETOPO, cell_columns, colour_countries, country_names, resolution,
        round_int, sample, sample_counties, split_area_km2, terrain_class,
    };
    use crate::geo::overlay::to_multipolygon;
    use crate::geo::provinces::{self, Centres};
    use crate::geo::raster_terrain::{Elevation, SAMPLE_DEPTH};
    use crate::geo::transport::{self, Mode, NodeKind};
    use crate::geo::{counties, resources, sphere};
    use crate::json::Json;
    use crate::numeric::round;
    use crate::sources::{CONFIG_DIR, read_table, workspace_root};
    use crate::{econ, fetch, table};

    /// The name of a code, the code itself where there is none.
    pub(super) fn name_of<'a>(
        names: &'a BTreeMap<String, Option<String>>,
        code: &'a str,
    ) -> Result<&'a str, String> {
        match names.get(code) {
            None => Ok(code),
            Some(Some(name)) => Ok(name),
            Some(None) => Err(format!("admin-0 leaves the name of {code} blank")),
        }
    }

    /// An object with its keys in the order given.
    fn fields(entries: Vec<(&str, Json)>) -> Json {
        Json::Fields(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }

    fn ints<T: Copy + Into<i64>>(v: &[T]) -> Json {
        Json::list(v.iter().map(|&x| Json::Int(x.into())))
    }

    fn rounded(v: impl IntoIterator<Item = f64>, ndigits: usize) -> Json {
        Json::list(v.into_iter().map(|x| round(x, ndigits)))
    }

    fn rounded_ints(v: &[f64]) -> Result<Json, String> {
        Ok(Json::List(
            v.iter()
                .map(|&x| round_int(x).map(Json::Int))
                .collect::<Result<_, _>>()?,
        ))
    }

    fn hex(digest: &[u8; 32]) -> String {
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// What a build wrote: each file's SHA-256 by its path in the pack, and the build stats in
    /// the manifest's order.
    pub struct Built {
        pub tables: BTreeMap<String, String>,
        pub stats: Json,
    }

    fn count(n: usize) -> Json {
        Json::Int(i64::try_from(n).expect("count fits i64"))
    }

    /// Where a build writes, and the SHA-256 of each file it wrote by its path in the pack.
    struct Pack<'a> {
        out: &'a Path,
        tables: BTreeMap<String, String>,
    }

    impl Pack<'_> {
        fn table(&mut self, name: &str, batch: &RecordBatch) -> Result<(), String> {
            let path = self.out.join(name);
            let digest = table::write(&path, batch, &Default::default())
                .map_err(|e| format!("{}: {e}", path.display()))?;
            self.tables.insert(name.to_owned(), hex(&digest));
            Ok(())
        }

        fn columns(&mut self, name: &str, columns: Vec<(&str, ArrayRef)>) -> Result<(), String> {
            let batch = table::batch(
                columns
                    .into_iter()
                    .map(|(n, a)| (n.to_owned(), a))
                    .collect(),
            )?;
            self.table(name, &batch)
        }

        fn text(&mut self, name: &str, text: &str) -> Result<(), String> {
            let path = self.out.join(name);
            std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            let digest: [u8; 32] = Sha256::digest(text).into();
            self.tables.insert(name.to_owned(), hex(&digest));
            Ok(())
        }
    }

    /// Builds the pack into `out` from the fetch cache, the Voronoi splits seeded from, and the
    /// sea grid placed at, the cell centres `centres` gives.
    pub fn build(out: &Path, centres: &Centres) -> Result<Built, String> {
        let root = workspace_root();
        let cache = root.join(fetch::CACHE_DIR);
        for e in fetch::entries(&root)? {
            fetch::fetch(&e, &cache).map_err(|e| e.to_string())?;
        }
        let config = read_table(&root.join(CONFIG_DIR).join("pack.toml"))?;
        let split_area = split_area_km2(&config)?;
        let dev_slice: BTreeSet<&str> = config
            .get("dev_slice")
            .and_then(toml::Value::as_array)
            .ok_or("pack.toml: no dev_slice")?
            .iter()
            .filter_map(toml::Value::as_str)
            .collect();

        let units = counties::load(&root)?;
        let provs = provinces::build(
            &cache.join(ADMIN1),
            &cache.join(ADMIN0),
            split_area,
            resolution(&config, "land_res")?,
            &units,
            centres,
        )?;
        let n = provs.len();
        // Counties: each in the province it overlaps most, clipped to it (ADR 0009).
        let clipped = counties::clip(&units, &provs, split_area)?;
        let cty = &clipped.counties;
        let cgeoms = cty
            .iter()
            .map(|c| to_multipolygon(&c.geom))
            .collect::<Result<Vec<_>, _>>()?;
        let canchor: Vec<(f64, f64)> = cty.iter().map(|c| (c.lon, c.lat)).collect();
        let cty_area: Vec<f64> = cgeoms.iter().map(sphere::area_km2).collect();
        let z = Elevation::open(&cache.join(ETOPO))?;
        let cs = sample_counties(&cgeoms, &canchor, resolution(&config, "county_res")?, &z)?;
        let pgeoms: Vec<MultiPolygon> = provs.iter().map(|p| provinces::parts(&p.geom)).collect();
        let panchor: Vec<(f64, f64)> = provs.iter().map(|p| (p.lon, p.lat)).collect();
        let ps = sample(
            &pgeoms,
            &panchor,
            resolution(&config, "land_res")?,
            &z,
            SAMPLE_DEPTH,
        )?;
        drop(z);
        let res = resources::counts(&cache.join(resources::MRDS), &pgeoms)?;
        let t = transport::build(
            &provs,
            &ps.means.tri_m,
            &cache,
            resolution(&config, "sea_res")?,
            resolution(&config, "sea_zone_res")?,
            centres,
        )?;
        let g = &t.graph;

        let per_province = |kind: NodeKind| {
            let mut v = vec![0u32; n];
            for (k, p) in g.kind.iter().zip(&g.province) {
                if let (true, Some(p)) = (*k == kind, p) {
                    v[*p] += 1;
                }
            }
            v
        };
        let codes: Vec<String> = provs
            .iter()
            .map(|p| p.country.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let country_of = |i: usize| {
            provs
                .get(i)
                .map(|p| p.country.as_str())
                .ok_or_else(|| format!("road edge at node {i}, which is not a province"))
        };
        let mut borders = BTreeSet::new();
        for e in g.edges.iter().filter(|e| e.mode == Mode::Road) {
            let (a, b) = (country_of(e.a)?, country_of(e.b)?);
            if a != b {
                let (a, b) = if a < b { (a, b) } else { (b, a) };
                borders.insert((a.to_owned(), b.to_owned()));
            }
        }
        let colour = colour_countries(&codes, &borders)?;
        let names = country_names(&cache.join(ADMIN0), &provinces::borders()?.names)?;
        let name_or_code = |c: &str| name_of(&names, c).map(str::to_owned);
        let mut sovereign: BTreeMap<&str, &str> = BTreeMap::new();
        for p in &provs {
            sovereign.insert(&p.country, &p.sovereign);
        }
        let terrain: Vec<u8> = ps
            .means
            .elev_mean_m
            .iter()
            .zip(&ps.means.tri_m)
            .map(|(&e, &t)| terrain_class(e, t))
            .collect();
        let in_slice: Vec<bool> = provs
            .iter()
            .map(|p| dev_slice.contains(p.country.as_str()))
            .collect();
        let ports = per_province(NodeKind::Port);
        let airports = per_province(NodeKind::Airport);
        let rail: Vec<bool> = per_province(NodeKind::Rail)
            .iter()
            .map(|&k| k > 0)
            .collect();
        let mut cells_per = vec![0u32; n];
        for &o in &ps.owner {
            cells_per[o as usize] += 1;
        }

        let web = out.join("web");
        std::fs::create_dir_all(&web).map_err(|e| format!("{}: {e}", web.display()))?;
        let text = |v: Vec<&str>| -> ArrayRef { Arc::new(StringArray::from(v)) };
        let texts = |v: Vec<String>| -> ArrayRef { Arc::new(StringArray::from(v)) };
        let f64s = |v: Vec<f64>| -> ArrayRef { Arc::new(Float64Array::from(v)) };
        let u32s = |v: Vec<u32>| -> ArrayRef { Arc::new(UInt32Array::from(v)) };
        let bools = |v: Vec<bool>| -> ArrayRef { Arc::new(BooleanArray::from(v)) };
        let mut pack = Pack {
            out,
            tables: BTreeMap::new(),
        };

        let mut columns: Vec<(&str, ArrayRef)> = vec![
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
            ("lon", f64s(provs.iter().map(|p| p.lon).collect())),
            ("lat", f64s(provs.iter().map(|p| p.lat).collect())),
            ("area_km2", f64s(provs.iter().map(|p| p.area_km2).collect())),
            ("elev_mean_m", f64s(ps.means.elev_mean_m.clone())),
            ("elev_std_m", f64s(ps.means.elev_std_m.clone())),
            ("tri_m", f64s(ps.means.tri_m.clone())),
            ("terrain", Arc::new(UInt8Array::from(terrain.clone()))),
            ("coastal", bools(t.coastal.clone())),
            ("dev_slice", bools(in_slice.clone())),
            ("cells", u32s(cells_per)),
            ("ports", u32s(ports.clone())),
            ("airports", u32s(airports.clone())),
            ("rail", bools(rail.clone())),
        ];
        let res_names: Vec<String> = resources::GROUPS
            .iter()
            .map(|(group, _)| format!("res_{group}"))
            .collect();
        for (name, v) in res_names.iter().zip(&res) {
            columns.push((name, u32s(v.clone())));
        }
        pack.columns("provinces.arrow", columns)?;
        let sovereign_of = |c: &str| {
            sovereign
                .get(c)
                .copied()
                .ok_or_else(|| format!("no sovereign for {c}"))
        };
        let sovereigns: Vec<&str> = codes
            .iter()
            .map(|c| sovereign_of(c))
            .collect::<Result<_, _>>()?;
        let code_names: Vec<String> = codes
            .iter()
            .map(|c| name_or_code(c))
            .collect::<Result<_, _>>()?;
        let sovereign_names: Vec<String> = sovereigns
            .iter()
            .map(|c| name_or_code(c))
            .collect::<Result<_, _>>()?;
        let colours: Vec<u8> = codes.iter().map(|c| colour[c]).collect();
        pack.columns(
            "countries.arrow",
            vec![
                ("code", text(codes.iter().map(String::as_str).collect())),
                ("name", texts(code_names.clone())),
                ("sovereign", text(sovereigns)),
                ("sovereign_name", texts(sovereign_names.clone())),
                ("colour", Arc::new(UInt8Array::from(colours.clone()))),
            ],
        )?;
        pack.table("cells.arrow", &table::batch(cell_columns("province", &ps))?)?;
        for (name, batch) in transport::tables(&t.graph, &t.zones, &transport::chokepoints()?)? {
            pack.table(name, &batch)?;
        }
        pack.columns(
            "counties.arrow",
            vec![
                ("name", text(cty.iter().map(|c| c.name.as_str()).collect())),
                (
                    "country",
                    text(cty.iter().map(|c| c.iso.as_str()).collect()),
                ),
                (
                    "level",
                    text(cty.iter().map(|c| c.level.as_str()).collect()),
                ),
                ("province", u32s(clipped.province.clone())),
                ("area_km2", f64s(cty_area.clone())),
                ("elev_mean_m", f64s(cs.means.elev_mean_m.clone())),
                ("elev_std_m", f64s(cs.means.elev_std_m.clone())),
                ("tri_m", f64s(cs.means.tri_m.clone())),
            ],
        )?;

        let index: BTreeMap<&str, i64> = codes
            .iter()
            .enumerate()
            .map(|(i, c)| (c.as_str(), i as i64))
            .collect();
        let mut attrs = vec![
            ("id", Json::list(provs.iter().map(|p| p.id.as_str()))),
            ("name", Json::list(provs.iter().map(|p| p.name.as_str()))),
            (
                "country",
                Json::list(provs.iter().map(|p| index[p.country.as_str()])),
            ),
            ("area_km2", rounded(provs.iter().map(|p| p.area_km2), 1)),
            ("elev_mean_m", rounded_ints(&ps.means.elev_mean_m)?),
            ("tri_m", rounded(ps.means.tri_m.iter().copied(), 1)),
            ("terrain", ints(&terrain)),
            ("coastal", Json::list(t.coastal.iter().copied())),
            ("dev_slice", Json::list(in_slice)),
            ("ports", ints(&ports)),
            ("airports", ints(&airports)),
            ("rail", Json::list(rail)),
        ];
        for (name, v) in res_names.iter().zip(&res) {
            attrs.push((name, ints(v)));
        }
        attrs.push((
            "countries",
            fields(vec![
                ("code", Json::list(codes.iter())),
                ("name", Json::list(code_names)),
                ("sovereign", Json::list(sovereign_names)),
                ("colour", ints(&colours)),
            ]),
        ));
        pack.text("web/provinces.json", &fields(attrs).text())?;
        let county_attrs = fields(vec![
            ("name", Json::list(cty.iter().map(|c| c.name.as_str()))),
            ("level", Json::list(cty.iter().map(|c| c.level.as_str()))),
            ("province", ints(&clipped.province)),
            ("area_km2", rounded(cty_area.iter().copied(), 1)),
            ("elev_mean_m", rounded_ints(&cs.means.elev_mean_m)?),
            ("tri_m", rounded(cs.means.tri_m.iter().copied(), 1)),
        ]);
        pack.text("web/counties.json", &county_attrs.text())?;

        let report = econ::build::build(out, out, &cache)?;
        for line in &report.summary {
            eprintln!("{line}");
        }
        pack.tables
            .extend(report.tables.iter().map(|(name, d)| (name.clone(), hex(d))));

        let s = &t.stats;
        let largest = provs
            .iter()
            .map(|p| p.area_km2)
            .fold(f64::NEG_INFINITY, f64::max);
        let split = |rule: &str| {
            provs
                .iter()
                .filter(|p| p.split_rule == rule)
                .map(|p| p.id.split('.').next().unwrap_or_default())
                .collect::<BTreeSet<_>>()
                .len()
        };
        let stats = fields(vec![
            ("land_borders", count(s.land_borders)),
            ("rail_borders", count(s.rail_borders)),
            ("ports_dropped", count(s.ports_dropped)),
            ("ports", count(s.ports)),
            ("minor_ports", count(s.minor_ports)),
            ("airports", count(s.airports)),
            ("air_links", count(s.air_links)),
            ("coastal", count(s.coastal)),
            ("provinces", count(n)),
            ("countries", count(codes.len())),
            ("cells", count(ps.cells.len())),
            ("nodes", count(g.kind.len())),
            ("edges", count(g.edges.len())),
            ("sea_zones", count(t.zones.len())),
            ("largest_province_km2", Json::Int(round_int(largest)?)),
            ("split_by_counties", count(split("counties"))),
            ("split_by_voronoi", count(split("voronoi"))),
            ("counties", count(cty.len())),
            ("counties_unassigned", count(clipped.unassigned)),
            (
                "county_area_outside_province",
                Json::Float(round(1.0 - clipped.fits_inside / clipped.fits, 4)),
            ),
            ("econ", report.stats.json()),
        ]);
        Ok(Built {
            tables: pack.tables,
            stats,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn terrain_classes() {
        let classes: Vec<u8> = [(0.0, 5.0), (800.0, 10.0), (300.0, 60.0), (2500.0, 300.0)]
            .iter()
            .map(|&(e, t)| terrain_class(e, t))
            .collect();
        assert_eq!(classes, [0, 1, 2, 3]);
    }

    #[test]
    fn colouring_takes_the_lowest_free_colour_by_degree_then_code() {
        let codes: Vec<String> = ["A", "B", "C", "D"].map(String::from).to_vec();
        let borders: BTreeSet<(String, String)> = [("A", "B"), ("B", "C"), ("C", "D"), ("B", "D")]
            .iter()
            .map(|&(a, b)| (a.to_owned(), b.to_owned()))
            .collect();
        // B (3 neighbours) 0, then C and D (2 each) by code: C 1, D 2; A 1.
        let colour = colour_countries(&codes, &borders).unwrap();
        assert_eq!(colour.values().copied().collect::<Vec<_>>(), [1, 0, 1, 2]);
        let unknown: BTreeSet<_> = [("A".to_owned(), "Z".to_owned())].into();
        assert!(colour_countries(&codes, &unknown).is_err());
    }

    #[test]
    fn colours_wrap_around_the_palette() {
        // A clique of ten: the ninth by code finds 0 to 7 taken and takes 8, written 0; the
        // tenth finds the same written colours taken.
        let codes: Vec<String> = (0..10).map(|i| format!("C{i}")).collect();
        let mut borders = BTreeSet::new();
        for a in &codes {
            for b in &codes {
                if a < b {
                    borders.insert((a.clone(), b.clone()));
                }
            }
        }
        let colour = colour_countries(&codes, &borders).unwrap();
        assert_eq!(
            colour.values().copied().collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5, 6, 7, 0, 0]
        );
    }

    proptest! {
        #[test]
        fn neighbouring_countries_get_different_colours(
            pairs in proptest::collection::btree_set((0u8..31, 0u8..31), 0..120)
        ) {
            let codes: Vec<String> = (0..31).map(|i| format!("C{i:02}")).collect();
            let borders: BTreeSet<(String, String)> = pairs
                .iter()
                .filter(|(a, b)| a != b)
                .map(|(a, b)| (format!("C{a:02}"), format!("C{b:02}")))
                .collect();
            let colour = colour_countries(&codes, &borders).unwrap();
            let degree = codes
                .iter()
                .map(|c| borders.iter().filter(|(a, b)| a == c || b == c).count())
                .max()
                .unwrap_or(0);
            if degree < PALETTE_SIZE {
                prop_assert!(borders.iter().all(|(a, b)| colour[a] != colour[b]));
            }
        }
    }

    #[test]
    fn rounding_to_an_integer_is_python_s() {
        assert_eq!(round_int(2.5).unwrap(), 2);
        assert_eq!(round_int(3.5).unwrap(), 4);
        assert_eq!(round_int(-0.5).unwrap(), 0);
        assert_eq!(round_int(-1.5).unwrap(), -2);
        assert_eq!(round_int(1234.4999999999998).unwrap(), 1234);
        assert!(round_int(f64::NAN).is_err());
        assert!(round_int(f64::INFINITY).is_err());
    }

    #[cfg(feature = "geos")]
    #[test]
    fn a_code_without_a_name_is_its_own() {
        use full::name_of;
        let names: BTreeMap<String, Option<String>> = [
            ("FRA".to_owned(), Some("France".to_owned())),
            ("XXX".to_owned(), None),
        ]
        .into();
        assert_eq!(name_of(&names, "FRA").unwrap(), "France");
        assert_eq!(name_of(&names, "YUG").unwrap(), "YUG");
        assert!(name_of(&names, "XXX").is_err());
    }
}
