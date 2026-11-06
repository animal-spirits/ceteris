//! Initial drift of the pack countries (ADR 0103; specs/econ/SPEC.md, Initial drift).
//!
//! TFP: the 1995-2000 log-linear trend of PWT rtfpna less the world's (the 2000 GDP-weighted
//! mean trend), shrunk to its World Bank region's mean by empirical Bayes: g = m_r + B (g_raw -
//! m_r), B = tau^2 / (tau^2 + se^2), se the trend's standard error and tau^2 the variance of the
//! deviations from the regional means less their mean se^2 (method of moments, at least 0).
//! ADR 0108's catch-up term is not built, so nothing is netted out yet.
//!
//! Capital: the log gap between the 1995-2000 steady-state capital-output ratio s / (n + g +
//! delta) (means of csh_i and delta, trends of population and GDP per person) and the 2000 ratio
//! rnna / rgdpna; F rises by gap / s_L in total, which closes the gap at fixed A and L where VA
//! is Cobb-Douglas.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, Float64Array, StringArray};

use crate::read::ByYear;

type ByEconomy = BTreeMap<String, f64>;

/// The trend window.
pub const FIRST: i64 = 1995;
pub const LAST: i64 = 2000;

/// Where a country's drift comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Its own PWT series.
    Pwt,
    /// Its region's mean TFP drift, no capital drift.
    Region,
    /// Neither: no drift.
    None,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Pwt => "pwt",
            Source::Region => "region",
            Source::None => "none",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drift {
    /// TFP drift g_A, per year.
    pub g_a: f64,
    /// Total log rise of F.
    pub f_gap: f64,
    pub source: Source,
}

/// OLS slope and its squared standard error of ln `series` on the years of the window, where
/// every year has a positive value.
pub fn log_trend(series: &ByYear) -> Option<(f64, f64)> {
    let points: Vec<(f64, f64)> = (FIRST..=LAST)
        .map(|y| {
            let v = *series.get(&y)?;
            (v > 0.0).then(|| (y as f64, v.ln()))
        })
        .collect::<Option<_>>()?;
    let n = points.len() as f64;
    let (mx, my) = (
        points.iter().map(|p| p.0).sum::<f64>() / n,
        points.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let b = sxy / sxx;
    let ssr: f64 = points
        .iter()
        .map(|p| (p.1 - my - b * (p.0 - mx)).powi(2))
        .sum();
    Some((b, ssr / (n - 2.0) / sxx))
}

fn mean_over_window(series: &ByYear) -> Option<f64> {
    let v: Vec<f64> = (FIRST..=LAST)
        .map(|y| series.get(&y).copied())
        .collect::<Option<_>>()?;
    Some(v.iter().sum::<f64>() / v.len() as f64)
}

/// ln(K/Y* / K/Y 2000) of one economy, K/Y* = s / (n + g + delta); `None` where a series is
/// missing or n + g + delta is not positive.
pub fn capital_gap(pwt: &BTreeMap<&str, BTreeMap<String, ByYear>>, e: &str) -> Option<f64> {
    let get = |name: &str| pwt.get(name)?.get(e);
    let (gdp, pop) = (get("rgdpna")?, get("pop")?);
    let per_person: ByYear = gdp
        .iter()
        .filter_map(|(y, v)| Some((*y, v / pop.get(y).filter(|p| **p > 0.0)?)))
        .collect();
    let (n, _) = log_trend(pop)?;
    let (g, _) = log_trend(&per_person)?;
    let s = mean_over_window(get("csh_i")?)?;
    let delta = mean_over_window(get("delta")?)?;
    let ky = get("rnna")?.get(&LAST)? / gdp.get(&LAST).filter(|y| **y > 0.0)?;
    let rate = n + g + delta;
    (rate > 0.0 && s > 0.0 && ky > 0.0).then(|| (s / rate / ky).ln())
}

/// Shrunk relative TFP trends by economy: `trend` the raw (slope, se^2), `weight` 2000 GDP,
/// `region` the World Bank region.
pub fn shrunk_tfp(
    trend: &BTreeMap<String, (f64, f64)>,
    weight: &BTreeMap<String, f64>,
    region: &BTreeMap<String, String>,
) -> Result<(ByEconomy, ByEconomy), String> {
    let weighted: Vec<(f64, f64)> = trend
        .iter()
        .filter_map(|(e, (b, _))| Some((*b, *weight.get(e).filter(|w| **w > 0.0)?)))
        .collect();
    let total: f64 = weighted.iter().map(|p| p.1).sum();
    if total <= 0.0 {
        return Err("no PWT TFP trend with 2000 GDP".into());
    }
    let world = weighted.iter().map(|(b, w)| b * w).sum::<f64>() / total;
    let raw: BTreeMap<&str, (f64, f64, &str)> = trend
        .iter()
        .filter_map(|(e, (b, se2))| Some((e.as_str(), (b - world, *se2, region.get(e)?.as_str()))))
        .collect();
    let mut sums: BTreeMap<&str, (f64, usize)> = BTreeMap::new();
    for (g, _, r) in raw.values() {
        let s = sums.entry(r).or_default();
        s.0 += g;
        s.1 += 1;
    }
    let means: BTreeMap<String, f64> = sums
        .iter()
        .map(|(r, (s, n))| ((*r).to_owned(), s / *n as f64))
        .collect();
    let n = raw.len() as f64;
    let spread = raw
        .values()
        .map(|(g, _, r)| (g - means[*r]).powi(2))
        .sum::<f64>()
        / n;
    let noise = raw.values().map(|(_, se2, _)| se2).sum::<f64>() / n;
    let tau2 = (spread - noise).max(0.0);
    let shrunk = raw
        .iter()
        .map(|(e, (g, se2, r))| {
            let (m, b) = (means[*r], tau2 / (tau2 + se2));
            (
                (*e).to_owned(),
                if b.is_nan() { m } else { m + b * (g - m) },
            )
        })
        .collect();
    Ok((shrunk, means))
}

/// Each pack country's drift: over its economies (`parts`) with a shrunk TFP trend, their 2000
/// GDP-weighted mean g_A and capital gap (0 where none has one), the gap over the country's
/// labour share; without one, its first economy's regional mean g_A and no capital drift.
pub fn drifts(
    codes: &[String],
    parts: &dyn Fn(&str) -> Vec<String>,
    labour_share: &dyn Fn(&str) -> Option<f64>,
    pwt: &BTreeMap<&str, BTreeMap<String, ByYear>>,
    region: &BTreeMap<String, String>,
) -> Result<Vec<Drift>, String> {
    let series = |name: &str| pwt.get(name).cloned().unwrap_or_default();
    let trend: BTreeMap<String, (f64, f64)> = series("rtfpna")
        .iter()
        .filter_map(|(e, s)| Some((e.clone(), log_trend(s)?)))
        .collect();
    let gdp: BTreeMap<String, f64> = series("rgdpna")
        .iter()
        .filter_map(|(e, s)| Some((e.clone(), *s.get(&LAST)?)))
        .collect();
    let (shrunk, means) = shrunk_tfp(&trend, &gdp, region)?;
    codes
        .iter()
        .map(|code| {
            let ps = parts(code);
            let own: Vec<(f64, f64, f64)> = ps
                .iter()
                .filter_map(|e| {
                    let g = *shrunk.get(e)?;
                    let w = gdp.get(e).copied().filter(|w| *w > 0.0).unwrap_or(1.0);
                    Some((g, capital_gap(pwt, e).unwrap_or(0.0), w))
                })
                .collect();
            if own.is_empty() {
                let mean = ps.first().and_then(|e| means.get(region.get(e)?));
                return Ok(match mean {
                    Some(&g_a) => Drift {
                        g_a,
                        f_gap: 0.0,
                        source: Source::Region,
                    },
                    None => Drift {
                        g_a: 0.0,
                        f_gap: 0.0,
                        source: Source::None,
                    },
                });
            }
            let total: f64 = own.iter().map(|p| p.2).sum();
            let s_l = labour_share(code)
                .filter(|s| *s > 0.0)
                .ok_or_else(|| format!("{code}: no labour share for the capital drift"))?;
            Ok(Drift {
                g_a: own.iter().map(|p| p.0 * p.2).sum::<f64>() / total,
                f_gap: own.iter().map(|p| p.1 * p.2).sum::<f64>() / total / s_l,
                source: Source::Pwt,
            })
        })
        .collect()
}

/// `g_a`, `f_gap` and `drift_source` of `econ_countries.arrow`.
pub fn columns(d: &[Drift]) -> Vec<(String, ArrayRef)> {
    vec![
        (
            "g_a".into(),
            Arc::new(Float64Array::from_iter_values(d.iter().map(|x| x.g_a))) as ArrayRef,
        ),
        (
            "f_gap".into(),
            Arc::new(Float64Array::from_iter_values(d.iter().map(|x| x.f_gap))),
        ),
        (
            "drift_source".into(),
            Arc::new(StringArray::from_iter_values(
                d.iter().map(|x| x.source.as_str()),
            )),
        ),
    ]
}

/// Build summary: sources, and the range of g_A and f.
pub fn report(d: &[Drift], codes: &[String]) -> String {
    let count = |s: Source| d.iter().filter(|x| x.source == s).count();
    let extreme = |key: fn(&Drift) -> f64| {
        let (lo, hi) = (
            d.iter()
                .enumerate()
                .min_by(|a, b| key(a.1).total_cmp(&key(b.1))),
            d.iter()
                .enumerate()
                .max_by(|a, b| key(a.1).total_cmp(&key(b.1))),
        );
        match (lo, hi) {
            (Some((i, a)), Some((j, b))) => format!(
                "{:.4} ({}) to {:.4} ({})",
                key(a),
                codes[i],
                key(b),
                codes[j]
            ),
            _ => "none".into(),
        }
    };
    format!(
        "initial drift (ADR 0103): {} pwt, {} region, {} none; g_A {}; f {}",
        count(Source::Pwt),
        count(Source::Region),
        count(Source::None),
        extreme(|x| x.g_a),
        extreme(|x| x.f_gap)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(f: impl Fn(i64) -> f64) -> ByYear {
        (FIRST..=LAST).map(|y| (y, f(y))).collect()
    }

    #[test]
    fn log_trend_recovers_an_exact_growth_rate() {
        let (b, se2) = log_trend(&series(|y| 2.0 * (0.03 * (y - FIRST) as f64).exp())).unwrap();
        assert!((b - 0.03).abs() < 1e-12, "{b}");
        assert!(se2 < 1e-20, "{se2}");
        let mut gap = series(|_| 1.0);
        gap.remove(&1997);
        assert_eq!(log_trend(&gap), None);
    }

    /// Without noise every economy keeps its own relative trend; with noise far above the
    /// spread every one takes its region's mean.
    #[test]
    fn shrinkage_runs_from_own_trend_to_regional_mean() {
        let region: BTreeMap<String, String> = [("A", "r"), ("B", "r"), ("C", "s"), ("D", "s")]
            .map(|(e, r)| (e.to_owned(), r.to_owned()))
            .into();
        let gdp: BTreeMap<String, f64> = ["A", "B", "C", "D"].map(|e| (e.to_owned(), 1.0)).into();
        let slopes = [0.01, 0.03, 0.0, 0.04];
        let at = |se2: f64| -> BTreeMap<String, (f64, f64)> {
            ["A", "B", "C", "D"]
                .iter()
                .zip(slopes)
                .map(|(e, b)| ((*e).to_owned(), (b, se2)))
                .collect()
        };
        let world = slopes.iter().sum::<f64>() / 4.0;
        let (exact, _) = shrunk_tfp(&at(0.0), &gdp, &region).unwrap();
        assert!((exact["A"] - (0.01 - world)).abs() < 1e-15);
        let (noisy, means) = shrunk_tfp(&at(1.0), &gdp, &region).unwrap();
        assert!((noisy["A"] - means["r"]).abs() < 1e-15);
        assert!((means["r"] - (0.02 - world)).abs() < 1e-15);
    }

    /// K/Y* = s / (n + g + delta): s 0.25, n 0.01, g 0.02, delta 0.05 give 3.125; at a 2000
    /// ratio of 2.5 the gap is ln 1.25.
    #[test]
    fn capital_gap_is_the_log_of_steady_over_actual() {
        let e = "A".to_owned();
        let pwt: BTreeMap<&str, BTreeMap<String, ByYear>> = [
            ("pop", series(|y| (0.01 * (y - FIRST) as f64).exp())),
            (
                "rgdpna",
                series(|y| 100.0 * (0.03 * (y - FIRST) as f64).exp()),
            ),
            ("csh_i", series(|_| 0.25)),
            ("delta", series(|_| 0.05)),
            (
                "rnna",
                series(|y| {
                    if y == LAST {
                        2.5 * 100.0 * (0.15f64).exp()
                    } else {
                        1.0
                    }
                }),
            ),
        ]
        .map(|(k, s)| (k, [(e.clone(), s)].into()))
        .into();
        let gap = capital_gap(&pwt, "A").unwrap();
        assert!((gap - 1.25f64.ln()).abs() < 1e-12, "{gap}");
    }
}
