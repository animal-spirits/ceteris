//! Sector labour shares and wealth shares of the pack countries (ADRs 0307, 0311): the
//! VA-weighted mean of a country's labour shares is its PWT share, a country outside SEA is
//! pooled with its flag set, and the wealth proxy holds the DFA's US top decile.

use std::collections::BTreeMap;

use ceteris_pack::econ::factors::{
    DFA_COUNTRY, DFA_TOP10, Factors, S_L_MAX, S_L_MIN, factors, tau2, wealth, wealth_proxy,
};
use ceteris_pack::read::icio::N_GOODS;
use ceteris_pack::read::sea::Sea;

/// Three SEA economies (A, B, C) and two outside SEA (D, E).
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
    let index = |c: &str| codes.iter().position(|x| x == c);
    let f = factors(
        &codes,
        &|c| index(c).map(|i| s_l[i]),
        &|c| index(c).map(|i| va[i]),
        &|c| vec![c.to_owned()],
        &income,
        &sea,
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
fn country_outside_sea_is_pooled_with_flag() {
    let (_, _, f) = build();
    assert_eq!(f.labour_pooled, [false, false, false, true, true]);
}

const US_GINI: f64 = 0.37;

#[test]
fn wealth_proxy_fits_the_us_top_decile() {
    let codes: Vec<String> = [DFA_COUNTRY, "X"].map(String::from).to_vec();
    let w = wealth(&codes, &|c| {
        Some(if c == DFA_COUNTRY { US_GINI } else { 0.5 })
    })
    .unwrap();
    assert!((w[0][2] - DFA_TOP10).abs() < 1e-12, "{}", w[0][2]);
}

#[test]
fn wealth_proxy_shares_sum_to_one() {
    let t = tau2(US_GINI).unwrap();
    let mut top = 0.0;
    for k in 1..=70 {
        let w = wealth_proxy(0.01 * f64::from(k), t);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(w.iter().all(|&x| x > 0.0 && x < 1.0), "{w:?}");
        assert!(w[2] > top, "Gini {k}%: top share {} not above {top}", w[2]);
        top = w[2];
    }
}
