//! Stability table of the stock-flow price rule (`specs/trade/SPEC.md`, Stability region): the
//! Jury conditions on the 3 x 3 transition matrix per traded good, over the demand elasticities
//! E in [sigma, theta_g + 1] and the supply elasticities eps of the fixture's enterprises of the
//! good, and the storage loop of the competitively stored goods (ADR 0091) by the Schur-Cohn test
//! on its 4 x 4 transition matrix. `ceteris calibrate stability [--storage] [FIXTURE_DIR]`
//! prints the rows of the first table, or with `--storage` of the second.

use std::collections::BTreeMap;
use std::path::Path;

use crate::write::Table;

pub const NAMES: [&str; 7] = [
    "Agriculture",
    "Crude oil, gas and refined fuels",
    "Coal and other mining",
    "Food",
    "Light manufacturing",
    "Chemicals and metals",
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

/// Competitive storage at one origin market (ADR 0091): P56 delta and k, P145 zeta, P144 psi,
/// P146 g_s.
#[derive(Clone, Copy, Debug)]
pub struct Storage {
    pub delta: f64,
    pub cost: f64,
    pub zeta: f64,
    pub psi: f64,
    pub gain: f64,
}

/// The transition matrix of (x_t, q_{t-1}, sigma_t, pi_t) of a competitively stored good's
/// origin market at price gain `kappa` (trade SPEC, Storage loop).
pub fn storage_matrix(
    e: f64,
    eta: f64,
    eps: f64,
    cover: f64,
    kappa: f64,
    st: Storage,
) -> [[f64; 4]; 4] {
    let c = st.zeta * (st.cost + st.delta);
    let rho = 1.0 / (1.0 + st.delta * cover);
    let b = 1.0 - eta;
    let a = rho * e * (1.0 + st.psi * c) + eta * eps;
    let cs = rho * (st.delta * cover - st.psi * c);
    let g = st.gain;
    [
        [
            1.0 - kappa * a,
            -kappa * b,
            kappa * cs,
            kappa * rho * st.psi,
        ],
        [eta * eps, b, 0.0, 0.0],
        [
            (e + eta * eps / rho) / cover,
            b / (rho * cover),
            1.0 - st.delta,
            0.0,
        ],
        [
            -g * kappa * a,
            -g * kappa * b,
            g * kappa * cs,
            1.0 - g + g * kappa * rho * st.psi,
        ],
    ]
}

/// Coefficients c_0..c_n (c_n = 1) of det(lambda I - m), by Faddeev-LeVerrier.
fn char_poly<const N: usize>(m: &[[f64; N]; N]) -> Vec<f64> {
    let mut c = vec![0.0; N + 1];
    c[N] = 1.0;
    let mut mk = [[0.0; N]; N];
    for k in 1..=N {
        let mut next = [[0.0; N]; N];
        for i in 0..N {
            for j in 0..N {
                next[i][j] = (0..N).map(|l| m[i][l] * mk[l][j]).sum::<f64>()
                    + if i == j { c[N + 1 - k] } else { 0.0 };
            }
        }
        mk = next;
        let trace: f64 = (0..N)
            .map(|i| (0..N).map(|l| m[i][l] * mk[l][i]).sum::<f64>())
            .sum();
        c[N - k] = -trace / k as f64;
    }
    c
}

/// Schur-Cohn: every root of sum c_i lambda^i strictly inside the unit circle.
fn schur_stable(mut c: Vec<f64>) -> bool {
    while c.len() > 1 {
        let n = c.len() - 1;
        if c[0].abs() >= c[n].abs() {
            return false;
        }
        c = (0..n)
            .map(|j| c[n] * c[j + 1] - c[0] * c[n - 1 - j])
            .collect();
    }
    true
}

/// The storage loop is stable: the spectral radius of [`storage_matrix`] below 1.
pub fn storage_stable(e: f64, eta: f64, eps: f64, cover: f64, kappa: f64, st: Storage) -> bool {
    schur_stable(char_poly(&storage_matrix(e, eta, eps, cover, kappa, st)))
}

/// First kappa1 on the [`STEP`] grid at which the storage loop leaves the region for some E in
/// [sigma, theta + 1] and eps in `eps`.
pub fn first_failing_storage(
    theta: f64,
    cover: f64,
    eta: f64,
    eps: (f64, f64),
    st: Storage,
) -> Result<f64, String> {
    let mut es = linspace(SIGMA_DEMAND, theta + 1.0, 80);
    es.extend([SIGMA_DEMAND, theta + 1.0]);
    let epss = linspace(eps.0, eps.1, 6);
    for i in 1..4000 {
        let k = f64::from(i) * STEP;
        if !es.iter().all(|&e| {
            epss.iter()
                .all(|&s| storage_stable(e, eta, s, cover, k, st))
        }) {
            return Ok(k);
        }
    }
    Err("no limit below 2".into())
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

/// The storage parameters of good `g` in params.json's trade section; where storage is off
/// there (psi = delta = 0, the default), the approved values of `TradeParams::with_storage`.
fn storage_of(trade: &serde_json::Value, g: usize) -> Result<Storage, String> {
    let at = |name: &str| number(&trade[name][g], &format!("trade.{name}"));
    let (mut delta, mut psi) = (at("storage_loss")?, at("speculation")?);
    if delta == 0.0 && psi == 0.0 {
        let on = ceteris_trade::TradeParams::default().with_storage();
        (delta, psi) = (on.storage_loss[g], on.speculation[g]);
    }
    Ok(Storage {
        delta,
        cost: at("storage_cost")?,
        zeta: at("convenience")?,
        psi,
        gain: number(&trade["storage_gain"], "trade.storage_gain")?,
    })
}

/// The storage table's rows for the fixture in `fixture` (trade SPEC, Storage loop): per
/// storable good the kappa1 limit of the storage loop, default mode / frictionless, over E in
/// [sigma, theta + 1] and the fixture's eps, at the fixture's storage values or, where storage
/// is off, the approved ones.
pub fn storage_table(fixture: &Path) -> Result<Vec<String>, String> {
    let trade = &params(fixture)?["trade"];
    let eps = eps_by_good(fixture)?;
    [0usize, 1, 2, 5]
        .iter()
        .map(|&g| {
            let theta = number(&trade["theta"][g], "trade.theta")?
                * number(&trade["theta_scale"], "trade.theta_scale")?;
            let cover = number(&trade["coverage"][g], "trade.coverage")?;
            let st = storage_of(trade, g)?;
            let range = *eps
                .get(&g)
                .ok_or_else(|| format!("no enterprise of good {g}"))?;
            let [a, b] =
                [ETAS[0], ETAS[1]].map(|eta| first_failing_storage(theta, cover, eta, range, st));
            Ok(format!(
                "| {g} {} | {theta:.2} | {} | {} / {} |",
                NAMES[g],
                crate::pyfmt::py_g(cover, 6),
                fmt(a?),
                fmt(b?)
            ))
        })
        .collect()
}

/// The table's rows for the fixture in `fixture`.
pub fn table(fixture: &Path) -> Result<Vec<String>, String> {
    let trade = &params(fixture)?["trade"];
    let eps = eps_by_good(fixture)?;
    NAMES
        .iter()
        .enumerate()
        .map(|(g, name)| {
            let theta = number(&trade["theta"][g], "trade.theta")?
                * number(&trade["theta_scale"], "trade.theta_scale")?;
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
                crate::pyfmt::py_g(cover, 6),
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

    /// Without storage (psi = delta = 0) the storage loop is the 3 x 3 map at kappa2 = 0 times a
    /// decoupled root 1 - g_s.
    #[test]
    fn storage_off_is_the_stock_rule_at_kappa2_0() {
        let off = Storage {
            delta: 0.0,
            cost: 0.009,
            zeta: 1.8,
            psi: 0.0,
            gain: 0.08,
        };
        let c = char_poly(&storage_matrix(3.0, 0.3, 1.2, 2.0, 0.05, off));
        // (lambda - (1 - g_s)) times the 3 x 3 polynomial at kappa2 0
        let (e, eta, eps, k1) = (3.0, 0.3, 1.2, 0.05);
        let a2 = e * k1 + eps * eta * k1 + eta - 3.0;
        let a1 = (eta - 2.0) * e * k1 - eps * eta * k1 - 2.0 * eta + 3.0;
        let a0 = (1.0 - eta) * (e * k1 - 1.0);
        let r = 0.92;
        let want = [-r * a0, a0 - r * a1, a1 - r * a2, a2 - r, 1.0];
        for (got, want) in c.iter().zip(want) {
            assert!((got - want).abs() < 1e-14, "{c:?}");
        }
        // the stock's unit root of the kappa2 = 0 edge is not strictly inside
        assert!(!storage_stable(e, eta, eps, 2.0, k1, off));
    }

    /// Schur-Cohn against known roots.
    #[test]
    fn schur_cohn_counts_the_unit_circle() {
        // (l - 0.5)(l + 0.9)(l - 0.99) inside; (l - 1.01)(l + 0.2) outside
        let inside = [0.4455, -0.846, -0.59, 1.0];
        assert!(schur_stable(inside.to_vec()));
        assert!(!schur_stable(vec![-0.202, -0.81, 1.0]));
        assert!(!schur_stable(vec![-1.0, 0.0, 1.0]));
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
