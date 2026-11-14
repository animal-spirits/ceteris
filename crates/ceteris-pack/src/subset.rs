//! `pack subset`: the small uncalibrated pack of RFC 0353 (ADRs 0370, 0374), cut from a full
//! pack by a fixed country list. Kept countries keep their provinces, graph nodes and econ rows;
//! every other country becomes one rest-of-world economy (no provinces, no gateways), aggregated
//! by the per-column rules of the config. The output is a deterministic function of the input
//! pack and the config.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use arrow_array::cast::AsArray;
use arrow_array::types::{Float64Type, UInt8Type, UInt32Type, UInt64Type};
use arrow_array::{
    Array, ArrayRef, BooleanArray, Float64Array, StringArray, UInt8Array, UInt32Array, UInt64Array,
};
use arrow_ipc::reader::FileReader;
use arrow_schema::DataType;
use sha2::{Digest, Sha256};

use crate::table::{self, hex};

const NONE: u32 = u32::MAX;

/// Tables copied unchanged.
const COPIED: [&str; 3] = ["econ_goods.arrow", "sea_zones.arrow", "chokepoints.arrow"];

/// Every table a subset of an uncalibrated pack holds.
const TABLES: [&str; 14] = [
    "provinces.arrow",
    "countries.arrow",
    "cells.arrow",
    "counties.arrow",
    "nodes.arrow",
    "edges.arrow",
    "sea_zones.arrow",
    "chokepoints.arrow",
    "econ_goods.arrow",
    "econ_countries.arrow",
    "econ_io.arrow",
    "econ_provinces.arrow",
    "econ_pops.arrow",
    "econ_tariffs.arrow",
];
const OTHER_TABLES: [&str; 2] = ["gateways.arrow", "econ_home_shares.arrow"];

/// How a column of an aggregated group (the dropped countries) is combined.
#[derive(Debug, Clone, PartialEq)]
pub enum Rule {
    /// Part of the row key (a country index or a plain index).
    Key,
    /// Sum, NaN skipped; NaN if every member is NaN.
    Sum,
    /// Mean weighted by the members' GDP, NaN skipped; unweighted if the weights vanish.
    Mean,
    /// True if any member is true.
    Any,
    /// The first member's value.
    First,
    /// A constant (parsed to the column's type) for aggregated groups.
    Const(String),
}

impl Rule {
    fn parse(s: &str) -> Result<Rule, String> {
        Ok(match s {
            "key" => Rule::Key,
            "sum" => Rule::Sum,
            "mean" => Rule::Mean,
            "any" => Rule::Any,
            "first" => Rule::First,
            _ => Rule::Const(
                s.strip_prefix("const:")
                    .ok_or_else(|| format!("unknown rule '{s}'"))?
                    .to_owned(),
            ),
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub countries: Vec<String>,
    pub row: String,
    pub cap_bytes: u64,
    pub exclude_sources: Vec<String>,
    /// Per table, column name (or `prefix*`) to rule.
    pub rules: BTreeMap<String, BTreeMap<String, Rule>>,
    text: String,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        let t: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
        let strings = |key: &str| -> Result<Vec<String>, String> {
            t.get(key)
                .and_then(|v| v.as_array())
                .ok_or_else(|| format!("subset config: no {key} list"))?
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| format!("subset config: {key} holds a non-string"))
                })
                .collect()
        };
        let mut countries = strings("countries")?;
        countries.sort();
        let row = t
            .get("row")
            .and_then(|v| v.as_str())
            .ok_or("subset config: no row code")?
            .to_owned();
        let cap_mb = t
            .get("cap_mb")
            .and_then(|v| v.as_integer())
            .ok_or("subset config: no cap_mb")?;
        let mut rules = BTreeMap::new();
        for (table, cols) in t
            .get("rules")
            .and_then(|v| v.as_table())
            .ok_or("subset config: no [rules]")?
        {
            let mut m = BTreeMap::new();
            for (col, rule) in cols.as_table().ok_or("subset config: a rules entry")? {
                let r = rule.as_str().ok_or("subset config: a rule is a string")?;
                m.insert(col.clone(), Rule::parse(r)?);
            }
            rules.insert(table.clone(), m);
        }
        Ok(Config {
            countries,
            row,
            cap_bytes: u64::try_from(cap_mb).map_err(|_| "cap_mb")? * 1_000_000,
            exclude_sources: strings("exclude_sources")?,
            rules,
            text: text.to_owned(),
        })
    }

    /// The rule of `column`: the exact name, else the longest `prefix*` or `*suffix` that matches.
    fn rule(&self, table: &str, column: &str) -> Option<&Rule> {
        let m = self.rules.get(table)?;
        m.get(column).or_else(|| {
            m.iter()
                .filter(|(k, _)| match (k.strip_suffix('*'), k.strip_prefix('*')) {
                    (Some(p), _) => column.starts_with(p),
                    (_, Some(s)) => column.ends_with(s),
                    _ => false,
                })
                .max_by_key(|(k, _)| k.len())
                .map(|(_, r)| r)
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Col {
    F(Vec<f64>),
    U8(Vec<u8>),
    U32(Vec<u32>),
    U64(Vec<u64>),
    B(Vec<bool>),
    S(Vec<String>),
}

impl Col {
    fn len(&self) -> usize {
        match self {
            Col::F(v) => v.len(),
            Col::U8(v) => v.len(),
            Col::U32(v) => v.len(),
            Col::U64(v) => v.len(),
            Col::B(v) => v.len(),
            Col::S(v) => v.len(),
        }
    }

    fn take(&self, rows: &[usize]) -> Col {
        fn pick<T: Clone>(v: &[T], rows: &[usize]) -> Vec<T> {
            rows.iter().map(|&r| v[r].clone()).collect()
        }
        match self {
            Col::F(v) => Col::F(pick(v, rows)),
            Col::U8(v) => Col::U8(pick(v, rows)),
            Col::U32(v) => Col::U32(pick(v, rows)),
            Col::U64(v) => Col::U64(pick(v, rows)),
            Col::B(v) => Col::B(pick(v, rows)),
            Col::S(v) => Col::S(pick(v, rows)),
        }
    }

    /// An integer column's value as a key.
    fn key(&self, r: usize) -> Result<u64, String> {
        match self {
            Col::U8(v) => Ok(u64::from(v[r])),
            Col::U32(v) => Ok(u64::from(v[r])),
            Col::U64(v) => Ok(v[r]),
            _ => Err("a key column must be an unsigned integer".into()),
        }
    }

    fn array(&self) -> ArrayRef {
        match self {
            Col::F(v) => Arc::new(Float64Array::from(v.clone())),
            Col::U8(v) => Arc::new(UInt8Array::from(v.clone())),
            Col::U32(v) => Arc::new(UInt32Array::from(v.clone())),
            Col::U64(v) => Arc::new(UInt64Array::from(v.clone())),
            Col::B(v) => Arc::new(BooleanArray::from(v.clone())),
            Col::S(v) => Arc::new(StringArray::from(v.clone())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub names: Vec<String>,
    pub cols: Vec<Col>,
}

impl Table {
    pub fn rows(&self) -> usize {
        self.cols.first().map_or(0, Col::len)
    }

    fn index(&self, name: &str) -> Result<usize, String> {
        self.names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| format!("no column {name}"))
    }

    fn col(&self, name: &str) -> Result<&Col, String> {
        Ok(&self.cols[self.index(name)?])
    }

    fn u32s(&self, name: &str) -> Result<&Vec<u32>, String> {
        match self.col(name)? {
            Col::U32(v) => Ok(v),
            _ => Err(format!("{name} is not u32")),
        }
    }

    fn strings(&self, name: &str) -> Result<&Vec<String>, String> {
        match self.col(name)? {
            Col::S(v) => Ok(v),
            _ => Err(format!("{name} is not a string")),
        }
    }

    fn take(&self, rows: &[usize]) -> Table {
        Table {
            names: self.names.clone(),
            cols: self.cols.iter().map(|c| c.take(rows)).collect(),
        }
    }

    fn keep(&self, mask: &[bool]) -> Table {
        let rows: Vec<usize> = (0..mask.len()).filter(|&r| mask[r]).collect();
        self.take(&rows)
    }

    /// Column `name` with every value through `map`.
    fn remap(&mut self, name: &str, map: &[u32]) -> Result<(), String> {
        let i = self.index(name)?;
        let Col::U32(v) = &mut self.cols[i] else {
            return Err(format!("{name} is not u32"));
        };
        for x in v.iter_mut() {
            if *x != NONE {
                *x = map[*x as usize];
            }
        }
        Ok(())
    }
}

pub fn read(path: &Path) -> Result<Table, String> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let reader =
        FileReader::try_new(File::open(path).map_err(|e| fail(&e))?, None).map_err(|e| fail(&e))?;
    let schema = reader.schema();
    let mut t = Table {
        names: schema.fields().iter().map(|f| f.name().clone()).collect(),
        cols: Vec::new(),
    };
    for f in schema.fields() {
        t.cols.push(match f.data_type() {
            DataType::Float64 => Col::F(Vec::new()),
            DataType::UInt8 => Col::U8(Vec::new()),
            DataType::UInt32 => Col::U32(Vec::new()),
            DataType::UInt64 => Col::U64(Vec::new()),
            DataType::Boolean => Col::B(Vec::new()),
            DataType::Utf8 => Col::S(Vec::new()),
            other => return Err(fail(&format!("column {} is {other}", f.name()))),
        });
    }
    for batch in reader {
        let batch = batch.map_err(|e| fail(&e))?;
        for (col, a) in t.cols.iter_mut().zip(batch.columns()) {
            if a.null_count() > 0 {
                return Err(fail(&"null value"));
            }
            match col {
                Col::F(v) => v.extend(a.as_primitive::<Float64Type>().values()),
                Col::U8(v) => v.extend(a.as_primitive::<UInt8Type>().values()),
                Col::U32(v) => v.extend(a.as_primitive::<UInt32Type>().values()),
                Col::U64(v) => v.extend(a.as_primitive::<UInt64Type>().values()),
                Col::B(v) => v.extend(a.as_boolean().iter().flatten()),
                Col::S(v) => v.extend(a.as_string::<i32>().iter().flatten().map(str::to_owned)),
            }
        }
    }
    Ok(t)
}

fn write(dir: &Path, name: &str, t: &Table) -> Result<String, String> {
    let batch = table::batch(
        t.names
            .iter()
            .cloned()
            .zip(t.cols.iter().map(Col::array))
            .collect(),
    )?;
    let path = dir.join(name);
    let digest = table::write(&path, &batch, &BTreeMap::new())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(hex(digest))
}

/// Dropped countries as one: `map` sends every old country index to its new one, `weight` is
/// the GDP of each old country. Rows are grouped by the country keys (mapped) and the other key
/// columns, in column order, so a table's row order is kept.
fn aggregate(
    config: &Config,
    name: &str,
    t: &Table,
    country_keys: &[&str],
    map: &[u32],
    row_index: u32,
    weight: &[f64],
) -> Result<Table, String> {
    let rules: Vec<&Rule> = t
        .names
        .iter()
        .map(|c| {
            if country_keys.contains(&c.as_str()) {
                Ok(&Rule::Key)
            } else {
                config
                    .rule(name, c)
                    .ok_or_else(|| format!("{name}: no rule for column {c}"))
            }
        })
        .collect::<Result<_, _>>()?;
    let key_cols: Vec<usize> = (0..t.cols.len())
        .filter(|&i| *rules[i] == Rule::Key)
        .collect();
    let country_cols: Vec<usize> = country_keys
        .iter()
        .map(|c| t.index(c))
        .collect::<Result<_, _>>()?;
    let mut groups: BTreeMap<Vec<u64>, Vec<usize>> = BTreeMap::new();
    for r in 0..t.rows() {
        let key = key_cols
            .iter()
            .map(|&i| {
                let k = t.cols[i].key(r)?;
                Ok(if country_cols.contains(&i) {
                    u64::from(map[k as usize])
                } else {
                    k
                })
            })
            .collect::<Result<Vec<u64>, String>>()?;
        groups.entry(key).or_default().push(r);
    }
    let mut out = Table {
        names: t.names.clone(),
        cols: t.cols.iter().map(|c| c.take(&[])).collect(),
    };
    for (key, members) in &groups {
        let aggregated = country_cols.iter().any(|&i| {
            key[key_cols.iter().position(|&k| k == i).unwrap_or(0)] == u64::from(row_index)
        });
        let w: Vec<f64> = members
            .iter()
            .map(|&r| {
                country_cols
                    .iter()
                    .map(|&i| match &t.cols[i] {
                        Col::U32(v) => weight[v[r] as usize],
                        _ => 1.0,
                    })
                    .product()
            })
            .collect();
        for (i, col) in t.cols.iter().enumerate() {
            let first = members[0];
            let rule = rules[i];
            if *rule == Rule::Key {
                let k = key[key_cols.iter().position(|&k| k == i).expect("key column")];
                push_key(&mut out.cols[i], k);
                continue;
            }
            let single = !aggregated;
            match (&mut out.cols[i], col) {
                (Col::F(o), Col::F(v)) => o.push(if single {
                    v[first]
                } else {
                    match rule {
                        Rule::Sum => sum(members.iter().map(|&r| v[r])),
                        Rule::Mean => mean(members.iter().map(|&r| v[r]), &w),
                        Rule::First => v[first],
                        Rule::Const(c) => c
                            .parse()
                            .map_err(|_| format!("{name}.{}: const", t.names[i]))?,
                        _ => return Err(format!("{name}.{}: rule does not fit f64", t.names[i])),
                    }
                }),
                (Col::B(o), Col::B(v)) => o.push(if single {
                    v[first]
                } else {
                    match rule {
                        Rule::Any => members.iter().any(|&r| v[r]),
                        Rule::First => v[first],
                        Rule::Const(c) => c
                            .parse()
                            .map_err(|_| format!("{name}.{}: const", t.names[i]))?,
                        _ => return Err(format!("{name}.{}: rule does not fit bool", t.names[i])),
                    }
                }),
                (Col::S(o), Col::S(v)) => o.push(if single {
                    v[first].clone()
                } else {
                    match rule {
                        Rule::First => v[first].clone(),
                        Rule::Const(c) => c.clone(),
                        _ => return Err(format!("{name}.{}: rule does not fit text", t.names[i])),
                    }
                }),
                (Col::U8(o), Col::U8(v)) => {
                    o.push(first_or_const(rule, single, v[first], |c| c.parse().ok())?)
                }
                (Col::U32(o), Col::U32(v)) => {
                    o.push(first_or_const(rule, single, v[first], |c| c.parse().ok())?)
                }
                (Col::U64(o), Col::U64(v)) => {
                    o.push(first_or_const(rule, single, v[first], |c| c.parse().ok())?)
                }
                _ => return Err(format!("{name}: column type changed")),
            }
        }
    }
    Ok(out)
}

fn first_or_const<T>(
    rule: &Rule,
    single: bool,
    first: T,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    match (single, rule) {
        (true, _) | (_, Rule::First) => Ok(first),
        (_, Rule::Const(c)) => parse(c).ok_or_else(|| format!("const {c}")),
        _ => Err("an integer column takes first or const".into()),
    }
}

fn push_key(col: &mut Col, k: u64) {
    match col {
        Col::U8(v) => v.push(k as u8),
        Col::U32(v) => v.push(k as u32),
        Col::U64(v) => v.push(k),
        _ => unreachable!("key columns are unsigned integers"),
    }
}

fn sum(values: impl Iterator<Item = f64>) -> f64 {
    let v: Vec<f64> = values.filter(|x| !x.is_nan()).collect();
    if v.is_empty() {
        f64::NAN
    } else {
        v.iter().sum()
    }
}

fn mean(values: impl Iterator<Item = f64>, weights: &[f64]) -> f64 {
    let pairs: Vec<(f64, f64)> = values
        .zip(weights.iter().copied())
        .filter(|(x, _)| !x.is_nan())
        .collect();
    let total: f64 = pairs.iter().map(|(_, w)| w).sum();
    if pairs.is_empty() {
        f64::NAN
    } else if total > 0.0 {
        pairs.iter().map(|(x, w)| x * w).sum::<f64>() / total
    } else {
        pairs.iter().map(|(x, _)| x).sum::<f64>() / pairs.len() as f64
    }
}

/// `owner_{k}` columns are indexed by country: kept owners keep a column at their new index,
/// the dropped owners' columns are summed into the rest-of-world owner.
fn merge_owner_columns(t: &mut Table, map: &[u32], new_n: usize) -> Result<(), String> {
    let mut summed: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
    let mut keep = Vec::new();
    for (i, name) in t.names.iter().enumerate() {
        let Some(k) = name.strip_prefix("owner_") else {
            keep.push(i);
            continue;
        };
        let old: usize = k.parse().map_err(|_| format!("column {name}"))?;
        let Col::F(v) = &t.cols[i] else {
            return Err(format!("{name} is not f64"));
        };
        let acc = summed
            .entry(
                *map.get(old)
                    .ok_or_else(|| format!("{name}: no such country"))?,
            )
            .or_insert_with(|| vec![0.0; v.len()]);
        for (a, x) in acc.iter_mut().zip(v) {
            *a += x;
        }
    }
    if summed.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = keep.iter().map(|&i| t.names[i].clone()).collect();
    let cols: Vec<Col> = keep.iter().map(|&i| t.cols[i].clone()).collect();
    *t = Table { names, cols };
    for j in 0..new_n as u32 {
        t.names.push(format!("owner_{j}"));
        t.cols.push(Col::F(
            summed.remove(&j).unwrap_or_else(|| vec![0.0; t.rows()]),
        ));
    }
    Ok(())
}

/// What `subset` wrote: file name to SHA-256, total bytes.
#[derive(Debug)]
pub struct Report {
    pub tables: BTreeMap<String, String>,
    pub bytes: u64,
    pub provinces: usize,
    pub countries: usize,
}

fn table_names() -> impl Iterator<Item = &'static str> {
    TABLES.into_iter().chain(OTHER_TABLES)
}

/// Every table and column of a pack with its type, one `table.column type` line each, and the
/// columns of the aggregated tables that `config` has no rule for.
pub fn schema(pack: &Path, config: &Config) -> Result<(Vec<String>, Vec<String>), String> {
    let (mut lines, mut missing) = (Vec::new(), Vec::new());
    for name in table_names() {
        let t = read(&pack.join(name))?;
        for (c, col) in t.names.iter().zip(&t.cols) {
            let ty = match col {
                Col::F(_) => "f64",
                Col::U8(_) => "u8",
                Col::U32(_) => "u32",
                Col::U64(_) => "u64",
                Col::B(_) => "bool",
                Col::S(_) => "utf8",
            };
            lines.push(format!("{name}.{c} {ty} rows={}", t.rows()));
            if config.rules.contains_key(name.trim_end_matches(".arrow"))
                && config.rule(name.trim_end_matches(".arrow"), c).is_none()
            {
                missing.push(format!("{name}.{c}"));
            }
        }
    }
    Ok((lines, missing))
}

pub fn subset(pack: &Path, config: &Config, out: &Path) -> Result<Report, String> {
    for entry in std::fs::read_dir(pack).map_err(|e| format!("{}: {e}", pack.display()))? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".arrow") && !table_names().any(|t| t == name) {
            return Err(format!(
                "{name}: not a table of an uncalibrated pack; subset takes the data state"
            ));
        }
    }
    let t = |name: &str| read(&pack.join(name));
    let mut econ_countries = t("econ_countries.arrow")?;
    let codes = econ_countries.strings("code")?.clone();
    let kept: BTreeSet<&str> = config.countries.iter().map(String::as_str).collect();
    if let Some(c) = kept.iter().find(|c| !codes.iter().any(|x| x == **c)) {
        return Err(format!("country {c} is not in econ_countries"));
    }
    if codes.contains(&config.row) {
        return Err(format!("row code {} is a pack country", config.row));
    }
    // kept countries in pack order, then the rest of world
    let mut next = 0u32;
    let map: Vec<u32> = codes
        .iter()
        .map(|c| {
            if kept.contains(c.as_str()) {
                next += 1;
                next - 1
            } else {
                u32::MAX
            }
        })
        .collect();
    let row_index = next;
    let map: Vec<u32> = map
        .iter()
        .map(|&m| if m == u32::MAX { row_index } else { m })
        .collect();
    let new_codes: Vec<&str> = codes
        .iter()
        .zip(&map)
        .filter(|(_, m)| **m != row_index)
        .map(|(c, _)| c.as_str())
        .collect();
    let gdp = match econ_countries.col("gdp")? {
        Col::F(v) => v
            .iter()
            .map(|x| if x.is_nan() { 0.0 } else { *x })
            .collect::<Vec<_>>(),
        _ => return Err("gdp is not f64".into()),
    };

    let provinces = t("provinces.arrow")?;
    let province_kept: Vec<bool> = provinces
        .strings("country")?
        .iter()
        .map(|c| kept.contains(c.as_str()))
        .collect();
    let mut next = 0u32;
    let pmap: Vec<u32> = province_kept
        .iter()
        .map(|&k| {
            if k {
                next += 1;
                next - 1
            } else {
                NONE
            }
        })
        .collect();
    let n_provinces = next as usize;
    let filter_by_province = |mut x: Table| -> Result<Table, String> {
        let mask: Vec<bool> = x
            .u32s("province")?
            .iter()
            .map(|&p| p != NONE && pmap[p as usize] != NONE)
            .collect();
        x = x.keep(&mask);
        x.remap("province", &pmap)?;
        Ok(x)
    };

    let mut tables: Vec<(&str, Table)> = vec![("provinces.arrow", provinces.keep(&province_kept))];
    let countries = t("countries.arrow")?;
    let mask: Vec<bool> = countries
        .strings("code")?
        .iter()
        .map(|c| kept.contains(c.as_str()))
        .collect();
    tables.push(("countries.arrow", countries.keep(&mask)));
    tables.push(("cells.arrow", filter_by_province(t("cells.arrow")?)?));
    tables.push(("counties.arrow", filter_by_province(t("counties.arrow")?)?));

    let nodes = t("nodes.arrow")?;
    let node_kept: Vec<bool> = nodes
        .u32s("province")?
        .iter()
        .map(|&p| p == NONE || pmap[p as usize] != NONE)
        .collect();
    let mut next = 0u32;
    let nmap: Vec<u32> = node_kept
        .iter()
        .map(|&k| {
            if k {
                next += 1;
                next - 1
            } else {
                NONE
            }
        })
        .collect();
    let mut new_nodes = nodes.keep(&node_kept);
    new_nodes.remap("province", &pmap)?;
    tables.push(("nodes.arrow", new_nodes));
    let edges = t("edges.arrow")?;
    let mask: Vec<bool> = edges
        .u32s("a")?
        .iter()
        .zip(edges.u32s("b")?)
        .map(|(&a, &b)| nmap[a as usize] != NONE && nmap[b as usize] != NONE)
        .collect();
    let mut new_edges = edges.keep(&mask);
    new_edges.remap("a", &nmap)?;
    new_edges.remap("b", &nmap)?;
    tables.push(("edges.arrow", new_edges));

    for name in COPIED {
        tables.push((name, t(name)?));
    }
    let provinces_econ = t("econ_provinces.arrow")?;
    if provinces_econ.rows() != province_kept.len() {
        return Err("econ_provinces has a row per province".into());
    }
    tables.push(("econ_provinces.arrow", provinces_econ.keep(&province_kept)));
    tables.push((
        "econ_pops.arrow",
        filter_by_province(t("econ_pops.arrow")?)?,
    ));

    let rule_name = |n: &str| n.trim_end_matches(".arrow").to_owned();
    let agg = |n: &str, x: &Table, keys: &[&str]| {
        aggregate(config, &rule_name(n), x, keys, &map, row_index, &gdp)
    };
    merge_owner_columns(&mut econ_countries, &map, row_index as usize + 1)?;
    econ_countries.names.push("__country".into());
    econ_countries
        .cols
        .push(Col::U32((0..codes.len() as u32).collect()));
    let mut c = agg("econ_countries.arrow", &econ_countries, &["__country"])?;
    let last = c.names.len() - 1;
    c.names.truncate(last);
    c.cols.truncate(last);
    let ci = c.index("code")?;
    if let Col::S(v) = &mut c.cols[ci] {
        if v.len() != new_codes.len() + 1 {
            return Err("the rest-of-world row is missing: every country is kept".into());
        }
        v[new_codes.len()] = config.row.clone();
    }
    tables.push(("econ_countries.arrow", c));
    tables.push((
        "econ_io.arrow",
        agg("econ_io.arrow", &t("econ_io.arrow")?, &["country"])?,
    ));
    tables.push((
        "econ_tariffs.arrow",
        agg(
            "econ_tariffs.arrow",
            &t("econ_tariffs.arrow")?,
            &["importer", "exporter"],
        )?,
    ));
    tables.push((
        "econ_home_shares.arrow",
        agg(
            "econ_home_shares.arrow",
            &t("econ_home_shares.arrow")?,
            &["country"],
        )?,
    ));

    // gateways of kept countries only; the rest of world has no ports
    let mut g = t("gateways.arrow")?;
    let mask: Vec<bool> = g
        .u32s("country")?
        .iter()
        .zip(g.u32s("node")?)
        .map(|(&c, &n)| map[c as usize] != row_index && nmap[n as usize] != NONE)
        .collect();
    g = g.keep(&mask);
    g.remap("country", &map)?;
    g.remap("node", &nmap)?;
    tables.push(("gateways.arrow", g));

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut digests = BTreeMap::new();
    let mut bytes = 0;
    tables.sort_by_key(|(n, _)| *n);
    for (name, x) in &tables {
        digests.insert((*name).to_owned(), write(out, name, x)?);
        bytes += std::fs::metadata(out.join(name))
            .map_err(|e| e.to_string())?
            .len();
    }
    if bytes > config.cap_bytes {
        return Err(format!(
            "the subset is {bytes} bytes, over the {} byte cap (ADR 0370)",
            config.cap_bytes
        ));
    }
    write_manifest(pack, config, out, &digests)?;
    Ok(Report {
        tables: digests,
        bytes,
        provinces: n_provinces,
        countries: new_codes.len() + 1,
    })
}

/// `manifest.json`: the sources of the input pack less the excluded ones (ADR 0374), the
/// subset's own configuration and the table digests.
fn write_manifest(
    pack: &Path,
    config: &Config,
    out: &Path,
    digests: &BTreeMap<String, String>,
) -> Result<(), String> {
    let path = pack.join("manifest.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let m: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let excluded = |id: &str| {
        config
            .exclude_sources
            .iter()
            .any(|p| p.strip_suffix('*').map_or(id == p, |p| id.starts_with(p)))
    };
    let sources = m["sources"].as_array().ok_or("manifest: no sources")?;
    let (drop, keep): (Vec<_>, Vec<_>) = sources
        .iter()
        .cloned()
        .partition(|s| s["source"].as_str().is_some_and(excluded));
    let out_manifest = serde_json::json!({
        "pack_format": m["pack_format"],
        "year": m["year"],
        "subset": {
            "countries": config.countries,
            "row": config.row,
            "config_sha256": hex(Sha256::digest(config.text.as_bytes())),
            "source_manifest_sha256": hex(Sha256::digest(text.as_bytes())),
        },
        "sources": keep,
        "excluded_sources": drop.iter().map(|s| s["source"].clone()).collect::<Vec<_>>(),
        "tables": digests,
    });
    let body = serde_json::to_string_pretty(&out_manifest).map_err(|e| e.to_string())? + "\n";
    std::fs::write(out.join("manifest.json"), body).map_err(|e| e.to_string())
}
