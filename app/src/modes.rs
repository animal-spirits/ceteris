//! Map modes (DESIGN 20.1): one RGBA colour per feature, uploaded to the feature colour texture
//! (ADR 0010), plus a legend. Ports `app/src/modes.ts`.

use crate::econ::View;
use crate::web::{Provinces, RESOURCE_GROUPS, TERRAIN};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Political,
    Terrain,
    /// One group (an index into `RESOURCE_GROUPS`), or all with `None`.
    Resources(Option<usize>),
    /// Freight cost from the selected origin.
    Cost,
    /// P2 (econ SPEC, App), by good where they take one.
    Price(usize),
    Output(usize),
    NetExports(usize),
    Wage,
    Unemployment,
}

impl Mode {
    /// Modes that change every tick, so the map interpolates between views.
    pub fn moves(self) -> bool {
        matches!(
            self,
            Self::Price(_)
                | Self::Output(_)
                | Self::NetExports(_)
                | Self::Wage
                | Self::Unemployment
        )
    }
}

pub const NO_DATA: [u8; 3] = [0x9a, 0x9a, 0x96];
/// Neighbour-separating slots.
pub const COUNTRY: [[u8; 3]; 8] = [
    [0x2a, 0x78, 0xd6],
    [0xeb, 0x68, 0x34],
    [0x1b, 0xaf, 0x7a],
    [0xed, 0xa1, 0x00],
    [0xe8, 0x7b, 0xa4],
    [0x00, 0x83, 0x00],
    [0x62, 0x50, 0xd6],
    [0xe3, 0x49, 0x48],
];
pub const TERRAIN_RAMP: [[u8; 3]; 4] = [
    [0xe3, 0xd3, 0xb0],
    [0xc6, 0xa5, 0x71],
    [0x9a, 0x73, 0x42],
    [0x5e, 0x44, 0x24],
];
/// Net imports (blue) to net exports (brown), three magnitude classes a side.
pub const DIVERGING: [[u8; 3]; 6] = [
    [0x2c, 0x5f, 0x8a],
    [0x5b, 0x8d, 0xb8],
    [0xa9, 0xc8, 0xdf],
    [0xec, 0xc4, 0x93],
    [0xc9, 0x7a, 0x3a],
    [0x7f, 0x42, 0x14],
];
pub const SEQUENTIAL: [[u8; 3]; 7] = [
    [0xfd, 0xe0, 0xc5],
    [0xf9, 0xb9, 0x8a],
    [0xf2, 0x8f, 0x53],
    [0xe0, 0x66, 0x2b],
    [0xb8, 0x4a, 0x1b],
    [0x8a, 0x35, 0x12],
    [0x5c, 0x23, 0x0b],
];

pub struct LegendItem {
    pub colour: [u8; 3],
    pub label: String,
}

pub struct Painted {
    pub title: String,
    pub fill: Vec<[u8; 4]>,
    pub legend: Vec<LegendItem>,
    pub note: String,
}

fn rgba([r, g, b]: [u8; 3]) -> [u8; 4] {
    [r, g, b, 255]
}

/// Class breaks at quantiles of the positive finite values, so each colour covers a similar
/// share of provinces. Ties (common with small counts) would leave classes empty, so they merge.
fn quantile_breaks(values: &[f64], classes: usize) -> Vec<f64> {
    let mut v: Vec<f64> = values
        .iter()
        .copied()
        .filter(|x| x.is_finite() && *x > 0.0)
        .collect();
    if v.is_empty() {
        return Vec::new();
    }
    v.sort_by(f64::total_cmp);
    let mut breaks: Vec<f64> = (1..classes)
        .map(|k| v[(k * v.len() / classes).min(v.len() - 1)])
        .filter(|&b| b > v[0])
        .collect();
    breaks.dedup();
    breaks
}

fn classify(x: f64, breaks: &[f64]) -> usize {
    breaks.iter().take_while(|&&b| x >= b).count()
}

/// Whole numbers with thousands separators from 100 up, three significant digits below.
pub fn fmt(x: f64) -> String {
    if x.abs() >= 100.0 || x.fract() == 0.0 {
        let digits = format!("{}", x.abs().round() as u64);
        let mut out = String::new();
        for (k, ch) in digits.chars().enumerate() {
            if k > 0 && (digits.len() - k) % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
        }
        if x < 0.0 { format!("-{out}") } else { out }
    } else {
        let decimals = (2 - x.abs().log10().floor() as i32).max(0) as usize;
        format!("{x:.decimals$}")
    }
}

/// Classes are [lower, next lower); counts label their integer ranges, e.g. "4 to 6".
fn sequential(values: &[f64], unit: &str, zero: Option<&str>) -> (Vec<[u8; 4]>, Vec<LegendItem>) {
    let breaks = quantile_breaks(values, SEQUENTIAL.len());
    let counts = values.iter().all(|x| x.fract() == 0.0);
    let fill = values
        .iter()
        .map(|&x| {
            if !x.is_finite() || (x <= 0.0 && zero.is_some()) {
                rgba(NO_DATA)
            } else {
                rgba(SEQUENTIAL[classify(x, &breaks)])
            }
        })
        .collect();
    let lowest = if zero.is_some() {
        values
            .iter()
            .copied()
            .filter(|&x| x > 0.0)
            .fold(f64::INFINITY, f64::min)
    } else {
        0.0
    };
    let mut legend: Vec<LegendItem> = zero
        .map(|label| LegendItem {
            colour: NO_DATA,
            label: label.to_owned(),
        })
        .into_iter()
        .collect();
    if lowest.is_finite() {
        legend.extend(
            std::iter::once(lowest)
                .chain(breaks.iter().copied())
                .enumerate()
                .map(|(k, lo)| {
                    let hi = breaks.get(k).map(|&b| if counts { b - 1.0 } else { b });
                    let range = match hi {
                        None => format!("{}+", fmt(lo)),
                        Some(hi) if hi == lo => fmt(lo),
                        Some(hi) => format!("{} to {}", fmt(lo), fmt(hi)),
                    };
                    let unit = if range == "1" {
                        unit.trim_end_matches('s')
                    } else {
                        unit
                    };
                    LegendItem {
                        colour: SEQUENTIAL[k],
                        label: format!("{range} {unit}"),
                    }
                }),
        );
    }
    (fill, legend)
}

/// Signed values in three magnitude classes a side, split at the terciles of |x|.
fn diverging(
    values: &[f64],
    unit: &str,
    negative: &str,
    positive: &str,
) -> (Vec<[u8; 4]>, Vec<LegendItem>) {
    let mut mag: Vec<f64> = values
        .iter()
        .filter(|x| x.is_finite() && **x != 0.0)
        .map(|x| x.abs())
        .collect();
    mag.sort_by(f64::total_cmp);
    let at = |k: usize| mag.get(k * mag.len() / 3).copied();
    let (Some(t1), Some(t2)) = (at(1), at(2)) else {
        return (vec![rgba(NO_DATA); values.len()], Vec::new());
    };
    let class = |x: f64| {
        let k = usize::from(x.abs() >= t1) + usize::from(x.abs() >= t2);
        if x < 0.0 { 2 - k } else { 3 + k }
    };
    let fill = values
        .iter()
        .map(|&x| {
            if x.is_finite() {
                rgba(DIVERGING[class(x)])
            } else {
                rgba(NO_DATA)
            }
        })
        .collect();
    let range = |lo: f64, hi: Option<f64>| match hi {
        Some(hi) => format!("{} to {} {unit}", fmt(lo), fmt(hi)),
        None => format!("{}+ {unit}", fmt(lo)),
    };
    let legend = [
        (0, format!("{negative} {}", range(t2, None))),
        (1, format!("{negative} {}", range(t1, Some(t2)))),
        (2, format!("{negative} under {} {unit}", fmt(t1))),
        (3, format!("{positive} under {} {unit}", fmt(t1))),
        (4, format!("{positive} {}", range(t1, Some(t2)))),
        (5, format!("{positive} {}", range(t2, None))),
    ]
    .into_iter()
    .map(|(k, label)| LegendItem {
        colour: DIVERGING[k],
        label,
    })
    .collect();
    (fill, legend)
}

pub fn political(p: &Provinces) -> Vec<[u8; 4]> {
    p.country
        .iter()
        .map(|&c| rgba(COUNTRY[p.countries.colour[c as usize] as usize % COUNTRY.len()]))
        .collect()
}

/// `cost`: USD per tonne from the origin to each province, once an origin is chosen; `econ`:
/// the presented tick's view, once the economy runs.
pub fn paint(mode: Mode, p: &Provinces, cost: Option<&[f64]>, econ: Option<&View>) -> Painted {
    let n = p.len();
    if mode.moves() {
        return match econ {
            Some(v) => economy(mode, n, v),
            None => Painted {
                title: "The economy is loading".into(),
                fill: vec![rgba(NO_DATA); n],
                legend: Vec::new(),
                note: String::new(),
            },
        };
    }
    match mode {
        Mode::Political => Painted {
            title: "Countries, 1 January 2000".into(),
            fill: political(p),
            legend: Vec::new(),
            note: "Colours only separate neighbours; click a province for its country.".into(),
        },
        Mode::Terrain => Painted {
            title: "Terrain".into(),
            fill: p
                .terrain
                .iter()
                .map(|&t| rgba(TERRAIN_RAMP[t as usize]))
                .collect(),
            legend: TERRAIN
                .iter()
                .zip(TERRAIN_RAMP)
                .map(|(label, colour)| LegendItem {
                    colour,
                    label: (*label).into(),
                })
                .collect(),
            note: "From ETOPO 2022 elevation and ruggedness (TRI).".into(),
        },
        Mode::Resources(group) => {
            let values: Vec<f64> = (0..n).map(|i| f64::from(p.sites(group, i))).collect();
            let (fill, legend) = sequential(&values, "sites", Some("none recorded"));
            Painted {
                title: format!("Mineral sites, {}", group.map_or("all groups", |g| RESOURCE_GROUPS[g])),
                fill,
                legend,
                note: "USGS MRDS producing and past-producing sites; about 89% are in the United States."
                    .into(),
            }
        }
        Mode::Cost => match cost {
            None => Painted {
                title: "Freight cost per tonne".into(),
                fill: vec![rgba(NO_DATA); n],
                legend: Vec::new(),
                note: "Click a province to set the origin.".into(),
            },
            Some(cost) => {
                let (fill, legend) = sequential(cost, "USD/t", None);
                Painted {
                    title: "Freight cost per tonne from origin".into(),
                    fill,
                    legend,
                    note: "Least-cost path over road, rail, sea and air; 2000 USD, prior parameters (P04)."
                        .into(),
                }
            }
        },
        _ => unreachable!("moving modes return above"),
    }
}

fn economy(mode: Mode, n: usize, v: &View) -> Painted {
    let at = format!("tick {}", v.tick);
    let seq = |values: Vec<f64>, unit: &str| sequential(&values, unit, None);
    let goods = &v.layout.goods;
    let name = |g: usize| goods.name[g].to_lowercase();
    let (title, (fill, legend), note) = match mode {
        Mode::Price(g) => (
            format!("Price of {}", name(g)),
            seq(v.price_of(n, g), "x world index"),
            format!("Buyer price over the world price index, {at}."),
        ),
        Mode::Output(g) => (
            format!("Output of {}", name(g)),
            seq(v.output_of(n, g), "per month"),
            format!("At the province's market price, numeraire per month, {at}."),
        ),
        Mode::NetExports(g) if goods.is_traded(g) => (
            format!("Net exports of {}", name(g)),
            diverging(&v.net_out_of(n, g), "per month", "in", "out"),
            format!(
                "Sales of the province's market less its buyers' purchases, numeraire per month, {at}."
            ),
        ),
        Mode::NetExports(g) => (
            format!("Net exports of {}", name(g)),
            (vec![rgba(NO_DATA); n], Vec::new()),
            "A local good: it does not ship between provinces.".into(),
        ),
        Mode::Wage => (
            "Wage".into(),
            seq(v.wages(n), "per labour unit"),
            format!("Numeraire per effective labour unit and month, {at}."),
        ),
        _ => (
            "Unemployment".into(),
            seq(v.unemployment(n).iter().map(|u| 100.0 * u).collect(), "%"),
            format!("Share of the labour force not employed, {at}."),
        ),
    };
    Painted {
        title,
        fill,
        legend,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ramps;

    #[test]
    fn ramps_pass_the_0221_check() {
        for (name, ramp) in [
            ("sequential", &SEQUENTIAL[..]),
            ("terrain", &TERRAIN_RAMP[..]),
        ] {
            let problems = ramps::check(ramp, true);
            assert!(problems.is_empty(), "{name}: {problems:?}");
        }
    }

    #[test]
    fn quantile_classes_and_count_labels() {
        let values: Vec<f64> = (0..=14).map(f64::from).collect();
        let (fill, legend) = sequential(&values, "sites", Some("none"));
        assert_eq!(fill[0], rgba(NO_DATA));
        assert_eq!(fill[1], rgba(SEQUENTIAL[0]));
        assert_eq!(fill[14], rgba(SEQUENTIAL[6]));
        assert_eq!(legend[0].label, "none");
        assert_eq!(legend[1].label, "1 to 2 sites");
        assert_eq!(legend.last().map(|l| l.label.as_str()), Some("13+ sites"));
        let (_, singles) = sequential(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0], "sites", Some("none"));
        assert_eq!(singles[1].label, "1 site");
    }

    #[test]
    fn all_zero_values_leave_only_the_zero_class() {
        let (fill, legend) = sequential(&[0.0, 0.0], "sites", Some("none"));
        assert_eq!(fill, vec![rgba(NO_DATA); 2]);
        assert_eq!(legend.len(), 1);
    }

    #[test]
    fn diverging_classes_split_by_sign_and_magnitude() {
        let values = [-9.0, -5.0, -1.0, 1.0, 5.0, 9.0, f64::NAN];
        let (fill, legend) = diverging(&values, "u", "in", "out");
        let want: Vec<[u8; 4]> = DIVERGING.iter().map(|&c| rgba(c)).collect();
        assert_eq!(&fill[..6], &want[..]);
        assert_eq!(fill[6], rgba(NO_DATA));
        assert_eq!(legend.len(), 6);
    }

    #[test]
    fn number_format() {
        assert_eq!(fmt(1234567.4), "1,234,567");
        assert_eq!(fmt(150.6), "151");
        assert_eq!(fmt(12.34), "12.3");
        assert_eq!(fmt(0.0345), "0.0345");
        assert_eq!(fmt(7.0), "7");
    }
}
