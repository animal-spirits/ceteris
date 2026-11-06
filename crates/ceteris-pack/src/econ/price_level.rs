//! Price level p_c of each pack country, the base price of its local goods (specs/econ/SPEC.md,
//! Price levels; ADR 0098 as amended by RFC 0282, D1, D7 and D9).

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, StringArray};

use crate::numeric::pairwise_sum;
use crate::read::{ByYear, WINDOW, YEAR};

/// WDI GDP at market rates, GDP per person at PPP and population, whose ratio is the WDI price
/// level (`wdi_implied`).
pub const WDI_PRICE_LEVEL: [&str; 3] = [
    "wdi_NY.GDP.MKTP.CD.zip",
    "wdi_NY.GDP.PCAP.PP.CD.zip",
    "wdi_SP.POP.TOTL.zip",
];
/// The economy every level is relative to.
pub const BASE: &str = "USA";
/// Tukey's (1977) far-out fences: a value more than this many interquartile ranges beyond the
/// quartiles of its reference set is a data error, not a price level.
pub const FAR_OUT: f64 = 3.0;

/// Where a country's price level comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// PWT 11 pl_da over the US's.
    Pwt,
    /// The WDI level (`wdi_implied`) over the US's of the same year.
    Wdi,
    /// The imputation donor's level.
    Donor,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Pwt => "pwt",
            Source::Wdi => "wdi",
            Source::Donor => "donor",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub value: f64,
    pub source: Source,
}

/// Each economy's value of the year nearest 2000 within `WINDOW` over the US's of that year.
pub fn relative(
    series: &BTreeMap<String, ByYear>,
    name: &str,
) -> Result<BTreeMap<String, f64>, String> {
    let base = series
        .get(BASE)
        .ok_or_else(|| format!("{name} has no price level for the US"))?;
    Ok(series
        .iter()
        .filter_map(|(k, by_year)| {
            let (year, v) = by_year
                .iter()
                .filter(|(y, _)| (**y - YEAR).abs() <= WINDOW)
                .min_by_key(|(y, _)| ((**y - YEAR).abs(), **y))?;
            Some((k.clone(), v / base.get(year).filter(|b| **b > 0.0)?))
        })
        .collect())
}

/// The price level WDI implies, by economy and year: GDP at market rates over GDP at PPP, the
/// latter GDP per person at PPP times population, in every year all three report.
pub fn wdi_implied(
    gdp: &BTreeMap<String, ByYear>,
    ppp_per_person: &BTreeMap<String, ByYear>,
    population: &BTreeMap<String, ByYear>,
) -> BTreeMap<String, ByYear> {
    gdp.iter()
        .filter_map(|(k, by_year)| {
            let (ppp, pop) = (ppp_per_person.get(k)?, population.get(k)?);
            let level: ByYear = by_year
                .iter()
                .filter_map(|(y, g)| Some((*y, g / (ppp.get(y)? * pop.get(y)?))))
                .collect();
            (!level.is_empty()).then(|| (k.clone(), level))
        })
        .collect()
}

/// Linear interpolation between order statistics (numpy's default) of sorted `x`.
fn quantile(x: &[f64], p: f64) -> f64 {
    let at = p * (x.len() - 1) as f64;
    let lo = at.floor() as usize;
    let hi = (lo + 1).min(x.len() - 1);
    x[lo] + (x[hi] - x[lo]) * (at - lo as f64)
}

/// `FAR_OUT` fences of `x`: Q1 - `FAR_OUT` IQR and Q3 + `FAR_OUT` IQR.
fn far_out(mut x: Vec<f64>) -> Result<(f64, f64), String> {
    if x.is_empty() || x.iter().any(|v| !v.is_finite()) {
        return Err("fences need finite values".into());
    }
    x.sort_by(f64::total_cmp);
    let (q1, q3) = (quantile(&x, 0.25), quantile(&x, 0.75));
    let iqr = q3 - q1;
    Ok((q1 - FAR_OUT * iqr, q3 + FAR_OUT * iqr))
}

/// The plausible range of a price level: the `FAR_OUT` fences of the logs of `levels`, as levels.
pub fn fences(levels: impl IntoIterator<Item = f64>) -> Result<(f64, f64), String> {
    let (lo, hi) = far_out(levels.into_iter().map(f64::ln).collect())?;
    Ok((lo.exp(), hi.exp()))
}

/// The outlier screen of the price levels: the Penn effect (price levels rise with income) fitted
/// on the PWT levels, and the fences a level must meet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    /// Fences of the PWT levels (`fences`).
    pub levels: (f64, f64),
    /// OLS of ln p on ln income over the PWT economies with an income and a level within
    /// `levels`.
    pub intercept: f64,
    pub slope: f64,
    /// Number of economies in the fit.
    pub fitted: usize,
    /// `FAR_OUT` fences of the fit's residuals.
    pub residuals: (f64, f64),
}

impl Screen {
    /// The screen of `pwt` (levels relative to the US) against `income` (GDP per person).
    pub fn fit(
        pwt: &BTreeMap<String, f64>,
        income: &BTreeMap<String, f64>,
    ) -> Result<Self, String> {
        let levels = fences(pwt.values().copied())?;
        let (x, y): (Vec<f64>, Vec<f64>) = pwt
            .iter()
            .filter(|(_, p)| (levels.0..=levels.1).contains(*p))
            .filter_map(|(c, p)| Some((income.get(c)?.ln(), p.ln())))
            .unzip();
        let n = x.len() as f64;
        let (mx, my) = (pairwise_sum(&x) / n, pairwise_sum(&y) / n);
        let dx: Vec<f64> = x.iter().map(|v| v - mx).collect();
        let sxx = pairwise_sum(&dx.iter().map(|d| d * d).collect::<Vec<_>>());
        let sxy = pairwise_sum(
            &dx.iter()
                .zip(&y)
                .map(|(d, v)| d * (v - my))
                .collect::<Vec<_>>(),
        );
        let slope = sxy / sxx;
        if !slope.is_finite() {
            return Err(format!(
                "price level screen: no income spread over {n} economies"
            ));
        }
        let intercept = my - slope * mx;
        let residuals = far_out(
            x.iter()
                .zip(&y)
                .map(|(a, b)| b - intercept - slope * a)
                .collect(),
        )?;
        Ok(Screen {
            levels,
            intercept,
            slope,
            fitted: x.len(),
            residuals,
        })
    }

    /// Whether `level` passes: its residual from the fit lies within the residual fences, or,
    /// with no income, the level within the level fences.
    pub fn keeps(&self, level: f64, income: Option<f64>) -> bool {
        match income {
            Some(y) => {
                let r = level.ln() - self.intercept - self.slope * y.ln();
                (self.residuals.0..=self.residuals.1).contains(&r)
            }
            None => (self.levels.0..=self.levels.1).contains(&level),
        }
    }
}

fn mean_of(parts: &[String], series: &BTreeMap<String, f64>) -> Option<f64> {
    let found: Vec<f64> = parts
        .iter()
        .filter_map(|p| series.get(p).copied())
        .collect();
    (!found.is_empty()).then(|| pairwise_sum(&found) / found.len() as f64)
}

/// The price level of each of `codes`: PWT (`pwt`, relative to the US) over the country's
/// economies (`parts`), else WDI (`wdi`, relative), each kept only where it passes `screen` at
/// the country's `income`; else the level its donor (`donor`) has by the same two rules. A
/// country of several economies takes the mean of their levels and of their incomes.
pub fn price_levels(
    codes: &[String],
    parts: &dyn Fn(&str) -> Vec<String>,
    donor: &dyn Fn(&str) -> Option<String>,
    pwt: &BTreeMap<String, f64>,
    wdi: &BTreeMap<String, f64>,
    income: &BTreeMap<String, f64>,
    screen: &Screen,
) -> Result<Vec<Level>, String> {
    let own = |ps: &[String]| {
        let y = mean_of(ps, income);
        [(pwt, Source::Pwt), (wdi, Source::Wdi)]
            .into_iter()
            .find_map(|(series, source)| {
                mean_of(ps, series)
                    .filter(|&value| screen.keeps(value, y))
                    .map(|value| Level { value, source })
            })
    };
    codes
        .iter()
        .map(|c| {
            if let Some(l) = own(&parts(c)) {
                return Ok(l);
            }
            let d = donor(c).ok_or_else(|| format!("{c} has no price level and no donor"))?;
            let l = own(std::slice::from_ref(&d))
                .ok_or_else(|| format!("{c}: donor {d} has no price level"))?;
            Ok(Level {
                value: l.value,
                source: Source::Donor,
            })
        })
        .collect()
}

/// `econ_countries.arrow` price level columns: `price_level` p_c, `price_level_source`, and
/// `price_level_imputed` where it is not PWT's.
pub fn columns(levels: &[Level]) -> Vec<(String, ArrayRef)> {
    vec![
        (
            "price_level".into(),
            Arc::new(levels.iter().map(|l| l.value).collect::<Float64Array>()) as ArrayRef,
        ),
        (
            "price_level_imputed".into(),
            Arc::new(
                levels
                    .iter()
                    .map(|l| Some(l.source != Source::Pwt))
                    .collect::<BooleanArray>(),
            ),
        ),
        (
            "price_level_source".into(),
            Arc::new(StringArray::from_iter_values(
                levels.iter().map(|l| l.source.as_str()),
            )),
        ),
    ]
}

/// One line of the build summary: levels by source and the screen.
pub fn report(levels: &[Level], screen: &Screen) -> String {
    let count = |s: Source| levels.iter().filter(|l| l.source == s).count();
    format!(
        "price levels: pwt {}, wdi {}, donor {}; level fences {:.4} to {:.4} times the US; \
         ln p = {:.4} + {:.4} ln y over {} economies, residual fences {:.4} to {:.4}",
        count(Source::Pwt),
        count(Source::Wdi),
        count(Source::Donor),
        screen.levels.0,
        screen.levels.1,
        screen.intercept,
        screen.slope,
        screen.fitted,
        screen.residuals.0,
        screen.residuals.1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_interpolates_between_order_statistics() {
        let x = [1.0, 2.0, 4.0, 8.0];
        assert_eq!(quantile(&x, 0.0), 1.0);
        assert_eq!(quantile(&x, 0.25), 1.75);
        assert_eq!(quantile(&x, 0.75), 5.0);
        assert_eq!(quantile(&x, 1.0), 8.0);
        assert_eq!(quantile(&[3.0], 0.25), 3.0);
    }
}
