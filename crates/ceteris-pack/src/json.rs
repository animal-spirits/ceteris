//! JSON as Python's `json.dumps` writes it. Canonical text of reader output: sorted keys, no
//! spaces, floats as Python's `repr` writes them, the text `json.dumps(x, sort_keys=True,
//! ensure_ascii=False, separators=(",", ":"))` gives; `Fields` keeps a dict's insertion order, and
//! `pretty` is `json.dumps(x, indent=2)`.

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
    /// An object with its keys in the order given, as Python writes a dict.
    Fields(Vec<(String, Json)>),
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

    /// `ensure_ascii=False, separators=(",", ":")`.
    pub fn text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, false, None);
        out
    }

    /// `indent=2`: one item per line, `": "` after keys, non-ASCII escaped.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, true, Some(0));
        out
    }

    fn write(&self, out: &mut String, ascii: bool, depth: Option<usize>) {
        let items: Vec<(Option<&str>, &Json)> = match self {
            Json::Null => return out.push_str("null"),
            Json::Bool(b) => return out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => return write!(out, "{i}").expect("write to String"),
            Json::Float(x) if x.is_nan() => return out.push_str("NaN"),
            Json::Float(x) if x.is_infinite() => {
                return out.push_str(if *x > 0.0 { "Infinity" } else { "-Infinity" });
            }
            Json::Float(x) => return out.push_str(&repr_f64(*x)),
            Json::Str(s) => return write_str(s, ascii, out),
            Json::List(items) => items.iter().map(|v| (None, v)).collect(),
            Json::Object(entries) => entries.iter().map(|(k, v)| (Some(k.as_str()), v)).collect(),
            Json::Fields(entries) => entries.iter().map(|(k, v)| (Some(k.as_str()), v)).collect(),
        };
        let (open, close) = if matches!(self, Json::List(_)) {
            ('[', ']')
        } else {
            ('{', '}')
        };
        out.push(open);
        let newline = |out: &mut String, d: usize| {
            out.push('\n');
            out.push_str(&"  ".repeat(d));
        };
        for (i, (key, value)) in items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            if let Some(d) = depth {
                newline(out, d + 1);
            }
            if let Some(k) = key {
                write_str(k, ascii, out);
                out.push_str(if depth.is_some() { ": " } else { ":" });
            }
            value.write(out, ascii, depth.map(|d| d + 1));
        }
        if let (Some(d), false) = (depth, items.is_empty()) {
            newline(out, d);
        }
        out.push(close);
    }
}

fn write_str(s: &str, ascii: bool, out: &mut String) {
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
            c if u32::from(c) < 0x20 || (ascii && u32::from(c) > 0x7e) => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    write!(out, "\\u{:04x}", u).expect("write to String");
                }
            }
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

    fn fields(entries: Vec<(&str, Json)>) -> Json {
        Json::Fields(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
        )
    }

    #[test]
    fn fields_keep_their_order() {
        let j = fields(vec![
            ("b", Json::Int(1)),
            ("a", Json::list([Json::Bool(true), Json::Null])),
        ]);
        assert_eq!(j.text(), r#"{"b":1,"a":[true,null]}"#);
    }

    #[test]
    fn pretty_writes_as_python_indents() {
        let j = fields(vec![
            (
                "b",
                Json::list([Json::Int(1), Json::Float(2.5), fields(vec![])]),
            ),
            (
                "a",
                fields(vec![
                    ("x", Json::List(vec![])),
                    ("y", Json::Str("é\u{7f}\u{1F600}".into())),
                ]),
            ),
            ("c", Json::Bool(true)),
        ]);
        // json.dumps(..., indent=2)
        let want = "{\n  \"b\": [\n    1,\n    2.5,\n    {}\n  ],\n  \"a\": {\n    \"x\": [],\n    \
                    \"y\": \"\\u00e9\\u007f\\ud83d\\ude00\"\n  },\n  \"c\": true\n}";
        assert_eq!(j.pretty(), want);
    }
}
