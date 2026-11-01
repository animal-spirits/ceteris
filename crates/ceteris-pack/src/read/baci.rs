//! CEPII BACI 2000 flows (HS 1992) and the OECD HS to CPA 2.1 conversion key.
//!
//! BACI sums are taken by group within each batch of rows, in row order, and then across
//! batches: a batch is the rows ending in one 1 MiB block of the decompressed member (cut at
//! the block's last line break), the record batches of pyarrow's streaming CSV reader.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;

use csv::StringRecord;

use super::people::parts;
use super::wdi::Names;
use super::xlsx::{self, Cell};
use super::{Csv, cell, err, float, int, read_member, with_member};
use crate::numeric::py_sum;

/// The BACI 2000 member of the package zip.
pub const BACI_2000: &str = "Y2000_V202601.csv";
/// Sheet of the OECD BTiGE HS to CPA 2.1 conversion key.
pub const KEY_SHEET: &str = "BTiGE FromHS-ToCPA-ToENDUSE";
/// Non-monetary gold (HS 7108) is basic metals (C24B), not metal ore.
pub const GOLD: &str = "7108";
/// Printed books and recorded media are goods made by printing and reproduction (C18).
pub const ADAPTED_CPA: [(&str, &str); 2] = [("J58", "C18"), ("J59_60", "C18")];
/// BACI names Taiwan "Other Asia, nes".
pub const BACI_ALIASES: [(&str, &str); 1] = [("S19", "TWN")];
const BLOCK: u64 = 1 << 20;

/// A quantity by HS 6-digit line.
pub type ByLine = BTreeMap<String, f64>;

/// Keys in order of first appearance with their running sums.
struct Ordered<K: Ord + Clone, const N: usize> {
    keys: Vec<K>,
    sums: Vec<[f64; N]>,
    index: BTreeMap<K, usize>,
}

impl<K: Ord + Clone, const N: usize> Ordered<K, N> {
    fn new() -> Self {
        Ordered {
            keys: Vec::new(),
            sums: Vec::new(),
            index: BTreeMap::new(),
        }
    }

    fn add(&mut self, key: &K, values: [f64; N]) {
        let i = *self.index.entry(key.clone()).or_insert_with(|| {
            self.keys.push(key.clone());
            self.sums.push([0.0; N]);
            self.keys.len() - 1
        });
        for (s, v) in self.sums[i].iter_mut().zip(values) {
            *s += v;
        }
    }

    fn drain(&mut self) -> Vec<(K, [f64; N])> {
        self.index.clear();
        self.keys.drain(..).zip(self.sums.drain(..)).collect()
    }
}

/// Reads the rows of a BACI CSV, calling `f` with each row (its cells of `columns`) and with
/// None at the end of each batch.
fn batches(
    input: &mut dyn Read,
    origin: &Path,
    columns: &[&str],
    mut f: impl FnMut(Option<(&StringRecord, &[usize])>) -> Result<(), String>,
) -> Result<(), String> {
    let mut csv = Csv::new(input, origin)?;
    let cols = columns
        .iter()
        .map(|c| csv.col(c))
        .collect::<Result<Vec<_>, _>>()?;
    let mut record = StringRecord::new();
    let mut batch = 0;
    while csv.next(&mut record)? {
        let here = (csv.offset() - 1) / BLOCK;
        if here != batch {
            f(None)?;
            batch = here;
        }
        f(Some((&record, &cols)))?;
    }
    f(None)
}

/// pyarrow's float column value: None for its null spellings.
fn nullable(text: &str, nulls: &[&str], origin: &str) -> Result<Option<f64>, String> {
    if nulls.contains(&text) {
        Ok(None)
    } else {
        float(text, origin).map(Some)
    }
}

/// pyarrow's default null spellings.
const NULLS: [&str; 17] = [
    "", "#N/A", "#N/A N/A", "#NA", "-1.#IND", "-1.#QNAN", "-NaN", "-nan", "1.#IND", "1.#QNAN",
    "N/A", "NA", "NULL", "NaN", "n/a", "nan", "null",
];

fn lpad6(k: &str) -> String {
    let n = k.chars().count();
    if n >= 6 {
        k.to_owned()
    } else {
        "0".repeat(6 - n) + k
    }
}

/// Tonnes and value (USD) by HS 6-digit line over the 2000 flows with a reported quantity.
///
/// BACI gives value v in thousands of USD and quantity q in metric tons; rows with missing q
/// are left out of both sums so that their ratio is a weight per value.
pub fn baci_by_product(baci: &Path) -> Result<(ByLine, ByLine), String> {
    let origin = baci.display().to_string();
    let (mut tonnes, mut value) = (BTreeMap::new(), BTreeMap::new());
    let mut groups: Ordered<String, 2> = Ordered::new();
    with_member(
        baci,
        |n| n.ends_with(BACI_2000),
        |member| {
            batches(member, baci, &["k", "v", "q"], |row| {
                let Some((r, c)) = row else {
                    for (k, [q, v]) in groups.drain() {
                        *tonnes.entry(k.clone()).or_insert(0.0) += q;
                        *value.entry(k).or_insert(0.0) += v * 1000.0;
                    }
                    return Ok(());
                };
                let Some(q) = nullable(cell(r, c[2]), &["", "NA"], &origin)? else {
                    return Ok(());
                };
                let v = nullable(cell(r, c[1]), &["", "NA"], &origin)?
                    .ok_or_else(|| format!("{origin}: no v"))?;
                groups.add(&lpad6(cell(r, c[0])), [q, v]);
                Ok(())
            })
        },
    )?;
    Ok((tonnes, value))
}

/// Trade value (USD) by HS 6-digit line over every 2000 flow.
pub fn baci_values(baci: &Path) -> Result<ByLine, String> {
    let origin = baci.display().to_string();
    let mut value = BTreeMap::new();
    let mut groups: Ordered<String, 1> = Ordered::new();
    with_member(
        baci,
        |n| n.ends_with(BACI_2000),
        |member| {
            batches(member, baci, &["k", "v"], |row| {
                let Some((r, c)) = row else {
                    for (k, [v]) in groups.drain() {
                        *value.entry(lpad6(&k)).or_insert(0.0) += v * 1000.0;
                    }
                    return Ok(());
                };
                if let Some(v) = nullable(cell(r, c[1]), &NULLS, &origin)? {
                    groups.add(&cell(r, c[0]).to_owned(), [v]);
                }
                Ok(())
            })
        },
    )?;
    Ok(value)
}

/// BACI's numeric country codes and their ISO3 codes (`country_codes` of the package; the last
/// row of a repeated code wins).
pub fn country_codes(baci: &Path) -> Result<BTreeMap<i64, String>, String> {
    let origin = baci.display().to_string();
    let table = read_member(baci, |n| n.starts_with("country_codes"))?;
    let mut csv = Csv::new(table.as_slice(), baci)?;
    let (code, iso3) = (csv.col("country_code")?, csv.col("country_iso3")?);
    let mut numeric = BTreeMap::new();
    let mut row = StringRecord::new();
    while csv.next(&mut row)? {
        numeric.insert(int(cell(&row, code), &origin)?, cell(&row, iso3).to_owned());
    }
    Ok(numeric)
}

/// One 2000 BACI flow: exporter and importer codes, HS 6-digit line and value in thousands of
/// USD (NaN where empty).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    pub i: i64,
    pub j: i64,
    pub k: i64,
    pub v: f64,
}

/// Calls `f` with every 2000 flow in file order and with None at the end of each batch.
pub fn for_each_flow(baci: &Path, mut f: impl FnMut(Option<Flow>)) -> Result<(), String> {
    let origin = baci.display().to_string();
    with_member(
        baci,
        |n| n.ends_with(BACI_2000),
        |member| {
            batches(member, baci, &["i", "j", "k", "v"], |row| {
                let Some((r, c)) = row else {
                    f(None);
                    return Ok(());
                };
                f(Some(Flow {
                    i: int(cell(r, c[0]), &origin)?,
                    j: int(cell(r, c[1]), &origin)?,
                    k: int(cell(r, c[2]), &origin)?,
                    v: nullable(cell(r, c[3]), &NULLS, &origin)?.unwrap_or(f64::NAN),
                }));
                Ok(())
            })
        },
    )
}

/// 2000 exports plus imports (USD) of each pack country in `codes` from BACI, summing the
/// economies a country combines.
pub fn trade_by_country(
    baci: &Path,
    iso: &Names,
    codes: &[String],
) -> Result<BTreeMap<String, f64>, String> {
    let origin = baci.display().to_string();
    let numeric = country_codes(baci)?;
    let mut total: Ordered<i64, 1> = Ordered::new();
    let (mut by_i, mut by_j): (Ordered<i64, 1>, Ordered<i64, 1>) = (Ordered::new(), Ordered::new());
    with_member(
        baci,
        |n| n.ends_with(BACI_2000),
        |member| {
            batches(member, baci, &["i", "j", "v"], |row| {
                let Some((r, c)) = row else {
                    for side in [&mut by_i, &mut by_j] {
                        for (k, [v]) in side.drain() {
                            total.add(&k, [v * 1000.0]);
                        }
                    }
                    return Ok(());
                };
                let v = nullable(cell(r, c[2]), &NULLS, &origin)?
                    .ok_or_else(|| format!("{origin}: no v"))?;
                by_i.add(&int(cell(r, c[0]), &origin)?, [v]);
                by_j.add(&int(cell(r, c[1]), &origin)?, [v]);
                Ok(())
            })
        },
    )?;

    let mut by_iso: Ordered<Option<String>, 1> = Ordered::new();
    for (k, [v]) in total.drain() {
        let code = numeric.get(&k).map(|c| {
            BACI_ALIASES
                .iter()
                .find(|(a, _)| a == c)
                .map_or(c.clone(), |(_, b)| (*b).to_owned())
        });
        by_iso.add(&code, [v]);
    }
    let mut by_iso: BTreeMap<Option<String>, f64> =
        by_iso.drain().into_iter().map(|(k, [v])| (k, v)).collect();
    let scg = by_iso.remove(&Some("SCG".into())).unwrap_or(0.0);
    *by_iso.entry(Some("SRB".into())).or_insert(0.0) += scg;
    let mut out = BTreeMap::new();
    for c in codes {
        let value = py_sum(
            parts(c, iso)
                .iter()
                .map(|p| by_iso.get(&Some(p.clone())).copied().unwrap_or(0.0)),
        );
        if value > 0.0 {
            out.insert(c.clone(), value);
        }
    }
    Ok(out)
}

/// OECD BTiGE conversion key (2025): HS 1988/92 (key version 0) 6-digit line to CPA 2.1
/// activity, gold moved to C24B and books and media to C18.
pub fn read_hs_cpa(path: &Path) -> Result<Names, String> {
    read_hs_cpa_version(path, 0)
}

/// [`read_hs_cpa`] for the key's HS `version`: 0 HS 1988/92, 1 HS 1996, 2 HS 2002 (UN H0 to H2).
pub fn read_hs_cpa_version(path: &Path, version: u8) -> Result<Names, String> {
    let origin = path.display().to_string();
    let mut key = Names::new();
    for r in xlsx::sheet(path, KEY_SHEET)? {
        let row_version = r
            .get("HS-version")
            .ok_or_else(|| format!("{origin}: row without HS-version"))?;
        if *row_version == Cell::Number(f64::from(version)) {
            let text = |name: &str| {
                r.get(name)
                    .map(Cell::to_text)
                    .ok_or_else(|| err(path, format!("row without {name}")))
            };
            key.insert(text("HS-code")?, text("CPA")?);
        }
    }
    Ok(key
        .into_iter()
        .map(|(hs, code)| {
            let cpa = if hs.starts_with(GOLD) {
                "C24B".to_owned()
            } else {
                ADAPTED_CPA
                    .iter()
                    .find(|(a, _)| *a == code)
                    .map_or(code, |(_, b)| (*b).to_owned())
            };
            (hs, cpa)
        })
        .collect())
}
