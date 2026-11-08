//! `ceteris calibrate rho` (ADR 0027, register P53, specs/p3/PLAN.md Task E2): the monthly
//! adjustment rate of each theta class from local projections of bilateral flows on a tariff
//! change.
//!
//! The panel has one row per (class, pair, year): the flow and the applied tariff. For each class
//! and each year `y` the projection regresses `ln X(y) - ln X(base)` on the pair's tariff change
//! `ln(1 + tau(shock)) - ln(1 + tau(base))` across pairs, giving `beta_y`. Under the partial
//! adjustment of ADR 0027 the response `m` months after the shock is
//! `-theta (1 - (1 - rho)^m)`, so `rho` is found by a grid over half-lives with theta concentrated
//! out (it is linear given rho). Years before the shock are placebo coefficients and stay out of
//! the fit. The BACI and tariff extraction that fills the panel is separate; the estimator reads
//! the panel from a CSV.

use std::collections::BTreeMap;

/// Half-lives of the grid, in months, step 1; `rho = 1` (instant adjustment) is also tried.
pub const MAX_HALF_LIFE: u32 = 120;

/// Prior band of the half-life, in months (ADR 0027).
pub const PRIOR_HALF_LIFE: (f64, f64) = (12.0, 48.0);

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub class: String,
    pub pair: String,
    pub year: i32,
    pub flow: f64,
    pub tariff: f64,
}

/// The event: tariffs move between `base` and `shock`, and a flow is read `months` after the
/// start of the shock year plus half a year (annual flows are year averages).
#[derive(Clone, Copy, Debug)]
pub struct Design {
    pub base: i32,
    pub shock: i32,
}

impl Design {
    fn months(&self, year: i32) -> f64 {
        12.0 * f64::from(year - self.shock) + 6.0
    }
}

#[derive(Clone, Debug)]
pub struct Projection {
    pub year: i32,
    pub beta: f64,
    pub se: f64,
}

#[derive(Clone, Debug)]
pub struct ClassFit {
    pub class: String,
    /// Pairs with a flow in the base year and the year being projected.
    pub pairs: usize,
    pub projections: Vec<Projection>,
    /// Monthly adjustment rate, 1 for instant adjustment.
    pub rho: f64,
    pub half_life_months: f64,
    /// Long-run elasticity implied by the fit.
    pub theta: f64,
    /// Weighted sum of squared projection residuals at the optimum.
    pub sse: f64,
}

impl ClassFit {
    pub fn in_prior_band(&self) -> bool {
        (PRIOR_HALF_LIFE.0..=PRIOR_HALF_LIFE.1).contains(&self.half_life_months)
    }
}

/// Parses `class,pair,year,flow,tariff` lines; a header line and `#` comments are skipped.
pub fn parse_panel(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("class,") {
            continue;
        }
        let f: Vec<&str> = line.split(',').map(str::trim).collect();
        let [class, pair, year, flow, tariff] = f[..] else {
            return Err(format!(
                "line {}: expected class,pair,year,flow,tariff",
                i + 1
            ));
        };
        let num = |s: &str, what: &str| {
            s.parse::<f64>()
                .map_err(|_| format!("line {}: bad {what} '{s}'", i + 1))
        };
        rows.push(Row {
            class: class.into(),
            pair: pair.into(),
            year: year
                .parse()
                .map_err(|_| format!("line {}: bad year '{year}'", i + 1))?,
            flow: num(flow, "flow")?,
            tariff: num(tariff, "tariff")?,
        });
    }
    Ok(rows)
}

/// OLS slope and its homoskedastic standard error of `y` on `x` with a constant.
fn slope(x: &[f64], y: &[f64]) -> Option<(f64, f64)> {
    let n = x.len();
    if n < 3 {
        return None;
    }
    let (mx, my) = (
        x.iter().sum::<f64>() / n as f64,
        y.iter().sum::<f64>() / n as f64,
    );
    let sxx: f64 = x.iter().map(|v| (v - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let b = x
        .iter()
        .zip(y)
        .map(|(a, c)| (a - mx) * (c - my))
        .sum::<f64>()
        / sxx;
    let rss: f64 = x
        .iter()
        .zip(y)
        .map(|(a, c)| (c - my - b * (a - mx)).powi(2))
        .sum();
    Some((b, (rss / (n as f64 - 2.0) / sxx).sqrt()))
}

/// Local projections for one class: one coefficient per year other than the base.
fn project(rows: &[&Row], design: Design) -> (usize, Vec<Projection>) {
    let mut by_pair: BTreeMap<&str, BTreeMap<i32, (f64, f64)>> = BTreeMap::new();
    for r in rows {
        if r.flow > 0.0 {
            by_pair
                .entry(&r.pair)
                .or_default()
                .insert(r.year, (r.flow.ln(), (1.0 + r.tariff).ln()));
        }
    }
    let years: Vec<i32> = by_pair
        .values()
        .flat_map(|m| m.keys().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|&y| y != design.base)
        .collect();
    let mut pairs = 0;
    let mut out = Vec::new();
    for y in years {
        let (mut x, mut d) = (Vec::new(), Vec::new());
        for m in by_pair.values() {
            if let (Some(b), Some(s), Some(t)) =
                (m.get(&design.base), m.get(&design.shock), m.get(&y))
            {
                x.push(s.1 - b.1);
                d.push(t.0 - b.0);
            }
        }
        if let Some((beta, se)) = slope(&x, &d) {
            pairs = pairs.max(x.len());
            out.push(Projection { year: y, beta, se });
        }
    }
    (pairs, out)
}

/// Fits `rho` to the post-shock coefficients: for each grid rho, theta minimises the weighted
/// squared distance to `-theta g(m)` with `g = 1 - (1 - rho)^m`, in closed form.
fn fit_rho(projections: &[Projection], design: Design) -> Option<(f64, f64, f64)> {
    let post: Vec<(f64, f64, f64)> = projections
        .iter()
        .filter(|p| p.year >= design.shock && p.se > 0.0)
        .map(|p| (design.months(p.year), p.beta, 1.0 / (p.se * p.se)))
        .collect();
    if post.len() < 2 {
        return None;
    }
    let candidates = (1..=MAX_HALF_LIFE)
        .map(|h| 1.0 - 0.5f64.powf(1.0 / f64::from(h)))
        .chain(std::iter::once(1.0));
    let mut best: Option<(f64, f64, f64)> = None;
    for rho in candidates {
        let g = |m: f64| 1.0 - (1.0 - rho).powf(m);
        let num: f64 = post.iter().map(|&(m, b, w)| -w * b * g(m)).sum();
        let den: f64 = post.iter().map(|&(m, _, w)| w * g(m).powi(2)).sum();
        let theta = num / den;
        let sse: f64 = post
            .iter()
            .map(|&(m, b, w)| w * (b + theta * g(m)).powi(2))
            .sum();
        if best.is_none_or(|(_, _, s)| sse < s) {
            best = Some((rho, theta, sse));
        }
    }
    best
}

/// One fit per class, in class order; classes with too few pairs or post-shock years are
/// reported in the second list.
pub fn estimate(rows: &[Row], design: Design) -> (Vec<ClassFit>, Vec<String>) {
    let mut classes: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        classes.entry(&r.class).or_default().push(r);
    }
    let (mut fits, mut skipped) = (Vec::new(), Vec::new());
    for (class, rs) in classes {
        let (pairs, projections) = project(&rs, design);
        match fit_rho(&projections, design) {
            Some((rho, theta, sse)) => fits.push(ClassFit {
                class: class.into(),
                pairs,
                projections,
                rho,
                half_life_months: if rho >= 1.0 {
                    0.0
                } else {
                    (0.5f64.ln() / (1.0 - rho).ln()).abs()
                },
                theta,
                sse,
            }),
            None => skipped.push(class.into()),
        }
    }
    (fits, skipped)
}

pub fn report(fits: &[ClassFit], skipped: &[String]) -> String {
    let mut s = String::from("class,pairs,rho,half_life_months,theta,in_prior_band\n");
    for f in fits {
        s += &format!(
            "{},{},{:.6},{:.1},{:.3},{}\n",
            f.class,
            f.pairs,
            f.rho,
            f.half_life_months,
            f.theta,
            f.in_prior_band()
        );
    }
    for f in fits {
        for p in &f.projections {
            s += &format!(
                "# {} {} beta {:.4} se {:.4}\n",
                f.class, p.year, p.beta, p.se
            );
        }
    }
    for c in skipped {
        s += &format!("# {c}: too few pairs or post-shock years\n");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESIGN: Design = Design {
        base: 2017,
        shock: 2019,
    };

    /// Flows that respond to the tariff change with partial adjustment at `rho`, plus a small
    /// deterministic disturbance.
    fn simulate(class: &str, rho: f64, theta: f64, pairs: usize) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut seed = 12345u64;
        let mut noise = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / f64::from(1u32 << 31) - 0.5) * 0.02
        };
        for p in 0..pairs {
            let shock = 0.02 + 0.1 * (p as f64 / pairs as f64);
            let level = 10.0 + (p % 7) as f64;
            for year in 2016..=2024 {
                let tau = if year >= DESIGN.shock { shock } else { 0.0 };
                let m = DESIGN.months(year).max(0.0);
                let g = if year >= DESIGN.shock {
                    1.0 - (1.0 - rho).powf(m)
                } else {
                    0.0
                };
                let ln_flow = level - theta * g * (1.0 + shock).ln() + noise();
                rows.push(Row {
                    class: class.into(),
                    pair: format!("p{p}"),
                    year,
                    flow: ln_flow.exp(),
                    tariff: tau,
                });
            }
        }
        rows
    }

    #[test]
    fn recovers_half_life_from_simulated_flows() {
        let rho = 1.0 - 0.5f64.powf(1.0 / 24.0);
        let (fits, skipped) = estimate(&simulate("c", rho, 4.0, 400), DESIGN);
        assert!(skipped.is_empty());
        let f = &fits[0];
        assert!(
            (f.half_life_months - 24.0).abs() <= 3.0,
            "{}",
            f.half_life_months
        );
        assert!((f.theta - 4.0).abs() < 0.4, "{}", f.theta);
        assert!(f.in_prior_band());
    }

    #[test]
    fn instant_adjustment_gives_rho_one() {
        let (fits, _) = estimate(&simulate("c", 1.0, 3.0, 400), DESIGN);
        assert!(fits[0].rho >= 0.9, "{}", fits[0].rho);
        assert!(fits[0].half_life_months <= 1.0);
    }

    #[test]
    fn pre_shock_projection_is_a_zero_placebo() {
        let rho = 1.0 - 0.5f64.powf(1.0 / 24.0);
        let (fits, _) = estimate(&simulate("c", rho, 4.0, 400), DESIGN);
        let p = fits[0].projections.iter().find(|p| p.year == 2016).unwrap();
        assert!(p.beta.abs() < 3.0 * p.se + 1e-9, "{p:?}");
    }

    #[test]
    fn classes_are_fitted_separately() {
        let mut rows = simulate("slow", 1.0 - 0.5f64.powf(1.0 / 48.0), 4.0, 300);
        rows.extend(simulate("fast", 1.0 - 0.5f64.powf(1.0 / 12.0), 4.0, 300));
        let (fits, _) = estimate(&rows, DESIGN);
        let by = |c: &str| fits.iter().find(|f| f.class == c).unwrap().half_life_months;
        assert!(by("fast") < by("slow"));
    }

    #[test]
    fn class_without_variation_is_skipped() {
        let mut rows = simulate("c", 0.1, 3.0, 2);
        rows.retain(|r| r.year <= DESIGN.base);
        assert_eq!(estimate(&rows, DESIGN).1, vec!["c".to_string()]);
    }

    #[test]
    fn parse_panel_reads_rows_and_rejects_bad_lines() {
        let rows = parse_panel("class,pair,year,flow,tariff\n# c\nA,x-y,2017,10.5,0.02\n").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].year, 2017);
        assert!(parse_panel("A,x,2017,10\n").is_err());
        assert!(parse_panel("A,x,abc,10,0\n").is_err());
    }
}
