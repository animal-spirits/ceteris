//! Stability table of the stock-flow price rule (`specs/trade/SPEC.md`, Stability region): the
//! Jury conditions on the 3 x 3 transition matrix per traded good, over the demand elasticities
//! E in [sigma, theta_g + 1] and the supply elasticities eps of the fixture's enterprises of the
//! good. `ceteris calibrate stability [FIXTURE_DIR]` prints the rows of the table.

use std::collections::BTreeMap;
use std::path::Path;

use crate::write::Table;

pub const NAMES: [&str; 7] = [
    "Agriculture",
    "Crude oil and gas",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals, refined fuels, metals",
    "Machinery, electronics, vehicles",
];
/// Lower end of E: total household demand (P06).
pub const SIGMA_DEMAND: f64 = 0.6;
/// Default mode, frictionless (P41).
pub const ETAS: [f64; 2] = [0.3, 1.0];
pub const STEP: f64 = 0.0005;
/// kappa1 limit "as kappa2 goes to 0".
pub const TINY: f64 = 1e-9;

pub fn stable(e: f64, eta: f64, eps: f64, cover: f64, k1: f64, k2: f64) -> bool {
    let a2 = e * (k1 + k2) + e * k2 / cover + eps * eta * (k1 + k2 / cover) + eta - 3.0;
    let a1 =
        (eta - 2.0) * e * (k1 + k2) + (eta - 1.0) * e * k2 / cover - eps * eta * k1 - 2.0 * eta
            + 3.0;
    let a0 = (1.0 - eta) * (e * (k1 + k2) - 1.0);
    // Zero on the kappa2 = 0 edge, which counts as inside.
    let p1 = 1.0 + a2 + a1 + a0;
    (p1 > 0.0 || p1.abs() < 1e-12)
        && -1.0 + a2 - a1 + a0 < 0.0
        && a0.abs() < 1.0
        && 1.0 - a0 * a0 > (a1 - a0 * a2).abs()
}

/// `num` evenly spaced values from `start` to `stop`, both included, as numpy's `linspace`.
fn linspace(start: f64, stop: f64, num: usize) -> Vec<f64> {
    let div = (num - 1) as f64;
    let delta = stop - start;
    let step = delta / div;
    let mut y: Vec<f64> = (0..num)
        .map(|i| {
            let i = i as f64;
            if step == 0.0 {
                i / div * delta + start
            } else {
                i * step + start
            }
        })
        .collect();
    y[num - 1] = stop;
    y
}

/// The free coordinate of the table: kappa1 with kappa2 fixed, or kappa2 with kappa1 fixed.
#[derive(Clone, Copy, Debug)]
pub enum Free {
    Kappa1 { k2: f64 },
    Kappa2 { k1: f64 },
}

/// First value of the free coordinate on the [`STEP`] grid that leaves the region for some E
/// and eps.
pub fn first_failing(
    theta: f64,
    cover: f64,
    eta: f64,
    eps: (f64, f64),
    free: Free,
) -> Result<f64, String> {
    let mut es = linspace(SIGMA_DEMAND, theta + 1.0, 80);
    es.extend([SIGMA_DEMAND, theta + 1.0]);
    let epss = linspace(eps.0, eps.1, 6);
    for i in 1..4000 {
        let v = f64::from(i) * STEP;
        let (a, b) = match free {
            Free::Kappa1 { k2 } => (v, k2),
            Free::Kappa2 { k1 } => (k1, v),
        };
        if !es
            .iter()
            .all(|&e| epss.iter().all(|&s| stable(e, eta, s, cover, a, b)))
        {
            return Ok(v);
        }
    }
    Err("no limit below 2".into())
}

fn params(fixture: &Path) -> Result<serde_json::Value, String> {
    let path = fixture.join("params.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn number(v: &serde_json::Value, what: &str) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| format!("params.json: {what} is not a number"))
}

/// Supply elasticity of the labour target, s_L sigma_FL / ((1 - s_L) a_VA), range over the
/// enterprises of each good.
pub fn eps_by_good(fixture: &Path) -> Result<BTreeMap<usize, (f64, f64)>, String> {
    let read = |name: &str| Table::open(&fixture.join(format!("{name}.arrow")));
    let (ent, prov, countries, io) = (
        read("enterprises")?,
        read("provinces")?,
        read("countries")?,
        read("country_io")?,
    );
    let sigma_fl = number(&params(fixture)?["econ"]["sigma_fl"], "econ.sigma_fl")?;
    let s_l: BTreeMap<usize, f64> = countries
        .usize("country")?
        .into_iter()
        .zip(countries.f64("s_l")?)
        .collect();
    let country: BTreeMap<usize, usize> = prov
        .usize("province")?
        .into_iter()
        .zip(prov.usize("country")?)
        .collect();
    let mut inputs: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for ((c, g), a) in io
        .usize("country")?
        .into_iter()
        .zip(io.usize("sector")?)
        .zip(io.f64("a")?)
    {
        *inputs.entry((c, g)).or_insert(0.0) += a;
    }
    let mut eps: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    for (p, g) in ent.usize("province")?.into_iter().zip(ent.usize("good")?) {
        let c = *country
            .get(&p)
            .ok_or_else(|| format!("enterprise in unknown province {p}"))?;
        let s = *s_l
            .get(&c)
            .ok_or_else(|| format!("province in unknown country {c}"))?;
        let a = inputs.get(&(c, g)).copied().unwrap_or(0.0);
        let e = s * sigma_fl / ((1.0 - s) * (1.0 - a));
        let range = eps.entry(g).or_insert((e, e));
        *range = (range.0.min(e), range.1.max(e));
    }
    Ok(eps)
}

/// `x` rounded to 4 decimals, then to 3 with ties toward zero.
pub fn fmt(x: f64) -> String {
    let four = format!("{x:.4}");
    let negative = four.starts_with('-');
    let digits: String = four.chars().filter(char::is_ascii_digit).collect();
    let n4: u64 = digits.parse().expect("digits");
    let (q, r) = (n4 / 10, n4 % 10);
    let q = if r > 5 { q + 1 } else { q };
    let sign = if negative && q > 0 { "-" } else { "" };
    format!("{sign}{}.{:03}", q / 1000, q % 1000)
}

/// The table's rows for the fixture in `fixture`.
pub fn table(fixture: &Path) -> Result<Vec<String>, String> {
    let trade = &params(fixture)?["trade"];
    let eps = eps_by_good(fixture)?;
    NAMES
        .iter()
        .enumerate()
        .map(|(g, name)| {
            let theta = number(&trade["theta"][g], "trade.theta")?;
            let cover = number(&trade["coverage"][g], "trade.coverage")?;
            let range = *eps
                .get(&g)
                .ok_or_else(|| format!("no enterprise of good {g}"))?;
            let cells = [
                Free::Kappa1 { k2: TINY },
                Free::Kappa2 { k1: 0.05 },
                Free::Kappa2 { k1: 0.02 },
            ]
            .iter()
            .map(|&free| {
                let [a, b] =
                    [ETAS[0], ETAS[1]].map(|eta| first_failing(theta, cover, eta, range, free));
                Ok(format!("{} / {}", fmt(a?), fmt(b?)))
            })
            .collect::<Result<Vec<_>, String>>()?;
            Ok(format!(
                "| {g} {name} | {theta:.2} | {} | {} |",
                crate::gravity::py_g(cover, 6),
                cells.join(" | ")
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `str(Decimal(str(round(x, 4))).quantize(Decimal("0.001"), rounding=ROUND_HALF_DOWN))`.
    #[test]
    fn fmt_rounds_half_down() {
        for (x, want) in [
            (0.0035, "0.003"),
            (0.0036, "0.004"),
            (0.168, "0.168"),
            (0.1685, "0.168"),
            (0.16851, "0.168"),
            (0.16861, "0.169"),
            (1.0, "1.000"),
            (0.0005, "0.000"),
        ] {
            assert_eq!(fmt(x), want, "{x}");
        }
    }

    /// numpy's `linspace(0.6, 11.57, 80)` at a few points and `linspace(0.5, 0.5, 6)`.
    #[test]
    fn linspace_includes_both_ends() {
        let y = linspace(0.6, 11.57, 80);
        assert_eq!(y.len(), 80);
        assert_eq!(y[0], 0.6);
        assert_eq!(y[79], 11.57);
        assert_eq!(y[1], 0.6 + (11.57 - 0.6) / 79.0);
        assert_eq!(linspace(0.5, 0.5, 6), vec![0.5; 6]);
    }
}
