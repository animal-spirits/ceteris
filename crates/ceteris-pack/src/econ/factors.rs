//! Sector labour shares and wealth shares of the pack countries (specs/econ/SPEC.md, Pack
//! additions; ADRs 0307, 0308 and 0311).
//!
//! Labour shares: s_L,c,g = s_L,c r_g,c / (sum_h VA_c,h r_h,c / VA_c), r the WIOD SEA 2000
//! relative labour share of the good, so the VA-weighted mean is the PWT national share; a share
//! outside [`S_L_MIN`, `S_L_MAX`] is clipped and the others rescaled until the mean holds again.
//! Wealth: shares of the bottom 50%, middle 40% and top 10% of a lognormal whose log variance is
//! the income Gini's plus tau^2, tau fitted so the US top decile holds [`DFA_TOP10`].

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array};

use crate::read::icio::N_GOODS;

/// Bounds of a sector labour share (ADR 0307).
pub const S_L_MIN: f64 = 0.05;
pub const S_L_MAX: f64 = 0.95;
/// SEA economies nearest in log GDP per person whose mean relative shares a country outside SEA
/// takes (ADR 0307: the income group, which the pack has no table of).
pub const NEAREST: usize = 5;
/// P161: US top-10% share of net worth, Federal Reserve Distributional Financial Accounts 2000Q1
/// (ADR 0311).
pub const DFA_TOP10: f64 = 0.638;
/// The economy whose Gini fixes tau.
pub const DFA_COUNTRY: &str = "USA";
/// Population quantile at the top decile's lower bound.
const TOP_DECILE: f64 = 0.9;

/// Labour shares of one country's goods and whether a bound binds.
#[derive(Clone, Debug, PartialEq)]
pub struct LabourShares {
    pub share: [f64; N_GOODS],
    pub clipped: bool,
}

/// Rescales relative shares `r` to the national share `s_l` at value added `va` by good, clipping
/// at the bounds and rescaling the unclipped goods until the VA-weighted mean is `s_l`. A good
/// without value added takes its rescaled share, which no mean weighs.
pub fn labour_shares(
    s_l: f64,
    va: &[f64; N_GOODS],
    r: &[f64; N_GOODS],
) -> Result<LabourShares, String> {
    let total: f64 = va.iter().sum();
    if total <= 0.0 {
        return Err("no value added".into());
    }
    let mut fixed: [Option<f64>; N_GOODS] = [None; N_GOODS];
    // each round fixes at least one more good, so N_GOODS rounds settle it
    for _ in 0..=N_GOODS {
        let held: f64 = (0..N_GOODS)
            .filter_map(|g| fixed[g].map(|s| s * va[g]))
            .sum();
        let free: f64 = (0..N_GOODS)
            .filter(|&g| fixed[g].is_none())
            .map(|g| r[g] * va[g])
            .sum();
        if free <= 0.0 {
            return Err(format!(
                "labour share {s_l} cannot be met within the bounds"
            ));
        }
        let k = (s_l * total - held) / free;
        let mut share = [0.0; N_GOODS];
        let mut moved = false;
        for g in 0..N_GOODS {
            share[g] = match fixed[g] {
                Some(s) => s,
                None => {
                    let s = k * r[g];
                    if s < S_L_MIN || s > S_L_MAX {
                        fixed[g] = Some(s.clamp(S_L_MIN, S_L_MAX));
                        moved = true;
                    }
                    s
                }
            };
        }
        if !moved {
            return Ok(LabourShares {
                share,
                clipped: fixed.iter().any(Option::is_some),
            });
        }
    }
    Err(format!("labour share {s_l}: clipping did not settle"))
}

/// Relative labour share of each good, (LAB_g / VA_g) / (LAB / VA), from labour compensation and
/// value added by good; None for a good without positive value added.
pub fn relative_shares(
    lab: &[f64; N_GOODS],
    va: &[f64; N_GOODS],
) -> Option<[Option<f64>; N_GOODS]> {
    let (l, v): (f64, f64) = (lab.iter().sum(), va.iter().sum());
    if l <= 0.0 || v <= 0.0 {
        return None;
    }
    let national = l / v;
    Some(std::array::from_fn(|g| {
        (va[g] > 0.0 && lab[g] >= 0.0).then(|| lab[g] / va[g] / national)
    }))
}

/// Relative shares of every good for a country: its own (`sea`) where SEA has the good, else
/// the mean of the [`NEAREST`] SEA economies nearest in log GDP per person that have it (all
/// that have it where `income` is unknown), else 1. The flag says whether any good is pooled.
pub fn pooled(
    own: Option<&[Option<f64>; N_GOODS]>,
    income: Option<f64>,
    sea: &BTreeMap<String, [Option<f64>; N_GOODS]>,
    sea_income: &BTreeMap<String, f64>,
) -> ([f64; N_GOODS], bool) {
    let mut pooled_any = false;
    let r = std::array::from_fn(|g| {
        if let Some(x) = own.and_then(|o| o[g]) {
            return x;
        }
        pooled_any = true;
        let mut near: Vec<(f64, f64)> = sea
            .iter()
            .filter_map(|(code, rs)| {
                let x = rs[g]?;
                let d = match (income, sea_income.get(code)) {
                    (Some(a), Some(&b)) if a > 0.0 && b > 0.0 => (a.ln() - b.ln()).abs(),
                    (Some(_), _) => return None,
                    (None, _) => 0.0,
                };
                Some((d, x))
            })
            .collect();
        // distance, then the map's code order, which the stable sort keeps
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        near.truncate(NEAREST);
        if near.is_empty() {
            1.0
        } else {
            near.iter().map(|&(_, x)| x).sum::<f64>() / near.len() as f64
        }
    });
    (r, pooled_any)
}

fn phi(z: f64) -> f64 {
    0.5 * libm::erfc(-z / std::f64::consts::SQRT_2)
}

/// Inverse of [`phi`] by bisection to the nearest double, for p in (0, 1).
fn phi_inv(p: f64) -> f64 {
    let (mut lo, mut hi) = (-40.0, 40.0);
    loop {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            return mid;
        }
        if phi(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
}

/// Log dispersion of a lognormal with Gini `g`.
fn sigma_of_gini(g: f64) -> f64 {
    std::f64::consts::SQRT_2 * phi_inv((g + 1.0) / 2.0)
}

/// tau^2, the log-wealth variance above the income Gini's, from the US Gini 2000: the US
/// lognormal wealth distribution has top-decile share [`DFA_TOP10`].
pub fn tau2(us_gini: f64) -> Result<f64, String> {
    let sigma_w = phi_inv(TOP_DECILE) - phi_inv(1.0 - DFA_TOP10);
    let t = sigma_w.powi(2) - sigma_of_gini(us_gini).powi(2);
    if t < 0.0 {
        return Err(format!(
            "US Gini {us_gini} implies more dispersion than the DFA's wealth"
        ));
    }
    Ok(t)
}

/// Wealth shares of the bottom 50%, middle 40% and top 10% at Gini 2000 `g` (ADR 0311).
pub fn wealth_proxy(g: f64, tau2: f64) -> [f64; 3] {
    let sigma_w = (sigma_of_gini(g).powi(2) + tau2).sqrt();
    let bottom = phi(-sigma_w);
    let top = 1.0 - phi(phi_inv(TOP_DECILE) - sigma_w);
    [bottom, 1.0 - bottom - top, top]
}

/// Wealth shares of every pack country from its Gini 2000, tau fitted at [`DFA_COUNTRY`]'s.
pub fn wealth(
    codes: &[String],
    gini: &dyn Fn(&str) -> Option<f64>,
) -> Result<Vec<[f64; 3]>, String> {
    let t = tau2(gini(DFA_COUNTRY).ok_or("wealth proxy: no US Gini")?)?;
    codes
        .iter()
        .map(|c| {
            gini(c)
                .map(|g| wealth_proxy(g, t))
                .ok_or_else(|| format!("{c}: no Gini"))
        })
        .collect()
}

/// `econ_countries.arrow` columns `wealth_0` to `wealth_2` (bottom 50%, middle 40%, top 10%).
pub fn wealth_columns(wealth: &[[f64; 3]]) -> Vec<(String, ArrayRef)> {
    (0..3)
        .map(|b| {
            let a: ArrayRef = Arc::new(Float64Array::from(
                wealth.iter().map(|w| w[b]).collect::<Vec<_>>(),
            ));
            (format!("wealth_{b}"), a)
        })
        .collect()
}

/// Per pack country: labour shares and flags.
#[derive(Clone, Debug, Default)]
pub struct Factors {
    pub labour: Vec<LabourShares>,
    /// Some good's relative share is pooled from other SEA economies.
    pub labour_pooled: Vec<bool>,
}

/// `econ_countries.arrow` columns: `labour_share_0` to `labour_share_9`, `labour_share_pooled`,
/// `labour_share_clipped`.
pub fn columns(f: &Factors) -> Vec<(String, ArrayRef)> {
    let mut out: Vec<(String, ArrayRef)> = (0..N_GOODS)
        .map(|g| {
            let a: ArrayRef = Arc::new(Float64Array::from(
                f.labour.iter().map(|l| l.share[g]).collect::<Vec<_>>(),
            ));
            (format!("labour_share_{g}"), a)
        })
        .collect();
    let flag = |v: Vec<bool>| -> ArrayRef { Arc::new(BooleanArray::from(v)) };
    out.push(("labour_share_pooled".into(), flag(f.labour_pooled.clone())));
    out.push((
        "labour_share_clipped".into(),
        flag(f.labour.iter().map(|l| l.clipped).collect()),
    ));
    out
}

/// The labour share columns of every pack country (ADR 0307). `s_l` and `va` are each country's
/// PWT labour share and national VA by good, `parts` its economies (ISO3), `income` per economy,
/// `sea` SEA's LAB and VA by economy.
pub fn factors(
    codes: &[String],
    s_l: &dyn Fn(&str) -> Option<f64>,
    va: &dyn Fn(&str) -> Option<[f64; N_GOODS]>,
    parts: &dyn Fn(&str) -> Vec<String>,
    income: &BTreeMap<String, f64>,
    sea: &crate::read::sea::Sea,
) -> Result<Factors, String> {
    let sea_r: BTreeMap<String, [Option<f64>; N_GOODS]> = sea
        .iter()
        .filter_map(|(c, (lab, va))| Some((c.clone(), relative_shares(lab, va)?)))
        .collect();
    let mut out = Factors::default();
    for code in codes {
        let ps = parts(code);
        let s = s_l(code).ok_or_else(|| format!("{code}: no labour share"))?;
        let v = va(code).ok_or_else(|| format!("{code}: no value added"))?;
        let (mut lab_sum, mut va_sum, mut any) = ([0.0; N_GOODS], [0.0; N_GOODS], false);
        for p in &ps {
            if let Some((l, x)) = sea.get(p) {
                for g in 0..N_GOODS {
                    lab_sum[g] += l[g];
                    va_sum[g] += x[g];
                }
                any = true;
            }
        }
        let own = any.then(|| relative_shares(&lab_sum, &va_sum)).flatten();
        let inc = ps.first().and_then(|p| income.get(p)).copied();
        let (r, pooled_any) = pooled(own.as_ref(), inc, &sea_r, income);
        out.labour
            .push(labour_shares(s, &v, &r).map_err(|e| format!("{code}: {e}"))?);
        out.labour_pooled.push(pooled_any);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean(s: &[f64; N_GOODS], va: &[f64; N_GOODS]) -> f64 {
        s.iter().zip(va).map(|(s, v)| s * v).sum::<f64>() / va.iter().sum::<f64>()
    }

    #[test]
    fn unclipped_shares_scale_the_relative_shares() {
        let va = [1.0, 2.0, 0.5, 3.0, 1.0, 1.0, 2.0, 0.7, 1.5, 10.0];
        let r = [0.8, 0.3, 0.5, 1.0, 1.1, 0.9, 1.0, 0.6, 1.2, 1.05];
        let l = labour_shares(0.55, &va, &r).unwrap();
        assert!(!l.clipped);
        assert!((mean(&l.share, &va) - 0.55).abs() < 1e-15);
        let k = l.share[0] / r[0];
        assert!(
            l.share
                .iter()
                .zip(&r)
                .all(|(s, r)| (s / r - k).abs() < 1e-15)
        );
    }

    #[test]
    fn clipped_shares_keep_the_national_mean() {
        let va = [1.0, 5.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let r = [2.0, 0.01, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let l = labour_shares(0.5, &va, &r).unwrap();
        assert!(l.clipped);
        assert_eq!(l.share[1], S_L_MIN);
        assert!(l.share.iter().all(|&s| (S_L_MIN..=S_L_MAX).contains(&s)));
        assert!((mean(&l.share, &va) - 0.5).abs() < 1e-15);
    }
}
