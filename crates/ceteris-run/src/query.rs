//! The smallest query language of ADR 0188 (edits by query, ADR 0155): comparisons, `and`,
//! `or`, `not` over the named columns of a run's tables. A bare column is true when nonzero,
//! so flags are 0/1 columns. The text form is what an edit stores; it is deterministic.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    Parse(String),
    UnknownColumn(String),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "query: {e}"),
            Self::UnknownColumn(c) => write!(f, "query: unknown column `{c}`"),
        }
    }
}

impl std::error::Error for QueryError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Num(f64),
    Col(usize),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Cmp(Cmp, Box<Expr>, Box<Expr>),
}

/// A parsed predicate over `columns`, which it indexes by position.
#[derive(Debug, Clone, PartialEq)]
pub struct Query(Expr);

impl Query {
    /// Parses `text`; every identifier must be one of `columns`.
    pub fn parse(text: &str, columns: &[&str]) -> Result<Self, QueryError> {
        let mut p = Parser {
            toks: lex(text)?,
            at: 0,
            columns,
        };
        let expr = p.or()?;
        match p.toks.get(p.at) {
            None => Ok(Self(expr)),
            Some(t) => Err(QueryError::Parse(format!("unexpected `{t}`"))),
        }
    }

    /// Whether the row whose column `i` is `value(i)` matches.
    pub fn matches(&self, value: &impl Fn(usize) -> f64) -> bool {
        eval(&self.0, value) != 0.0
    }
}

fn eval(e: &Expr, v: &impl Fn(usize) -> f64) -> f64 {
    let truth = |x: f64| if x != 0.0 { 1.0 } else { 0.0 };
    match e {
        Expr::Num(n) => *n,
        Expr::Col(i) => v(*i),
        Expr::Not(a) => 1.0 - truth(eval(a, v)),
        Expr::And(a, b) => truth(eval(a, v)) * truth(eval(b, v)),
        Expr::Or(a, b) => truth(eval(a, v) + eval(b, v)),
        Expr::Cmp(op, a, b) => {
            let (x, y) = (eval(a, v), eval(b, v));
            let hit = match op {
                Cmp::Lt => x < y,
                Cmp::Le => x <= y,
                Cmp::Gt => x > y,
                Cmp::Ge => x >= y,
                Cmp::Eq => x == y,
                Cmp::Ne => x != y,
            };
            f64::from(u8::from(hit))
        }
    }
}

fn lex(text: &str) -> Result<Vec<String>, QueryError> {
    let chars: Vec<char> = text.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let two: String = chars[i..chars.len().min(i + 2)].iter().collect();
        if c.is_whitespace() {
            i += 1;
        } else if ["&&", "||", "<=", ">=", "==", "!="].contains(&two.as_str()) {
            toks.push(two);
            i += 2;
        } else if "()<>!".contains(c) {
            toks.push(c.to_string());
            i += 1;
        } else if c.is_ascii_digit() || c == '.' || c == '-' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || ".eE".contains(chars[i])) {
                i += 1;
            }
            toks.push(chars[start..i].iter().collect());
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            toks.push(chars[start..i].iter().collect());
        } else {
            return Err(QueryError::Parse(format!("unexpected character `{c}`")));
        }
    }
    Ok(toks)
}

struct Parser<'a> {
    toks: Vec<String>,
    at: usize,
    columns: &'a [&'a str],
}

impl Parser<'_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.at).map(String::as_str)
    }

    fn eat(&mut self, words: &[&str]) -> bool {
        if self.peek().is_some_and(|t| words.contains(&t)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn or(&mut self) -> Result<Expr, QueryError> {
        let mut e = self.and()?;
        while self.eat(&["||", "or"]) {
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }

    fn and(&mut self) -> Result<Expr, QueryError> {
        let mut e = self.not()?;
        while self.eat(&["&&", "and"]) {
            e = Expr::And(Box::new(e), Box::new(self.not()?));
        }
        Ok(e)
    }

    fn not(&mut self) -> Result<Expr, QueryError> {
        if self.eat(&["!", "not"]) {
            return Ok(Expr::Not(Box::new(self.not()?)));
        }
        let left = self.atom()?;
        let op = match self.peek() {
            Some("<") => Cmp::Lt,
            Some("<=") => Cmp::Le,
            Some(">") => Cmp::Gt,
            Some(">=") => Cmp::Ge,
            Some("==") => Cmp::Eq,
            Some("!=") => Cmp::Ne,
            _ => return Ok(left),
        };
        self.at += 1;
        Ok(Expr::Cmp(op, Box::new(left), Box::new(self.atom()?)))
    }

    fn atom(&mut self) -> Result<Expr, QueryError> {
        let tok = self
            .peek()
            .ok_or_else(|| QueryError::Parse("unexpected end".into()))?
            .to_string();
        self.at += 1;
        if tok == "(" {
            let e = self.or()?;
            return if self.eat(&[")"]) {
                Ok(e)
            } else {
                Err(QueryError::Parse("missing `)`".into()))
            };
        }
        if let Ok(n) = tok.parse::<f64>() {
            return Ok(Expr::Num(n));
        }
        if tok.starts_with(|c: char| c.is_alphabetic() || c == '_')
            && !["and", "or", "not"].contains(&tok.as_str())
        {
            return self
                .columns
                .iter()
                .position(|c| *c == tok)
                .map(Expr::Col)
                .ok_or(QueryError::UnknownColumn(tok));
        }
        Err(QueryError::Parse(format!("unexpected `{tok}`")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLS: [&str; 3] = ["port", "food_deficit", "pop"];

    fn hit(q: &str, row: [f64; 3]) -> bool {
        Query::parse(q, &COLS).unwrap().matches(&|i| row[i])
    }

    #[test]
    fn grammar_covers_comparisons_and_or_not() {
        assert!(hit("port && food_deficit > 0", [1.0, 2.0, 0.0]));
        assert!(!hit("port && food_deficit > 0", [0.0, 2.0, 0.0]));
        assert!(hit("port and food_deficit > 0", [1.0, 2.0, 0.0]));
        assert!(hit("!port || pop >= 10", [1.0, 0.0, 10.0]));
        assert!(hit("not (port and pop < 5)", [1.0, 0.0, 6.0]));
        assert!(hit("pop != -1", [0.0, 0.0, 0.0]));
    }

    #[test]
    fn query_names_unknown_column_error() {
        assert_eq!(
            Query::parse("port && famine > 0", &COLS),
            Err(QueryError::UnknownColumn("famine".into()))
        );
    }

    #[test]
    fn malformed_queries_are_parse_errors() {
        for q in ["port &&", "(port", "port $ 1", "port pop"] {
            assert!(
                matches!(Query::parse(q, &COLS), Err(QueryError::Parse(_))),
                "{q}"
            );
        }
    }
}
