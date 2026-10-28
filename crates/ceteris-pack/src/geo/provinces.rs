//! Province set: Natural Earth admin-1 at 2000 borders, oversized regions split (Section 5.2,
//! ADR 0006).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use geo::{Geometry, MakeValid, MultiPolygon, Validation};
use toml::Value;

use crate::read::natural_earth::{Dbf, shapes_from_zip};
use crate::sources::{CONFIG_DIR, read_table, workspace_root};

/// A county split needs at least this many counties per part to balance areas.
pub const MIN_COUNTIES_PER_PART: usize = 2;
/// A county split whose largest part exceeds this multiple of the area limit falls back to Voronoi.
pub const MAX_PART_FACTOR: f64 = 1.5;

#[derive(Clone, Debug)]
pub struct Province {
    /// Natural Earth adm1_code, with ".k" for the k-th part of a split region.
    pub id: String,
    pub name: String,
    pub country: String,
    pub sovereign: String,
    pub country_now: String,
    /// A Polygon or MultiPolygon.
    pub geom: Geometry,
    pub area_km2: f64,
    pub lon: f64,
    pub lat: f64,
    /// "counties" or "voronoi" for parts of a split region.
    pub split_rule: String,
}

#[cfg(feature = "geos")]
impl Province {
    fn part(&self, id: String, name: String, geom: Geometry) -> Province {
        Province {
            id,
            name,
            country: self.country.clone(),
            sovereign: self.sovereign.clone(),
            country_now: self.country_now.clone(),
            geom,
            area_km2: 0.0,
            lon: 0.0,
            lat: 0.0,
            split_rule: String::new(),
        }
    }
}

/// The polygons of a Polygon or MultiPolygon, as `shapely.get_parts` lists them; an empty
/// polygon has none.
pub fn parts(g: &Geometry) -> MultiPolygon {
    match g {
        Geometry::Polygon(p) if p.exterior().0.is_empty() => MultiPolygon(vec![]),
        Geometry::Polygon(p) => MultiPolygon(vec![p.clone()]),
        Geometry::MultiPolygon(m) => m.clone(),
        _ => MultiPolygon(vec![]),
    }
}

/// `borders_2000.toml`: successor states and transferred regions mapped to their 2000 country.
#[derive(Debug, Default)]
pub struct Borders {
    pub exclude: BTreeSet<String>,
    pub country: BTreeMap<String, String>,
    pub province: BTreeMap<String, String>,
    pub sovereign: BTreeMap<String, String>,
}

pub fn borders() -> Result<Borders, String> {
    let path = workspace_root().join(CONFIG_DIR).join("borders_2000.toml");
    let t = read_table(&path)?;
    let map = |key: &str| -> BTreeMap<String, String> {
        t.get(key)
            .and_then(Value::as_table)
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
            .collect()
    };
    let exclude = t
        .get("exclude")
        .and_then(|e| e.get("countries"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_owned))
        .collect();
    Ok(Borders {
        exclude,
        country: map("country"),
        province: map("province"),
        sovereign: map("sovereign"),
    })
}

/// Number of parts an oversized province is split into.
pub fn split_count(area_km2: f64, max_area_km2: f64) -> usize {
    // A float to integer cast saturates; areas are finite and far below usize::MAX.
    ((area_km2 / max_area_km2).ceil() as usize).max(1)
}

/// Recursive coordinate bisection of `order` into k groups of near-equal size (or weight);
/// deterministic.
pub fn bisect(
    points: &[[f64; 2]],
    order: Vec<usize>,
    k: usize,
    weights: Option<&[f64]>,
) -> Vec<Vec<usize>> {
    if k == 1 {
        return vec![order];
    }
    let ptp = |axis: usize| {
        let v = order.iter().map(|&i| points[i][axis]);
        v.clone().fold(f64::NEG_INFINITY, f64::max) - v.fold(f64::INFINITY, f64::min)
    };
    let axis = usize::from(ptp(1) > ptp(0));
    let mut ranked = order;
    ranked.sort_by(|&a, &b| {
        points[a][axis]
            .partial_cmp(&points[b][axis])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.cmp(&b))
    });
    let k1 = k / 2;
    let cut = match weights {
        None => ((ranked.len() * k1) as f64 / k as f64).round_ties_even() as usize,
        Some(w) => {
            let mut cum = Vec::with_capacity(ranked.len());
            let mut total = 0.0;
            for &i in &ranked {
                total += w[i];
                cum.push(total);
            }
            let target = total * k1 as f64 / k as f64;
            cum.partition_point(|&c| c < target) + 1
        }
    };
    let cut = cut.max(k1).min(ranked.len() - (k - k1));
    let rest = ranked.split_off(cut);
    let mut out = bisect(points, ranked, k1, weights);
    out.extend(bisect(points, rest, k - k1, weights));
    out
}

/// Natural Earth sov_a3 (which uses codes like GB1, NL1) to the sovereign's adm0_a3.
fn sovereign_codes(admin0: &Path) -> Result<BTreeMap<Option<String>, Option<String>>, String> {
    let dbf = Dbf::from_zip(admin0)?;
    let rows = dbf.select(["SOV_A3", "SOVEREIGNT", "ADMIN", "ADM0_A3"])?;
    let own = |s: Option<&str>| s.map(str::to_owned);
    let by_name: BTreeMap<_, _> = rows.iter().map(|r| (own(r[2]), own(r[3]))).collect();
    Ok(rows
        .iter()
        .map(|r| {
            let code = by_name.get(&own(r[1])).cloned().unwrap_or(own(r[0]));
            (own(r[0]), code)
        })
        .collect())
}

/// shapely's `make_valid` where a geometry is invalid: geo's repair, which matches GEOS on the
/// admin-1 polygons; a single part stays a Polygon.
pub fn make_valid(g: Geometry) -> Result<Geometry, String> {
    if g.is_valid() {
        return Ok(g);
    }
    let mut repaired = match &g {
        Geometry::Polygon(p) => p.make_valid(),
        Geometry::MultiPolygon(m) => m.make_valid(),
        g => return Err(format!("not polygonal: {g:?}")),
    }
    .map_err(|e| format!("{e:?}"))?;
    Ok(if repaired.0.len() == 1 {
        Geometry::Polygon(repaired.0.remove(0))
    } else {
        Geometry::MultiPolygon(repaired)
    })
}

/// The admin-1 regions at 2000 borders, ordered by (country, id).
pub fn load(admin1: &Path, admin0: &Path) -> Result<Vec<Province>, String> {
    let b = borders()?;
    let sov = sovereign_codes(admin0)?;
    let dbf = Dbf::from_zip(admin1)?;
    let rows = dbf.select(["adm1_code", "name", "adm0_a3", "sov_a3"])?;
    let shapes = shapes_from_zip(admin1)?;
    if rows.len() != shapes.len() {
        return Err(format!(
            "{}: {} records but {} shapes",
            admin1.display(),
            rows.len(),
            shapes.len()
        ));
    }
    let mut out = Vec::new();
    for ([code, name, now, sov_a3], shape) in rows.into_iter().zip(shapes) {
        let (code, now) = (code.unwrap_or_default(), now.unwrap_or_default());
        if b.exclude.contains(now) {
            continue;
        }
        let country = b
            .province
            .get(code)
            .or_else(|| b.country.get(now))
            .map_or(now, String::as_str)
            .to_owned();
        // A remapped province is under its 2000 country's sovereignty.
        let default = if country != now {
            country.clone()
        } else {
            let key = sov_a3.map(str::to_owned);
            sov.get(&key).cloned().unwrap_or(key).unwrap_or_default()
        };
        let sovereign = b.sovereign.get(&country).cloned().unwrap_or(default);
        let geom = shape.ok_or_else(|| format!("{code}: no geometry"))?;
        out.push(Province {
            id: code.to_owned(),
            name: name.unwrap_or(code).to_owned(),
            country,
            sovereign,
            country_now: now.to_owned(),
            geom: make_valid(geom)?,
            area_km2: 0.0,
            lon: 0.0,
            lat: 0.0,
            split_rule: String::new(),
        });
    }
    out.sort_by(|a, b| (&a.country, &a.id).cmp(&(&b.country, &b.id)));
    Ok(out)
}

#[cfg(feature = "geos")]
pub use split::{build, split, split_by_counties};

#[cfg(feature = "geos")]
mod split {
    use geo::{BoundingRect, Coord, Geometry, Rect};
    use geos::Geometry as Geos;
    use h3o::{LatLng, Resolution};

    use super::{MAX_PART_FACTOR, MIN_COUNTIES_PER_PART, Province, bisect, parts, split_count};
    use crate::geo::counties::{County, assign};
    use crate::geo::overlay::{
        self, difference, from_geo, intersection, is_empty, polygonal, to_geo, transform, union_all,
    };
    use crate::geo::raster_terrain::cells_of;
    use crate::geo::sphere::{anchor, area_km2, cos_lat, crosses_antimeridian};
    use crate::numeric::pairwise_sum;

    fn mean(v: &[f64]) -> f64 {
        pairwise_sum(v) / v.len() as f64
    }

    fn area(g: &Geos) -> Result<f64, String> {
        Ok(area_km2(&overlay::to_multipolygon(g)?))
    }

    /// GEOS's Voronoi diagram of the seeds with `extend_to` of envelope `extend`, its cells in
    /// seed order (shapely's `ordered=True`).
    fn voronoi(seeds: &[Coord], extend: Rect) -> Result<Vec<Geos>, String> {
        let fail = |e: geos::Error| e.to_string();
        let points = seeds
            .iter()
            .map(|c| {
                geos::CoordSeq::new_from_buffer(&[c.x, c.y], 1, geos::CoordType::XY)
                    .and_then(Geos::create_point)
                    .map_err(fail)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bounds = overlay::rectangle(
            extend.min().x,
            extend.min().y,
            extend.max().x,
            extend.max().y,
        )?;
        let diagram = Geos::create_multipoint(
            points
                .iter()
                .map(|p| geos::Geom::clone(p).map_err(fail))
                .collect::<Result<_, _>>()?,
        )
        .and_then(|m| geos::Geom::voronoi(&m, Some(&bounds), 0.0, false))
        .map_err(fail)?;
        let cells = overlay::parts(&diagram)?;
        points
            .iter()
            .map(|p| {
                for c in &cells {
                    if geos::Geom::contains(c, p).map_err(fail)? {
                        return geos::Geom::clone(c).map_err(fail);
                    }
                }
                Err("a Voronoi seed outside every cell".to_owned())
            })
            .collect()
    }

    /// Voronoi split for regions without county data (ADR 0006).
    pub fn split(p: &Province, k: usize, res: Resolution) -> Result<Vec<Province>, String> {
        let wrapped = crosses_antimeridian(&parts(&p.geom));
        let g = from_geo(&p.geom)?;
        let g = if wrapped { overlay::unwrap(&g)? } else { g };
        let cells = cells_of(&parts(&p.geom), res)?;
        if cells.len() < k {
            return Ok(vec![p.clone()]);
        }
        let mut ll: Vec<[f64; 2]> = cells
            .iter()
            .map(|&c| {
                let c = LatLng::from(c);
                [c.lng(), c.lat()]
            })
            .collect();
        if wrapped {
            for c in &mut ll {
                if c[0] < 0.0 {
                    c[0] += 360.0;
                }
            }
        }
        // Bisect and draw Voronoi boundaries in a locally equal-distance plane.
        let lat: Vec<f64> = ll.iter().map(|c| c[1]).collect();
        let scale = cos_lat(mean(&lat));
        let xy: Vec<[f64; 2]> = ll.iter().map(|c| [c[0] * scale, c[1] * 1.0]).collect();
        let groups = bisect(&xy, (0..xy.len()).collect(), k, None);
        let seeds: Vec<Coord> = groups
            .iter()
            .map(|g| {
                let (mut x, mut y) = (0.0, 0.0);
                for &i in g {
                    x += xy[i][0];
                    y += xy[i][1];
                }
                let n = g.len() as f64;
                Coord { x: x / n, y: y / n }
            })
            .collect();
        let plane = transform(&g, |x, y| (x * scale, y * 1.0))?;
        let env = overlay::to_multipolygon(&plane)?
            .bounding_rect()
            .ok_or("an empty province")?;
        let extend = Rect::new(
            Coord {
                x: env.min().x - 1.0,
                y: env.min().y - 1.0,
            },
            Coord {
                x: env.max().x + 1.0,
                y: env.max().y + 1.0,
            },
        );
        let mut out = Vec::new();
        for (i, region) in voronoi(&seeds, extend)?.iter().enumerate() {
            let cut = intersection(&plane, region)?;
            let piece = polygonal(&transform(&cut, |x, y| (x / scale, y / 1.0))?)?;
            let piece = if wrapped {
                overlay::wrap(&piece)?
            } else {
                piece
            };
            if is_empty(&piece)? {
                continue;
            }
            let i = i + 1;
            out.push(p.part(
                format!("{}.{i}", p.id),
                format!("{} ({i})", p.name),
                to_geo(&piece)?,
            ));
        }
        Ok(out)
    }

    /// Split along county borders: bisect the counties into k groups of equal area, then give
    /// each gap between the sources' borders to the part it shares most border with. Counties
    /// too large to balance are first cut by Voronoi, inside themselves. None if counties cannot
    /// carry the split.
    pub fn split_by_counties(
        p: &Province,
        k: usize,
        members: &[&Geos],
        max_area_km2: f64,
        res: Resolution,
    ) -> Result<Option<Vec<Province>>, String> {
        let geom = from_geo(&p.geom)?;
        let mut clipped = Vec::new();
        for m in members {
            let x = polygonal(&intersection(*m, &geom)?)?;
            if !is_empty(&x)? {
                clipped.push(x);
            }
        }
        if clipped.len() < MIN_COUNTIES_PER_PART * k {
            return Ok(None);
        }
        let mut pieces = Vec::new();
        for x in clipped {
            let n = split_count(area(&x)?, max_area_km2 / 2.0);
            if n > 1 {
                let whole = p.part(String::new(), String::new(), to_geo(&x)?);
                for q in split(&whole, n, res)? {
                    pieces.push(from_geo(&q.geom)?);
                }
            } else {
                pieces.push(x);
            }
        }
        let shapes = pieces
            .iter()
            .map(overlay::to_multipolygon)
            .collect::<Result<Vec<_>, _>>()?;
        let mut ll = shapes
            .iter()
            .map(|x| {
                anchor(x)
                    .map(|(x, y)| [x, y])
                    .ok_or("a piece without parts")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if crosses_antimeridian(&parts(&p.geom)) {
            for c in &mut ll {
                if c[0] < 0.0 {
                    c[0] += 360.0;
                }
            }
        }
        let lat: Vec<f64> = ll.iter().map(|c| c[1]).collect();
        let scale = cos_lat(mean(&lat));
        let xy: Vec<[f64; 2]> = ll.iter().map(|c| [c[0] * scale, c[1] * 1.0]).collect();
        let weight: Vec<f64> = shapes.iter().map(area_km2).collect();
        let groups = bisect(&xy, (0..pieces.len()).collect(), k, Some(&weight));
        let mut groups_of: Vec<Vec<Geos>> = Vec::with_capacity(k);
        for g in &groups {
            let members = g
                .iter()
                .map(|&i| geos::Geom::clone(&pieces[i]).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
            groups_of.push(vec![union_all(members)?]);
        }
        let firsts = groups_of
            .iter()
            .map(|q| geos::Geom::clone(&q[0]).map_err(|e| e.to_string()))
            .collect::<Result<_, _>>()?;
        let covered = union_all(firsts)?;
        let boundaries = groups_of
            .iter()
            .map(|q| geos::Geom::boundary(&q[0]).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        for gap in overlay::parts(&polygonal(&difference(&geom, &covered)?)?)? {
            let edge = geos::Geom::boundary(&gap).map_err(|e| e.to_string())?;
            let shared = boundaries
                .iter()
                .map(|b| {
                    intersection(&edge, b)
                        .and_then(|l| geos::Geom::length(&l).map_err(|e| e.to_string()))
                })
                .collect::<Result<Vec<f64>, _>>()?;
            let j = if shared.iter().any(|&s| s > 0.0) {
                argmax(&shared)
            } else {
                let d = groups_of
                    .iter()
                    .map(|q| geos::Geom::distance(&gap, &q[0]).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<f64>, _>>()?;
                argmin(&d)
            };
            groups_of[j].push(gap);
        }
        let mut out = Vec::with_capacity(k);
        for (i, q) in groups_of.into_iter().enumerate() {
            let i = i + 1;
            let g = polygonal(&union_all(q)?)?;
            out.push(p.part(
                format!("{}.{i}", p.id),
                format!("{} ({i})", p.name),
                to_geo(&g)?,
            ));
        }
        let largest = out
            .iter()
            .map(|q| area_km2(&parts(&q.geom)))
            .fold(f64::NEG_INFINITY, f64::max);
        Ok((largest <= MAX_PART_FACTOR * max_area_km2).then_some(out))
    }

    /// numpy's argmax: the first largest.
    fn argmax(v: &[f64]) -> usize {
        let mut best = 0;
        for (i, &x) in v.iter().enumerate() {
            if x > v[best] {
                best = i;
            }
        }
        best
    }

    /// numpy's argmin: the first smallest.
    fn argmin(v: &[f64]) -> usize {
        let mut best = 0;
        for (i, &x) in v.iter().enumerate() {
            if x < v[best] {
                best = i;
            }
        }
        best
    }

    /// Provinces at 2000 borders; oversized ones split along county borders where `counties`
    /// cover them, by Voronoi otherwise.
    pub fn build(
        admin1: &std::path::Path,
        admin0: &std::path::Path,
        max_area_km2: f64,
        res: Resolution,
        counties: &[County],
    ) -> Result<Vec<Province>, String> {
        let mut base = super::load(admin1, admin0)?;
        for p in &mut base {
            p.area_km2 = area_km2(&parts(&p.geom));
        }
        let owner = if counties.is_empty() {
            vec![]
        } else {
            let geoms: Vec<Geometry> = base.iter().map(|p| p.geom.clone()).collect();
            let countries: Vec<[&str; 2]> = base
                .iter()
                .map(|p| [p.country.as_str(), p.country_now.as_str()])
                .collect();
            assign(counties, &geoms, &countries)?
        };
        let mut out = Vec::new();
        for (i, p) in base.into_iter().enumerate() {
            let k = split_count(p.area_km2, max_area_km2);
            if k == 1 {
                out.push(p);
                continue;
            }
            let members: Vec<&Geos> = counties
                .iter()
                .zip(&owner)
                .filter(|(_, o)| **o == Some(i))
                .map(|(c, _)| &c.geom)
                .collect();
            let (rule, split_parts) = match split_by_counties(&p, k, &members, max_area_km2, res)? {
                Some(parts) => ("counties", parts),
                None => ("voronoi", split(&p, k, res)?),
            };
            for mut q in split_parts {
                q.split_rule = rule.to_owned();
                out.push(q);
            }
        }
        for p in &mut out {
            let shape = parts(&p.geom);
            p.area_km2 = area_km2(&shape);
            (p.lon, p.lat) = anchor(&shape).ok_or_else(|| format!("{}: no parts", p.id))?;
        }
        Ok(out)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::geo::overlay::rectangle;
        use geos::Geom;

        fn province(g: Geometry) -> Province {
            Province {
                id: "X-1".into(),
                name: "X".into(),
                country: "XXX".into(),
                sovereign: "XXX".into(),
                country_now: "XXX".into(),
                geom: g,
                area_km2: 0.0,
                lon: 0.0,
                lat: 0.0,
                split_rule: String::new(),
            }
        }

        fn planar(g: &Geometry) -> f64 {
            from_geo(g).unwrap().area().unwrap()
        }

        #[test]
        fn county_split_follows_county_borders_and_covers_the_province() {
            let p = province(to_geo(&rectangle(0.0, 0.0, 4.0, 4.0).unwrap()).unwrap());
            let grid: Vec<Geos> = (0..4)
                .flat_map(|x| (0..4).map(move |y| (f64::from(x), f64::from(y))))
                .map(|(x, y)| rectangle(x, y, x + 1.0, y + 1.0).unwrap())
                .collect();
            let refs: Vec<&Geos> = grid.iter().collect();
            let parts = split_by_counties(&p, 4, &refs, 1e9, Resolution::Four)
                .unwrap()
                .unwrap();
            assert_eq!(parts.len(), 4);
            let all =
                union_all(parts.iter().map(|q| from_geo(&q.geom).unwrap()).collect()).unwrap();
            let whole = from_geo(&p.geom).unwrap();
            assert!(all.sym_difference(&whole).unwrap().area().unwrap() < 1e-9);
            for q in &parts {
                assert!((planar(&q.geom) - 4.0).abs() < 1e-12);
                // Every part is a union of whole counties.
                let g = from_geo(&q.geom).unwrap();
                for c in &grid {
                    let a = intersection(&g, c).unwrap().area().unwrap();
                    assert!((a - c.area().unwrap()).abs() < 1e-9 || a < 1e-9);
                }
            }
        }

        #[test]
        fn county_split_declines_with_too_few_counties() {
            let p = province(to_geo(&rectangle(0.0, 0.0, 4.0, 4.0).unwrap()).unwrap());
            let halves = [
                rectangle(0.0, 0.0, 4.0, 2.0).unwrap(),
                rectangle(0.0, 2.0, 4.0, 4.0).unwrap(),
            ];
            let refs: Vec<&Geos> = halves.iter().collect();
            assert!(
                split_by_counties(&p, 4, &refs, 1e9, Resolution::Four)
                    .unwrap()
                    .is_none()
            );
        }

        #[test]
        fn voronoi_split_covers_the_province_with_k_parts() {
            let p = province(to_geo(&rectangle(10.0, 40.0, 16.0, 44.0).unwrap()).unwrap());
            let parts = split(&p, 3, Resolution::Four).unwrap();
            assert_eq!(parts.len(), 3);
            let ids: Vec<&str> = parts.iter().map(|q| q.id.as_str()).collect();
            assert_eq!(ids, ["X-1.1", "X-1.2", "X-1.3"]);
            let all =
                union_all(parts.iter().map(|q| from_geo(&q.geom).unwrap()).collect()).unwrap();
            let whole = from_geo(&p.geom).unwrap();
            assert!(all.sym_difference(&whole).unwrap().area().unwrap() < 1e-9);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_2000_borders_remap_successor_states() {
        let b = borders().unwrap();
        assert_eq!(b.country["SDS"], "SDN");
        assert_eq!(b.country["MNE"], "YUG");
        assert_eq!(b.province["RUS-283"], "UKR");
    }

    fn points() -> impl Strategy<Value = Vec<[f64; 2]>> {
        prop::collection::vec(
            (0.0..10.0f64, 0.0..10.0f64).prop_map(|(x, y)| [x, y]),
            1..400,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]

        #[test]
        fn split_count_keeps_parts_under_the_limit(area in 0.0..5e6f64) {
            let k = split_count(area, 250_000.0);
            prop_assert!(k >= 1 && area / k as f64 <= 250_000.0);
        }

        #[test]
        fn bisection_partitions_into_near_equal_groups(pts in points(), k in 1usize..=12) {
            let n = pts.len();
            let k = k.min(n);
            let groups = bisect(&pts, (0..n).collect(), k, None);
            prop_assert_eq!(groups.len(), k);
            let mut all: Vec<usize> = groups.concat();
            all.sort_unstable();
            prop_assert_eq!(all, (0..n).collect::<Vec<_>>());
            let sizes: Vec<usize> = groups.iter().map(Vec::len).collect();
            let spread = sizes.iter().max().unwrap() - sizes.iter().min().unwrap();
            prop_assert!(spread <= k.ilog2() as usize + 1);
            prop_assert_eq!(bisect(&pts, (0..n).collect(), k, None), groups);
        }

        #[test]
        fn weighted_bisection_balances_weight(
            pts in prop::collection::vec((0.0..10.0f64, 0.0..10.0f64, 1.0..10.0f64), 8..300),
            k in 2usize..=8,
        ) {
            let xy: Vec<[f64; 2]> = pts.iter().map(|p| [p.0, p.1]).collect();
            let w: Vec<f64> = pts.iter().map(|p| p.2).collect();
            let groups = bisect(&xy, (0..xy.len()).collect(), k, Some(&w));
            prop_assert!(groups.len() == k && groups.iter().all(|g| !g.is_empty()));
            let mut all: Vec<usize> = groups.concat();
            all.sort_unstable();
            prop_assert_eq!(all, (0..xy.len()).collect::<Vec<_>>());
            let sums: Vec<f64> = groups.iter().map(|g| g.iter().map(|&i| w[i]).sum()).collect();
            let spread = sums.iter().copied().fold(f64::MIN, f64::max)
                - sums.iter().copied().fold(f64::MAX, f64::min);
            // Each cut lands within one item of its target; errors compound over log2(k) levels.
            let wmax = w.iter().copied().fold(f64::MIN, f64::max);
            prop_assert!(spread <= 2.0 * (k.ilog2() as f64 + 1.0) * wmax);
        }
    }
}
