//! Natural Earth attributes, from the dBASE table of a zipped shapefile, and shapes, from its
//! `.shp` member.

use std::collections::BTreeMap;
use std::path::Path;

use geo::{
    BoundingRect, Coord, Geometry, LineString, MultiLineString, MultiPolygon, Point, Polygon, Rect,
};

use super::{err, int, read_member};

pub const ADMIN0: &str = "ne_10m_admin_0_countries.zip";
pub const ADMIN1: &str = "ne_10m_admin_1_states_provinces.zip";

/// Natural Earth codes that differ from the ISO 3166-1 alpha-3 code the data sources use.
pub const EXTRA_ISO: [(&str, &str); 1] = [("KOS", "XKX")];
/// Admin-1 name fields a DOSE region name may equal, the primary name first; `|` separates alternates.
pub const NAME_FIELDS: [&str; 7] = [
    "name",
    "name_en",
    "name_alt",
    "woe_name",
    "gn_name",
    "gns_name",
    "name_local",
];

/// The text columns of a dBASE table: values up to any NUL and without surrounding spaces, None
/// where blank, as GDAL reads them. Deleted records are skipped.
pub struct Dbf {
    columns: BTreeMap<String, usize>,
    rows: Vec<Vec<Option<String>>>,
}

impl Dbf {
    /// The `.dbf` member of the zipped shapefile at `path`.
    pub fn from_zip(path: &Path) -> Result<Dbf, String> {
        let bytes = read_member(path, |n| n.to_ascii_lowercase().ends_with(".dbf"))?;
        Dbf::parse(&bytes).map_err(|e| err(path, e))
    }

    pub fn parse(b: &[u8]) -> Result<Dbf, String> {
        let le16 = |i: usize| usize::from(u16::from_le_bytes([b[i], b[i + 1]]));
        if b.len() < 32 {
            return Err("truncated dBASE header".into());
        }
        let records = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
        let (header, record) = (le16(8), le16(10));
        let mut fields = Vec::new();
        let mut at = 32;
        while at + 32 <= header && b[at] != 0x0d {
            let name = &b[at..at + 11];
            let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(11)];
            fields.push((
                String::from_utf8_lossy(name).into_owned(),
                usize::from(b[at + 16]),
            ));
            at += 32;
        }
        if header + records * record > b.len() {
            return Err("truncated dBASE records".into());
        }
        let mut rows = Vec::with_capacity(records);
        for r in 0..records {
            let rec = &b[header + r * record..header + (r + 1) * record];
            if rec[0] == b'*' {
                continue;
            }
            let mut offset = 1;
            let mut row = Vec::with_capacity(fields.len());
            for (name, width) in &fields {
                let raw = &rec[offset..offset + width];
                offset += width;
                // GDAL reads a field as a C string: a NUL ends it.
                let raw = &raw[..raw.iter().position(|&c| c == 0).unwrap_or(raw.len())];
                let text = std::str::from_utf8(raw).map_err(|e| format!("{name}: {e}"))?;
                let text = text.trim_matches(' ');
                row.push((!text.is_empty()).then(|| text.to_owned()));
            }
            rows.push(row);
        }
        let columns = fields
            .into_iter()
            .enumerate()
            .map(|(i, (n, _))| (n, i))
            .collect();
        Ok(Dbf { columns, rows })
    }

    /// Values of the named columns, row by row.
    pub fn select<const N: usize>(
        &self,
        names: [&str; N],
    ) -> Result<Vec<[Option<&str>; N]>, String> {
        let mut idx = [0; N];
        for (slot, name) in idx.iter_mut().zip(names) {
            *slot = *self
                .columns
                .get(name)
                .ok_or_else(|| format!("no field {name}"))?;
        }
        Ok(self
            .rows
            .iter()
            .map(|row| idx.map(|i| row[i].as_deref()))
            .collect())
    }
}

/// ISO alpha-3 by Natural Earth ADM0_A3 (as WDI and ILOSTAT write them).
pub type IsoCodes = BTreeMap<String, String>;

/// Natural Earth ADM0_A3 to ISO alpha-3, and ISO alpha-3 to UN M49 numeric code.
pub fn read_codes(admin0: &Path) -> Result<(IsoCodes, BTreeMap<String, i64>), String> {
    let dbf = Dbf::from_zip(admin0)?;
    let (mut iso, mut m49) = (BTreeMap::new(), BTreeMap::new());
    for [a3, own, i3, n3] in dbf
        .select(["ADM0_A3", "ISO_A3", "ISO_A3_EH", "ISO_N3_EH"])
        .map_err(|e| err(admin0, e))?
    {
        let (a3, own, i3, n3) = (
            a3.unwrap_or(""),
            own.unwrap_or(""),
            i3.unwrap_or(""),
            n3.unwrap_or(""),
        );
        // The _EH code fills gaps (France, Norway) but also gives dependencies without a code of their own the parent's.
        if i3 != "-99" && (own != "-99" || a3 == i3) {
            iso.insert(a3.to_owned(), i3.to_owned());
            if n3 != "-99" {
                m49.insert(i3.to_owned(), int(n3, &admin0.display().to_string())?);
            }
        }
    }
    for (a3, i3) in EXTRA_ISO {
        iso.insert(a3.into(), i3.into());
    }
    Ok((iso, m49))
}

/// ISO_A3_EH by ISO_A2_EH of admin-0, for sources keyed by alpha-2 codes (WID); codes of -99
/// left out.
pub fn iso3_by_iso2(admin0: &Path) -> Result<BTreeMap<String, String>, String> {
    let dbf = Dbf::from_zip(admin0)?;
    let mut out = BTreeMap::new();
    for [a2, a3] in dbf
        .select(["ISO_A2_EH", "ISO_A3_EH"])
        .map_err(|e| err(admin0, e))?
    {
        let (a2, a3) = (a2.unwrap_or(""), a3.unwrap_or(""));
        if a2 != "-99" && a3 != "-99" && !a2.is_empty() {
            out.entry(a2.to_owned()).or_insert_with(|| a3.to_owned());
        }
    }
    Ok(out)
}

/// ISO_A3_EH by each ADMIN, NAME and NAME_LONG of admin-0, the first country to use a name keeping it.
pub fn iso_by_name(admin0: &Path) -> Result<BTreeMap<String, String>, String> {
    let dbf = Dbf::from_zip(admin0)?;
    let mut out = BTreeMap::new();
    for [admin, name, long, i3] in dbf
        .select(["ADMIN", "NAME", "NAME_LONG", "ISO_A3_EH"])
        .map_err(|e| err(admin0, e))?
    {
        for n in [admin, name, long] {
            out.entry(n.unwrap_or("").to_owned())
                .or_insert_with(|| i3.unwrap_or("").to_owned());
        }
    }
    Ok(out)
}

/// Natural Earth names of each admin-1 region (adm1_code), primary name first, without repeats.
pub fn province_names(admin1: &Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    let dbf = Dbf::from_zip(admin1)?;
    let [n0, n1, n2, n3, n4, n5, n6] = NAME_FIELDS;
    let mut out = BTreeMap::new();
    for [code, f0, f1, f2, f3, f4, f5, f6] in dbf
        .select(["adm1_code", n0, n1, n2, n3, n4, n5, n6])
        .map_err(|e| err(admin1, e))?
    {
        let mut names: Vec<String> = Vec::new();
        for field in [f0, f1, f2, f3, f4, f5, f6] {
            for n in field
                .unwrap_or("")
                .split('|')
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                if !names.iter().any(|m| m == n) {
                    names.push(n.to_owned());
                }
            }
        }
        out.insert(code.unwrap_or("").to_owned(), names);
    }
    Ok(out)
}

/// Postal state code (from ISO 3166-2) by adm1_code of each United States region.
pub fn us_states(admin1: &Path) -> Result<BTreeMap<String, String>, String> {
    let dbf = Dbf::from_zip(admin1)?;
    let mut out = BTreeMap::new();
    for [code, adm0, iso] in dbf
        .select(["adm1_code", "adm0_a3", "iso_3166_2"])
        .map_err(|e| err(admin1, e))?
    {
        if let (Some(code), Some("USA"), Some(state)) =
            (code, adm0, iso.and_then(|s| s.strip_prefix("US-")))
        {
            out.insert(code.to_owned(), state.to_owned());
        }
    }
    Ok(out)
}

/// The shapes of the `.shp` member of the zipped shapefile at `path`, one per record (None for
/// a null shape), as GDAL's shapefile driver reads them: points, lines (a MultiLineString when
/// a record has more than one part) and polygons.
pub fn shapes_from_zip(path: &Path) -> Result<Vec<Option<Geometry>>, String> {
    let bytes = read_member(path, |n| n.to_ascii_lowercase().ends_with(".shp"))?;
    parse_shapes(&bytes).map_err(|e| err(path, e))
}

pub fn parse_shapes(b: &[u8]) -> Result<Vec<Option<Geometry>>, String> {
    let le32 = |i: usize| i32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let le64 = |i: usize| f64::from_le_bytes(b[i..i + 8].try_into().expect("8 bytes"));
    let count = |i: usize| usize::try_from(le32(i)).map_err(|_| "negative count".to_owned());
    let mut out = Vec::new();
    let mut at = 100;
    while at + 8 <= b.len() {
        let words = i32::from_be_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]);
        let len = 2 * usize::try_from(words).map_err(|_| "negative record length")?;
        let rec = at + 8;
        if rec + len > b.len() || len < 4 {
            return Err("truncated shapefile record".into());
        }
        at = rec + len;
        let parts = || -> Result<Vec<LineString>, String> {
            let (parts, points) = (count(rec + 36)?, count(rec + 40)?);
            let xy = rec + 44 + 4 * parts;
            if xy + 16 * points > at {
                return Err("truncated record".into());
            }
            let mut starts = (0..parts)
                .map(|k| count(rec + 44 + 4 * k))
                .collect::<Result<Vec<_>, _>>()?;
            starts.push(points);
            Ok(starts
                .windows(2)
                .map(|w| {
                    (w[0]..w[1])
                        .map(|i| Coord {
                            x: le64(xy + 16 * i),
                            y: le64(xy + 16 * i + 8),
                        })
                        .collect()
                })
                .collect())
        };
        match le32(rec) {
            0 => out.push(None),
            1 => {
                if rec + 20 > at {
                    return Err("truncated point record".into());
                }
                out.push(Some(Geometry::Point(Point::new(
                    le64(rec + 4),
                    le64(rec + 12),
                ))));
            }
            3 => {
                let mut lines = parts()?;
                out.push(Some(if lines.len() == 1 {
                    Geometry::LineString(lines.remove(0))
                } else {
                    Geometry::MultiLineString(MultiLineString(lines))
                }));
            }
            5 => out.push(Some(organize(parts()?))),
            t => return Err(format!("unsupported shape type {t}")),
        }
    }
    Ok(out)
}

fn shoelace(r: &LineString) -> f64 {
    r.0.windows(2)
        .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
        .sum()
}

/// Whether a ray from `p` towards +x crosses the ring an odd number of times, and whether `p`
/// lies on one of the crossing segments: GDAL's `isPointInRing` and `isPointOnRingBoundary`.
fn crossings(ring: &LineString, p: Coord) -> (bool, bool) {
    let (mut odd, mut on) = (false, false);
    let n = ring.0.len();
    for i in 0..n {
        let (a, b) = (ring.0[i], ring.0[if i == 0 { n - 1 } else { i - 1 }]);
        let (x1, y1, x2, y2) = (a.x - p.x, a.y - p.y, b.x - p.x, b.y - p.y);
        if (y1 > 0.0 && y2 <= 0.0) || (y2 > 0.0 && y1 <= 0.0) {
            let x = (x1 * y2 - x2 * y1) / (y2 - y1);
            odd ^= 0.0 < x;
            on |= x == 0.0;
        }
    }
    (odd, on)
}

fn envelope_contains(a: &Rect, b: &Rect) -> bool {
    a.min().x <= b.min().x
        && a.min().y <= b.min().y
        && a.max().x >= b.max().x
        && a.max().y >= b.max().y
}

/// Rings to polygons as GDAL's `organizePolygons` with `METHOD=ONLY_CCW` does for shapefiles:
/// clockwise rings are shells; a counter-clockwise ring is a hole of the smallest larger shell
/// that contains it, or a shell of its own. Shells and holes keep the record's order.
fn organize(rings: Vec<LineString>) -> Geometry {
    let n = rings.len();
    let clockwise: Vec<bool> = rings.iter().map(|r| shoelace(r) < 0.0).collect();
    if n == 1 || clockwise.iter().filter(|&&c| c).count() == 1 {
        let shell = clockwise.iter().position(|&c| c).unwrap_or(0);
        let mut rings = rings;
        let exterior = rings.remove(shell);
        return Geometry::Polygon(Polygon::new(exterior, rings));
    }
    let envelopes: Vec<Rect> = rings
        .iter()
        .map(|r| {
            r.bounding_rect()
                .unwrap_or(Rect::new(Coord::zero(), Coord::zero()))
        })
        .collect();
    let mut by_area: Vec<usize> = (0..n).collect();
    by_area.sort_by(|&i, &j| {
        shoelace(&rings[j])
            .abs()
            .total_cmp(&shoelace(&rings[i]).abs())
    });
    let mut shell_of: Vec<Option<usize>> = vec![None; n];
    let mut top = vec![false; n];
    top[by_area[0]] = true;
    for (k, &i) in by_area.iter().enumerate().skip(1) {
        if clockwise[i] {
            top[i] = true;
            continue;
        }
        let inside = by_area[..k].iter().rev().find(|&&j| {
            if !envelope_contains(&envelopes[j], &envelopes[i]) {
                return false;
            }
            // From the first vertex of ring i not on ring j's boundary.
            rings[i]
                .0
                .iter()
                .map(|&p| crossings(&rings[j], p))
                .find(|&(_, on)| !on)
                .is_some_and(|(odd, _)| odd)
        });
        match inside {
            Some(&j) if top[j] => shell_of[i] = Some(j),
            _ => top[i] = true,
        }
    }
    let mut polygons: Vec<Option<Polygon>> = vec![None; n];
    let mut rings: Vec<Option<LineString>> = rings.into_iter().map(Some).collect();
    for i in 0..n {
        if top[i] {
            polygons[i] = Some(Polygon::new(rings[i].take().expect("ring"), vec![]));
        }
    }
    for i in 0..n {
        if let Some(j) = shell_of[i] {
            let hole = rings[i].take().expect("ring");
            polygons[j].as_mut().expect("shell").interiors_push(hole);
        }
    }
    let mut polygons: Vec<Polygon> = polygons.into_iter().flatten().collect();
    if polygons.len() == 1 {
        Geometry::Polygon(polygons.remove(0))
    } else {
        Geometry::MultiPolygon(MultiPolygon(polygons))
    }
}
