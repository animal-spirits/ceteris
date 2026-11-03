//! Sector labour shares and wealth shares of the pack countries (specs/econ/SPEC.md, Pack
//! additions; ADRs 0307 and 0308).
//!
//! Labour shares: s_L,c,g = s_L,c r_g,c / (sum_h VA_c,h r_h,c / VA_c), r the WIOD SEA 2000
//! relative labour share of the good, so the VA-weighted mean is the PWT national share; a share
//! outside [`S_L_MIN`, `S_L_MAX`] is clipped and the others rescaled until the mean holds again.
//! Wealth: WID 2000 shares of the bottom 50%, middle 40% and top 10%, a negative share at 0.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, StringArray};

use crate::read::icio::N_GOODS;

/// Bounds of a sector labour share (ADR 0307).
pub const S_L_MIN: f64 = 0.05;
pub const S_L_MAX: f64 = 0.95;
/// SEA economies nearest in log GDP per person whose mean relative shares a country outside SEA
/// takes (ADR 0307: the income group, which the pack has no table of).
pub const NEAREST: usize = 5;

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

/// Wealth shares of the bottom 50%, middle 40% and top 10% from WID's p0p50, p50p90 and p90p100
/// (each may be negative or not sum to 1 exactly): a negative share is set to 0 and the three
/// rescaled to sum to 1. The flag says whether a share was negative.
pub fn wealth_brackets(raw: [f64; 3]) -> Result<([f64; 3], bool), String> {
    let clipped = raw.iter().any(|&x| x < 0.0);
    let pos = raw.map(|x| x.max(0.0));
    let total: f64 = pos.iter().sum();
    if total <= 0.0 {
        return Err(format!("wealth shares {raw:?}"));
    }
    Ok((pos.map(|x| x / total), clipped))
}

/// Population-weighted mean wealth shares of `members` that have their own.
pub fn mean_wealth(members: &[(&[f64; 3], f64)]) -> Option<[f64; 3]> {
    let n: f64 = members.iter().map(|m| m.1).sum();
    (n > 0.0)
        .then(|| std::array::from_fn(|b| members.iter().map(|(w, p)| w[b] * p).sum::<f64>() / n))
}

/// Per pack country: labour shares and flags, wealth shares and their source.
#[derive(Clone, Debug, Default)]
pub struct Factors {
    pub labour: Vec<LabourShares>,
    /// Some good's relative share is pooled from other SEA economies.
    pub labour_pooled: Vec<bool>,
    pub wealth: Vec<[f64; 3]>,
    /// `wid`, `region` (its region's WID countries) or `world` (every WID country).
    pub wealth_source: Vec<&'static str>,
    /// A WID share was negative and set to 0.
    pub wealth_clipped: Vec<bool>,
}

/// `econ_countries.arrow` columns: `labour_share_0` to `labour_share_9`, `labour_share_pooled`,
/// `labour_share_clipped`, `wealth_0` to `wealth_2` (bottom 50%, middle 40%, top 10%),
/// `wealth_source`, `wealth_clipped`.
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
    for b in 0..3 {
        out.push((
            format!("wealth_{b}"),
            Arc::new(Float64Array::from(
                f.wealth.iter().map(|w| w[b]).collect::<Vec<_>>(),
            )),
        ));
    }
    out.push((
        "wealth_source".into(),
        Arc::new(StringArray::from(f.wealth_source.clone())),
    ));
    out.push(("wealth_clipped".into(), flag(f.wealth_clipped.clone())));
    out
}

/// The factor columns of every pack country (ADRs 0307, 0308). `s_l` and `va` are each country's
/// PWT labour share and national VA by good, `parts` its economies (ISO3), `income` and
/// `population` per economy, `region` the World Bank region per economy; `sea` SEA's LAB and VA
/// by economy, `wid` WID's three shares by economy (ISO3).
#[allow(clippy::too_many_arguments)]
pub fn factors(
    codes: &[String],
    s_l: &dyn Fn(&str) -> Option<f64>,
    va: &dyn Fn(&str) -> Option<[f64; N_GOODS]>,
    parts: &dyn Fn(&str) -> Vec<String>,
    income: &BTreeMap<String, f64>,
    population: &BTreeMap<String, f64>,
    region: &BTreeMap<String, String>,
    sea: &crate::read::sea::Sea,
    wid: &BTreeMap<String, [f64; 3]>,
) -> Result<Factors, String> {
    let sea_r: BTreeMap<String, [Option<f64>; N_GOODS]> = sea
        .iter()
        .filter_map(|(c, (lab, va))| Some((c.clone(), relative_shares(lab, va)?)))
        .collect();
    let mut wid_clean: BTreeMap<String, ([f64; 3], bool)> = BTreeMap::new();
    for (c, raw) in wid {
        wid_clean.insert(
            c.clone(),
            wealth_brackets(*raw).map_err(|e| format!("WID {c}: {e}"))?,
        );
    }
    let pool = |pick: &dyn Fn(&str) -> bool| -> Option<([f64; 3], bool)> {
        let members: Vec<(&[f64; 3], f64)> = wid_clean
            .iter()
            .filter(|(c, _)| pick(c))
            .filter_map(|(c, (w, _))| Some((w, *population.get(c)?)))
            .collect();
        let clipped = wid_clean.iter().any(|(c, (_, k))| pick(c) && *k);
        mean_wealth(&members).map(|w| (w, clipped))
    };
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

        let own_w = pool(&|c| ps.iter().any(|p| p == c));
        let reg = ps.first().and_then(|p| region.get(p));
        let (w, source) = match own_w {
            Some(w) => (w, "wid"),
            None => match reg.and_then(|r| pool(&|c| region.get(c) == Some(r))) {
                Some(w) => (w, "region"),
                None => (pool(&|_| true).ok_or("WID has no countries")?, "world"),
            },
        };
        out.wealth.push(w.0);
        out.wealth_clipped.push(w.1);
        out.wealth_source.push(source);
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

    #[test]
    fn negative_wealth_share_is_zero_and_the_rest_rescaled() {
        let (w, clipped) = wealth_brackets([-0.02, 0.32, 0.72]).unwrap();
        assert!(clipped);
        assert_eq!(w[0], 0.0);
        assert!((w[1] + w[2] - 1.0).abs() < 1e-15);
        assert!((w[1] / w[2] - 0.32 / 0.72).abs() < 1e-15);
    }
}
