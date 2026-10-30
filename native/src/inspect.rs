//! Inspector rows (DESIGN 20.2): every attribute of one province, its county, and the freight
//! cost from the origin. Ports `app/src/Inspector.tsx`.

use crate::freight::Costs;
use crate::web::{Counties, Provinces, RESOURCE_GROUPS, TERRAIN};

pub struct Row {
    pub key: String,
    pub value: String,
    /// Set in the number face (ADR 0137).
    pub number: bool,
}

/// Thousands separators and at most `digits` decimals, trailing zeros dropped.
pub fn num(x: f64, digits: usize) -> String {
    let s = format!("{:.digits$}", x.abs());
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    let (int, frac) = s.split_once('.').map_or((s, ""), |(i, f)| (i, f));
    let mut out = String::new();
    for (k, ch) in int.chars().enumerate() {
        if k > 0 && (int.len() - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if !frac.is_empty() {
        out = format!("{out}.{frac}");
    }
    if x < 0.0 && out.chars().any(|c| c != '0' && c != '.' && c != ',') {
        out.insert(0, '-');
    }
    out
}

fn text(key: &str, value: impl Into<String>) -> Row {
    Row {
        key: key.into(),
        value: value.into(),
        number: false,
    }
}

fn number(key: &str, value: impl Into<String>) -> Row {
    Row {
        number: true,
        ..text(key, value)
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

pub fn rows(
    p: &Provinces,
    i: usize,
    counties: Option<&Counties>,
    county: Option<u32>,
    costs: Option<&Costs>,
) -> Vec<Row> {
    let c = p.country[i] as usize;
    let mut rows = vec![
        text("Province ID", &p.id[i]),
        text(
            "Country (2000)",
            format!("{} ({})", p.countries.name[c], p.countries.code[c]),
        ),
        text("Sovereign", &p.countries.sovereign[c]),
        number("Area", format!("{} km²", num(p.area_km2[i], 0))),
        number("Mean elevation", format!("{} m", num(p.elev_mean_m[i], 0))),
        number("Ruggedness (TRI)", format!("{} m", num(p.tri_m[i], 1))),
        text("Terrain", TERRAIN[p.terrain[i] as usize]),
        text("Coastal", yes_no(p.coastal[i])),
        number("Ports", p.ports[i].to_string()),
        number("Airports", p.airports[i].to_string()),
        text("Rail border crossing", yes_no(p.rail[i])),
        text("Development slice", yes_no(p.dev_slice[i])),
    ];
    rows.extend(
        RESOURCE_GROUPS
            .iter()
            .enumerate()
            .map(|(g, name)| number(&format!("Sites, {name}"), p.sites(Some(g), i).to_string())),
    );
    // A click near a border can hit a county clipped to the neighbouring province; show it only
    // if it nests here.
    if let (Some(k), Some(cs)) = (county.map(|k| k as usize), counties)
        && cs.province[k] as usize == i
    {
        let name = if cs.name[k].is_empty() {
            "unnamed"
        } else {
            &cs.name[k]
        };
        rows.extend([
            text("County", name),
            text("County level", &cs.level[k]),
            number("County area", format!("{} km²", num(cs.area_km2[k], 0))),
            number(
                "County elevation",
                format!("{} m", num(cs.elev_mean_m[k], 0)),
            ),
            number("County TRI", format!("{} m", num(cs.tri_m[k], 1))),
        ]);
    }
    if let Some(costs) = costs {
        rows.extend([
            text("Origin", &p.name[costs.origin as usize]),
            number("Freight cost", format!("{} USD/t", num(costs.cost[i], 2))),
            number(
                "Transit time",
                format!("{} days", num(costs.hours[i] / 24.0, 1)),
            ),
        ]);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_format() {
        assert_eq!(num(1234567.0, 0), "1,234,567");
        assert_eq!(num(20597.0, 0), "20,597");
        assert_eq!(num(56.1, 1), "56.1");
        assert_eq!(num(3.0, 2), "3");
        assert_eq!(num(-282.0, 0), "-282");
        assert_eq!(num(1234.567, 2), "1,234.57");
    }
}
