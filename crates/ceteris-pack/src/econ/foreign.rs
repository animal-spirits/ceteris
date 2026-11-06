//! Foreign-owned shares and their owners of the pack countries (specs/econ/SPEC.md, Pack
//! additions; ADR 0309).
//!
//! f_c,g is the foreign-owned share of AAMNE 2000 value added of good g (goods 1 to 6) in the
//! country's economies, capped at [`F_MAX`]; the owners are the parents of the foreign-owned
//! gross output of those goods. A host outside AAMNE takes its ICIO donor's; AAMNE's rest of
//! the world as a parent (and a parent outside the pack) is spread over the pack countries
//! outside AAMNE by GDP 2000.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array, StringArray};

use crate::read::aamne::{GOODS, Owned, ROW};
use crate::read::icio::N_GOODS;

/// Cap on a foreign-owned share (ADR 0309).
pub const F_MAX: f64 = 0.8;

/// Per pack country: foreign-owned share by good, owners (summing to 1 where some share is
/// nonzero), source and whether the cap binds.
#[derive(Clone, Debug, Default)]
pub struct Foreign {
    pub share: Vec<[f64; N_GOODS]>,
    pub owners: Vec<Vec<f64>>,
    /// `aamne`, `donor` (its ICIO donor's) or `none`.
    pub source: Vec<&'static str>,
    pub capped: Vec<bool>,
}

/// Foreign-owned shares of `economies` pooled: foreign over all value added by good, capped.
fn shares(economies: &[&str], owned: &Owned) -> ([f64; N_GOODS], bool) {
    let (mut dom, mut fr) = ([0.0; N_GOODS], [0.0; N_GOODS]);
    for e in economies {
        let (d, f) = &owned[*e];
        for g in GOODS {
            dom[g] += d[g];
            fr[g] += f[g];
        }
    }
    let mut capped = false;
    let share = std::array::from_fn(|g| {
        let total = dom[g] + fr[g];
        if !GOODS.contains(&g) || total <= 0.0 || fr[g] <= 0.0 {
            return 0.0;
        }
        let s = fr[g] / total;
        capped |= s > F_MAX;
        s.min(F_MAX)
    });
    (share, capped)
}

/// The foreign shares and owners of every pack country (`codes`). `parts` gives a country's
/// economies (ISO3), `donor` its ICIO donor economy, `gdp` its GDP 2000 (USD); `owned` and
/// `parents` are AAMNE's value added by ownership and foreign-owned output by parent.
pub fn foreign(
    codes: &[String],
    parts: &dyn Fn(&str) -> Vec<String>,
    donor: &dyn Fn(&str) -> Option<String>,
    gdp: &[f64],
    owned: &Owned,
    parents: &BTreeMap<String, BTreeMap<String, f64>>,
) -> Result<Foreign, String> {
    let nc = codes.len();
    let hosted = |e: &str| e != ROW && owned.contains_key(e);
    let economies: Vec<Vec<String>> = codes.iter().map(|c| parts(c)).collect();
    let mut index: BTreeMap<&str, usize> = BTreeMap::new();
    for (c, ps) in economies.iter().enumerate() {
        for p in ps {
            index.insert(p, c);
        }
    }
    let outside: Vec<usize> = (0..nc)
        .filter(|&c| !economies[c].iter().any(|p| hosted(p)))
        .collect();
    let outside_gdp: f64 = outside.iter().map(|&c| gdp[c]).sum();
    if outside_gdp <= 0.0 {
        return Err("no GDP outside AAMNE to spread its rest of the world over".into());
    }
    let mut out = Foreign::default();
    for (c, code) in codes.iter().enumerate() {
        let own: Vec<&str> = economies[c]
            .iter()
            .map(String::as_str)
            .filter(|p| hosted(p))
            .collect();
        let (from, source) = if !own.is_empty() {
            (own, "aamne")
        } else {
            match donor(code) {
                Some(d) if hosted(&d) => {
                    (vec![owned.get_key_value(&d).unwrap().0.as_str()], "donor")
                }
                _ => (Vec::new(), "none"),
            }
        };
        let (mut share, capped) = shares(&from, owned);
        let mut owners = vec![0.0; nc];
        for e in &from {
            for (parent, &go) in parents.get(*e).into_iter().flatten() {
                match index
                    .get(parent.as_str())
                    .filter(|_| parent.as_str() != ROW)
                {
                    Some(&o) => owners[o] += go,
                    None => {
                        for &o in &outside {
                            owners[o] += go * gdp[o] / outside_gdp;
                        }
                    }
                }
            }
        }
        let total: f64 = owners.iter().sum();
        if total > 0.0 {
            owners.iter_mut().for_each(|o| *o /= total);
        } else {
            share = [0.0; N_GOODS];
        }
        out.share.push(share);
        out.owners.push(owners);
        out.source.push(if total > 0.0 { source } else { "none" });
        out.capped.push(capped && total > 0.0);
    }
    Ok(out)
}

/// `econ_countries.arrow` columns: `foreign_0` to `foreign_9`, `owner_0` to `owner_<n-1>` (by
/// country index), `foreign_source`, `foreign_capped`.
pub fn columns(f: &Foreign) -> Vec<(String, ArrayRef)> {
    let col = |v: Vec<f64>| -> ArrayRef { Arc::new(Float64Array::from(v)) };
    let mut out: Vec<(String, ArrayRef)> = (0..N_GOODS)
        .map(|g| {
            (
                format!("foreign_{g}"),
                col(f.share.iter().map(|s| s[g]).collect()),
            )
        })
        .collect();
    for o in 0..f.owners.len() {
        out.push((
            format!("owner_{o}"),
            col(f.owners.iter().map(|w| w[o]).collect()),
        ));
    }
    out.push((
        "foreign_source".into(),
        Arc::new(StringArray::from(f.source.clone())),
    ));
    out.push((
        "foreign_capped".into(),
        Arc::new(BooleanArray::from(f.capped.clone())),
    ));
    out
}

/// Countries by source, capped, and the largest shares of goods 1 to 6 with their hosts.
pub fn report(f: &Foreign, codes: &[String]) -> String {
    let count = |s: &str| f.source.iter().filter(|&&x| x == s).count();
    let mut top: Vec<(f64, &str, usize)> = f
        .share
        .iter()
        .zip(codes)
        .flat_map(|(s, c)| GOODS.map(move |g| (s[g], c.as_str(), g)))
        .collect();
    top.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(b.1)).then(a.2.cmp(&b.2)));
    let top: Vec<String> = top
        .iter()
        .take(5)
        .map(|(s, c, g)| format!("{c} good {g} {s:.3}"))
        .collect();
    format!(
        "foreign owners (ADR 0309): {} AAMNE, {} donor, {} none, {} capped at {F_MAX}; top {}",
        count("aamne"),
        count("donor"),
        count("none"),
        f.capped.iter().filter(|&&c| c).count(),
        top.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(rows: &[(&str, f64, f64)]) -> Owned {
        rows.iter()
            .map(|&(c, d, f)| {
                let mut a = ([0.0; N_GOODS], [0.0; N_GOODS]);
                for g in 0..N_GOODS {
                    a.0[g] = d;
                    a.1[g] = f;
                }
                (c.to_owned(), a)
            })
            .collect()
    }

    #[test]
    fn shares_owners_donors_and_the_rest_of_the_world() {
        let owned = owned(&[("AAA", 1.0, 1.0), ("BBB", 1.0, 9.0), ("ROW", 1.0, 1.0)]);
        let parents: BTreeMap<String, BTreeMap<String, f64>> = [
            ("AAA", vec![("BBB", 3.0), ("ROW", 1.0)]),
            ("BBB", vec![("AAA", 2.0)]),
        ]
        .into_iter()
        .map(|(h, ps)| {
            (
                h.to_owned(),
                ps.into_iter().map(|(p, x)| (p.to_owned(), x)).collect(),
            )
        })
        .collect();
        let codes: Vec<String> = ["AAA", "BBB", "CCC", "DDD"].map(String::from).to_vec();
        let donor = |c: &str| (c == "CCC").then(|| "AAA".to_owned());
        let f = foreign(
            &codes,
            &|c| vec![c.to_owned()],
            &donor,
            &[4.0, 4.0, 1.0, 3.0],
            &owned,
            &parents,
        )
        .unwrap();
        assert_eq!(f.source, ["aamne", "aamne", "donor", "none"]);
        assert_eq!(f.share[0][1], 0.5);
        assert_eq!(f.share[0][0], 0.0);
        assert_eq!(f.share[0][7], 0.0);
        assert_eq!(f.share[1][3], F_MAX);
        assert_eq!(f.capped, [false, true, false, false]);
        // AAA: 3 of 4 to BBB, the rest of the world's 1 to CCC and DDD by GDP 1 : 3
        assert_eq!(f.owners[0], [0.0, 0.75, 0.0625, 0.1875]);
        assert_eq!(f.owners[1], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(f.owners[2], f.owners[0]);
        assert_eq!(f.share[3], [0.0; N_GOODS]);
    }
}
