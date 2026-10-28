//! Canonical JSON of reader output: sorted keys, no spaces, floats as Python's `repr` writes
//! them, the text `json.dumps(x, sort_keys=True, ensure_ascii=False, separators=(",", ":"))`
//! gives in Python.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::numeric::repr_f64;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    pub fn object<K: ToString, V: Into<Json>>(entries: impl IntoIterator<Item = (K, V)>) -> Json {
        Json::Object(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.into()))
                .collect(),
        )
    }

    pub fn list<V: Into<Json>>(items: impl IntoIterator<Item = V>) -> Json {
        Json::List(items.into_iter().map(Into::into).collect())
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => write!(out, "{i}").expect("write to String"),
            Json::Float(x) if x.is_nan() => out.push_str("NaN"),
            Json::Float(x) if x.is_infinite() => {
                out.push_str(if *x > 0.0 { "Infinity" } else { "-Infinity" });
            }
            Json::Float(x) => out.push_str(&repr_f64(*x)),
            Json::Str(s) => write_str(s, out),
            Json::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_str(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 => write!(out, "\\u{:04x}", u32::from(c)).expect("write"),
            c => out.push(c),
        }
    }
    out.push('"');
}

impl From<bool> for Json {
    fn from(b: bool) -> Self {
        Json::Bool(b)
    }
}

impl From<i64> for Json {
    fn from(i: i64) -> Self {
        Json::Int(i)
    }
}

impl From<f64> for Json {
    fn from(x: f64) -> Self {
        Json::Float(x)
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Json::Str(s.to_owned())
    }
}

impl From<String> for Json {
    fn from(s: String) -> Self {
        Json::Str(s)
    }
}

impl From<&String> for Json {
    fn from(s: &String) -> Self {
        Json::Str(s.clone())
    }
}

impl From<&f64> for Json {
    fn from(x: &f64) -> Self {
        Json::Float(*x)
    }
}

impl From<&i64> for Json {
    fn from(i: &i64) -> Self {
        Json::Int(*i)
    }
}

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(v: Vec<T>) -> Self {
        Json::list(v)
    }
}

impl<T: Into<Json> + Copy, const N: usize> From<[T; N]> for Json {
    fn from(v: [T; N]) -> Self {
        Json::list(v)
    }
}

impl<K: ToString, V: Into<Json>> From<BTreeMap<K, V>> for Json {
    fn from(m: BTreeMap<K, V>) -> Self {
        Json::object(m)
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(v: Option<T>) -> Self {
        v.map_or(Json::Null, Into::into)
    }
}

impl<A: Into<Json>, B: Into<Json>> From<(A, B)> for Json {
    fn from((a, b): (A, B)) -> Self {
        Json::List(vec![a.into(), b.into()])
    }
}

/// A set as Python's `sorted` list of it.
impl<T: Into<Json>> From<BTreeSet<T>> for Json {
    fn from(s: BTreeSet<T>) -> Self {
        Json::list(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_python_canonical_json() {
        let j = Json::object([
            ("b", Json::list([1.0, 1e-5, 2000.0])),
            ("a", Json::Str("é\"\t\u{1}".into())),
            ("c", Json::Null),
            ("d", Json::Int(-4)),
        ]);
        assert_eq!(
            j.text(),
            r#"{"a":"é\"\t\u0001","b":[1.0,1e-05,2000.0],"c":null,"d":-4}"#
        );
    }
}
