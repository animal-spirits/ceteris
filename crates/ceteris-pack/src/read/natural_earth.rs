//! Natural Earth admin-0 and admin-1 attributes, from the dBASE table of the zipped shapefile.

use std::collections::BTreeMap;
use std::path::Path;

use super::{err, int, read_member};

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

/// ISO_A3_EH by each ADMIN, NAME and NAME_LONG of admin-0, the first country to use a name keeping it.
pub fn country_names(admin0: &Path) -> Result<BTreeMap<String, String>, String> {
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
