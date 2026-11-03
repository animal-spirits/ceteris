//! Sector labour shares and wealth shares of the pack countries (ADR 0104, RFC 0302): the
//! VA-weighted mean of a country's labour shares is its PWT share, and a country missing a source
//! takes the documented fallback with its flag set.

use std::collections::BTreeMap;

use ceteris_pack::econ::factors::{Factors, S_L_MAX, S_L_MIN, factors};
use ceteris_pack::read::icio::N_GOODS;
use ceteris_pack::read::sea::Sea;

/// Three SEA economies (A, B, C), one outside SEA and WID (D, same region as A), one outside SEA
/// in WID (E, a region of its own).
fn build() -> (Vec<[f64; N_GOODS]>, Vec<f64>, Factors) {
    let codes: Vec<String> = ["A", "B", "C", "D", "E"].map(String::from).to_vec();
    let va: Vec<[f64; N_GOODS]> = (0..codes.len())
        .map(|c| std::array::from_fn(|g| 1.0 + ((c + 3) * (g + 1) % 7) as f64))
        .collect();
    let s_l = vec![0.55, 0.6, 0.45, 0.5, 0.65];
    let mut sea = Sea::new();
    for (k, code) in ["A", "B", "C"].iter().enumerate() {
        let v: [f64; N_GOODS] = std::array::from_fn(|g| 10.0 + g as f64);
        // mining and real estate capital-heavy, services labour-heavy
        let lab: [f64; N_GOODS] = std::array::from_fn(|g| {
            v[g] * match g {
                1 | 2 => 0.2,
                9 => 0.7 + 0.05 * k as f64,
                _ => 0.5,
            }
        });
        sea.insert((*code).into(), (lab, v));
    }
    let income: BTreeMap<String, f64> = [
        ("A", 30e3),
        ("B", 20e3),
        ("C", 2e3),
        ("D", 1.5e3),
        ("E", 8e3),
    ]
    .map(|(c, x)| (c.into(), x))
    .into();
    let population: BTreeMap<String, f64> = codes.iter().map(|c| (c.clone(), 1e6)).collect();
    let region: BTreeMap<String, String> = [
        ("A", "r1"),
        ("B", "r2"),
        ("C", "r1"),
        ("D", "r1"),
        ("E", "r3"),
    ]
    .map(|(c, r)| (c.into(), r.into()))
    .into();
    let wid: BTreeMap<String, [f64; 3]> = [
        ("A", [0.05, 0.35, 0.6]),
        ("B", [-0.01, 0.3, 0.71]),
        ("C", [0.03, 0.27, 0.7]),
        ("E", [0.04, 0.3, 0.66]),
    ]
    .map(|(c, w)| (c.into(), w))
    .into();
    let index = |c: &str| codes.iter().position(|x| x == c);
    let f = factors(
        &codes,
        &|c| index(c).map(|i| s_l[i]),
        &|c| index(c).map(|i| va[i]),
        &|c| vec![c.to_owned()],
        &income,
        &population,
        &region,
        &sea,
        &wid,
    )
    .unwrap();
    (va, s_l, f)
}

#[test]
fn labour_share_va_weighted_mean_equals_pwt() {
    let (va, s_l, f) = build();
    for (c, l) in f.labour.iter().enumerate() {
        let mean =
            l.share.iter().zip(&va[c]).map(|(s, v)| s * v).sum::<f64>() / va[c].iter().sum::<f64>();
        assert!(
            (mean - s_l[c]).abs() <= 1e-12,
            "country {c}: {mean} vs {}",
            s_l[c]
        );
        assert!(l.share.iter().all(|s| (S_L_MIN..=S_L_MAX).contains(s)));
        assert!(
            l.share[1] < l.share[9],
            "country {c}: mining below services"
        );
    }
}

#[test]
fn missing_source_falls_back_with_flag() {
    let (_, _, f) = build();
    assert_eq!(f.labour_pooled, [false, false, false, true, true]);
    // D, outside WID, takes its region's (A and C) population-weighted mean
    assert_eq!(f.wealth_source, ["wid", "wid", "wid", "region", "wid"]);
    for b in 0..3 {
        let want = (f.wealth[0][b] + f.wealth[2][b]) / 2.0;
        assert!((f.wealth[3][b] - want).abs() < 1e-15);
    }
    // B's negative bottom share is 0, the rest rescaled
    assert_eq!(f.wealth[1][0], 0.0);
    assert_eq!(f.wealth_clipped, [false, true, false, false, false]);
    for w in &f.wealth {
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-15 && w.iter().all(|&x| x >= 0.0));
    }
}
