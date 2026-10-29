//! JSON values printed as Python's `json.dumps` prints them.

use ceteris_reference::output::{json_float, json_string};

/// A JSON value, printed as Python's `json.dumps` prints it.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn object<'k>(entries: impl IntoIterator<Item = (&'k str, Json)>) -> Json {
        Json::Object(entries.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    fn scalar(&self) -> Option<String> {
        match self {
            Json::Null => Some("null".into()),
            Json::Int(i) => Some(i.to_string()),
            Json::Float(x) => Some(json_float(*x)),
            Json::Str(s) => Some(json_string(s)),
            Json::List(_) | Json::Object(_) => None,
        }
    }

    /// `json.dumps(value)`.
    pub fn compact(&self) -> String {
        if let Some(s) = self.scalar() {
            return s;
        }
        match self {
            Json::List(items) => {
                let items: Vec<String> = items.iter().map(Json::compact).collect();
                format!("[{}]", items.join(", "))
            }
            Json::Object(entries) => {
                let entries: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}: {}", json_string(k), v.compact()))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
            _ => unreachable!("scalars print above"),
        }
    }

    /// `json.dumps(value, indent=2)`.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        self.indent(0, &mut out);
        out
    }

    fn indent(&self, level: usize, out: &mut String) {
        if let Some(s) = self.scalar() {
            out.push_str(&s);
            return;
        }
        let pad = |n: usize| "  ".repeat(n);
        let (open, close, empty) = match self {
            Json::List(v) => ('[', ']', v.is_empty()),
            Json::Object(v) => ('{', '}', v.is_empty()),
            _ => unreachable!("scalars print above"),
        };
        out.push(open);
        if !empty {
            let mut first = true;
            let mut item = |key: Option<&str>, value: &Json, out: &mut String| {
                out.push_str(if first { "\n" } else { ",\n" });
                first = false;
                out.push_str(&pad(level + 1));
                if let Some(k) = key {
                    out.push_str(&json_string(k));
                    out.push_str(": ");
                }
                value.indent(level + 1, out);
            };
            match self {
                Json::List(v) => v.iter().for_each(|x| item(None, x, out)),
                Json::Object(v) => v.iter().for_each(|(k, x)| item(Some(k), x, out)),
                _ => unreachable!("scalars print above"),
            }
            out.push('\n');
            out.push_str(&pad(level));
        }
        out.push(close);
    }
}

#[cfg(test)]
mod tests {
    use super::Json;

    /// `json.dumps` and `json.dumps(..., indent=2)` of
    /// `{"a": [1, 2.5, None], "b": {}, "c": [], "d": {"e": "x", "f": float("nan")}}`.
    #[test]
    fn json_prints_as_python() {
        let v = Json::object([
            (
                "a",
                Json::List(vec![Json::Int(1), Json::Float(2.5), Json::Null]),
            ),
            ("b", Json::Object(vec![])),
            ("c", Json::List(vec![])),
            (
                "d",
                Json::object([("e", Json::Str("x".into())), ("f", Json::Float(f64::NAN))]),
            ),
        ]);
        assert_eq!(
            v.compact(),
            r#"{"a": [1, 2.5, null], "b": {}, "c": [], "d": {"e": "x", "f": NaN}}"#
        );
        assert_eq!(
            v.pretty(),
            "{\n  \"a\": [\n    1,\n    2.5,\n    null\n  ],\n  \"b\": {},\n  \"c\": [],\n  \
             \"d\": {\n    \"e\": \"x\",\n    \"f\": NaN\n  }\n}"
        );
    }
}
