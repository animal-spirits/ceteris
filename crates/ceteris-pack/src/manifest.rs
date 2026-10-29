//! `manifest.json` as build.py writes it, and `pack.toml` as `json.dumps` writes what `tomllib`
//! parses.

use std::collections::BTreeMap;

use toml::Spanned;
use toml::de::{DeString, DeTable, DeValue};

use crate::fetch::Entry;
use crate::json::Json;
use crate::table::PACK_FORMAT;

/// A TOML document as `json.dumps` writes what `tomllib` parses: tables as objects with their
/// keys in the order they appear, TOML types as their JSON counterparts. Datetimes, which
/// `json.dumps` cannot write, are an error.
pub fn toml_json(text: &str) -> Result<Json, String> {
    let values: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let order = DeTable::parse(text).map_err(|e| e.to_string())?;
    table_json(&values, order.get_ref())
}

/// Where a key first appears: its own span, or that of a key inside it, as a dotted header
/// (`[a.b]`) creates `a` before any `[a]`.
fn first(key: &Spanned<DeString>, value: &DeValue) -> usize {
    let inner = match value {
        DeValue::Table(t) => t.iter().map(|(k, v)| first(k, v.get_ref())).min(),
        DeValue::Array(a) => a
            .iter()
            .filter_map(|v| match v.get_ref() {
                DeValue::Table(t) => t.iter().map(|(k, v)| first(k, v.get_ref())).min(),
                _ => None,
            })
            .min(),
        _ => None,
    };
    inner.map_or(key.span().start, |i| i.min(key.span().start))
}

fn table_json(values: &toml::Table, order: &DeTable) -> Result<Json, String> {
    let mut keys: Vec<_> = order.iter().collect();
    keys.sort_by_key(|(k, v)| first(k, v.get_ref()));
    keys.into_iter()
        .map(|(k, v)| {
            let key: &str = k.get_ref();
            let value = values
                .get(key)
                .ok_or_else(|| format!("no value for {key}"))?;
            Ok((key.to_owned(), value_json(value, v.get_ref())?))
        })
        .collect::<Result<_, String>>()
        .map(Json::Fields)
}

fn value_json(value: &toml::Value, order: &DeValue) -> Result<Json, String> {
    use toml::Value;
    Ok(match (value, order) {
        (Value::String(s), _) => Json::Str(s.clone()),
        (Value::Integer(i), _) => Json::Int(*i),
        (Value::Float(x), _) => Json::Float(*x),
        (Value::Boolean(b), _) => Json::Bool(*b),
        (Value::Datetime(d), _) => return Err(format!("{d}: json.dumps writes no datetime")),
        (Value::Array(a), DeValue::Array(o)) => Json::List(
            a.iter()
                .zip(o.iter())
                .map(|(v, o)| value_json(v, o.get_ref()))
                .collect::<Result<_, _>>()?,
        ),
        (Value::Table(t), DeValue::Table(o)) => table_json(t, o)?,
        (v, _) => return Err(format!("{v}: parsed two ways")),
    })
}

/// `manifest.json` as build.py writes it (before `json.dumps(indent=2)`): the pack format,
/// year and configuration of `pack.toml` (its text), each fetched file's source, name and
/// SHA-256, each written file's SHA-256 by path, and the build stats.
pub fn manifest(
    config: &str,
    sources: &[Entry],
    tables: &BTreeMap<String, String>,
    stats: Json,
) -> Result<Json, String> {
    let config = toml_json(config)?;
    let year = match &config {
        Json::Fields(f) => f.iter().find(|(k, _)| k == "year").map(|(_, v)| v.clone()),
        _ => None,
    }
    .ok_or("pack.toml: no year")?;
    let format: i64 = PACK_FORMAT.parse().map_err(|_| "pack format")?;
    let sources = sources
        .iter()
        .map(|e| {
            let sha256 = e
                .sha256
                .as_deref()
                .ok_or_else(|| format!("{}: no sha256", e.name))?;
            Ok(Json::Fields(vec![
                ("source".to_owned(), Json::from(&e.source)),
                ("name".to_owned(), Json::from(&e.name)),
                ("sha256".to_owned(), Json::from(sha256)),
            ]))
        })
        .collect::<Result<_, String>>()?;
    Ok(Json::Fields(vec![
        ("pack_format".to_owned(), Json::Int(format)),
        ("year".to_owned(), year),
        ("config".to_owned(), config),
        ("sources".to_owned(), Json::List(sources)),
        ("tables".to_owned(), Json::object(tables.iter())),
        ("stats".to_owned(), stats),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_is_written_as_json_dumps_writes_tomllib_s_dict() {
        let text = "year = 2000\nzeta = 1.5\nalpha = \"é\"\nbig = 250_000\n\
                    list = [1, 2.0, -0.0, 1e300, inf]\n[sub]\nb = true\na = [ \"x\" ]\n\
                    [[rows]]\nk = 1\n[[rows]]\nk = 2\n";
        // json.dumps(tomllib.loads(text), indent=2)
        let want = "{\n  \"year\": 2000,\n  \"zeta\": 1.5,\n  \"alpha\": \"\\u00e9\",\n  \
                    \"big\": 250000,\n  \"list\": [\n    1,\n    2.0,\n    -0.0,\n    1e+300,\n    \
                    Infinity\n  ],\n  \"sub\": {\n    \"b\": true,\n    \"a\": [\n      \"x\"\n    \
                    ]\n  },\n  \"rows\": [\n    {\n      \"k\": 1\n    },\n    {\n      \"k\": 2\n    \
                    }\n  ]\n}";
        assert_eq!(toml_json(text).unwrap().pretty(), want);
        assert!(toml_json("when = 2000-01-01").is_err());
    }

    #[test]
    fn a_table_a_dotted_header_creates_comes_where_that_header_is() {
        let text = "z = 1\n[a.b]\nx = 1\n[c]\nw = 1\n[a]\ny = 2\n";
        // json.dumps(tomllib.loads(text))
        let want = r#"{"z":1,"a":{"b":{"x":1},"y":2},"c":{"w":1}}"#;
        assert_eq!(toml_json(text).unwrap().text(), want);
    }

    #[test]
    fn the_manifest_is_build_py_s() {
        let entry = |source: &str, name: &str, sha256: Option<&str>| Entry {
            source: source.to_owned(),
            name: name.to_owned(),
            sha256: sha256.map(str::to_owned),
            bytes: None,
            url: "u".to_owned(),
            optional: false,
            manual: false,
        };
        let sources = [
            entry("natural-earth", "b.zip", Some("ab")),
            entry("x", "geoboundaries/A-ADM2.geojson", Some("cd")),
        ];
        let tables: BTreeMap<String, String> = [
            ("web/provinces.json", "03"),
            ("cells.arrow", "01"),
            ("web/map.pmtiles:countries", "02"),
        ]
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .into();
        let stats = Json::Fields(vec![
            ("ports".to_owned(), Json::Int(3)),
            (
                "fills".to_owned(),
                Json::Fields(vec![(
                    "provinces".to_owned(),
                    Json::Fields(vec![
                        ("vertices".to_owned(), Json::Int(5)),
                        ("triangles".to_owned(), Json::Int(4)),
                    ]),
                )]),
            ),
            ("share".to_owned(), Json::Float(0.0302)),
        ]);
        let config = "year = 2000\nland_res = 4\n";
        // json.dumps(manifest, indent=2) + "\n" as build.py writes it.
        let want = "{\n  \"pack_format\": 3,\n  \"year\": 2000,\n  \"config\": {\n    \"year\": 2000,\n    \
                    \"land_res\": 4\n  },\n  \"sources\": [\n    {\n      \"source\": \"natural-earth\",\n      \
                    \"name\": \"b.zip\",\n      \"sha256\": \"ab\"\n    },\n    {\n      \"source\": \"x\",\n      \
                    \"name\": \"geoboundaries/A-ADM2.geojson\",\n      \"sha256\": \"cd\"\n    }\n  ],\n  \
                    \"tables\": {\n    \"cells.arrow\": \"01\",\n    \"web/map.pmtiles:countries\": \"02\",\n    \
                    \"web/provinces.json\": \"03\"\n  },\n  \"stats\": {\n    \"ports\": 3,\n    \"fills\": {\n      \
                    \"provinces\": {\n        \"vertices\": 5,\n        \"triangles\": 4\n      }\n    },\n    \
                    \"share\": 0.0302\n  }\n}\n";
        let got = manifest(config, &sources, &tables, stats.clone())
            .unwrap()
            .pretty()
            + "\n";
        assert_eq!(got, want);
        let unpinned = [entry("x", "y", None)];
        assert!(manifest(config, &unpinned, &tables, stats).is_err());
    }
}
