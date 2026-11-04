//! Transfer ratios of the trade imbalances (specs/econ/SPEC.md, Transfers; ADR 0100 as amended
//! by ADRs 0296, 0298, 0300 and 0315): minus each pack country's current account over its own
//! GDP; the model scales it by national income and spreads the world discrepancy by income.

use std::collections::BTreeMap;
use std::sync::Arc;

use arrow_array::{ArrayRef, BooleanArray, Float64Array};

use crate::numeric::pairwise_sum;
use crate::read::ByYear;
use crate::read::people::parts;
use crate::read::wdi::Names;

/// WDI current account balance, BoP, current USD; GDP of every year is
/// `price_level::WDI_PRICE_LEVEL`'s NY.GDP.MKTP.CD.
pub const CURRENT_ACCOUNT: &str = "wdi_BN.CAB.XOKA.CD.zip";
/// The observed path, 2000 to 2024 (ADR 0100); the model's `inputs::TRANSFER_YEARS`.
pub const FIRST: i64 = 2000;
pub const LAST: i64 = 2024;

/// The 2000 current accounts and the ratios of every year.
#[derive(Clone, Debug, PartialEq)]
pub struct Transfers {
    /// USD, 0 where none of the country's economies reports one.
    pub ca_2000: Vec<f64>,
    pub missing: Vec<bool>,
    /// The fixed-share mode's rho_c: the 2000 current account over the pack's `gdp` (ADRs
    /// 0298, 0315).
    pub share: Vec<f64>,
    /// rho_c of each year FIRST to LAST, over the country's GDP that year (ADRs 0300, 0315);
    /// `path[year - FIRST][country]`.
    pub path: Vec<Vec<f64>>,
}

/// The sum over the country's WDI economies of their values in `year`; None where none has one.
pub fn of_country(
    series: &BTreeMap<String, ByYear>,
    code: &str,
    iso: &Names,
    year: i64,
) -> Option<f64> {
    let found: Vec<f64> = parts(code, iso)
        .iter()
        .filter_map(|p| series.get(p)?.get(&year).copied())
        .collect();
    (!found.is_empty()).then(|| pairwise_sum(&found))
}

/// rho_c = -CA_c / GDP_c (ADR 0315): 0 where the current account is missing (ADR 0296) or the
/// GDP is missing or not positive. The model spreads the world discrepancy.
pub fn shares(ca: &[Option<f64>], gdp: &[Option<f64>]) -> Vec<f64> {
    ca.iter()
        .zip(gdp)
        .map(|(c, g)| match (c, g) {
            (Some(c), Some(g)) if *g > 0.0 => -c / g,
            _ => 0.0,
        })
        .collect()
}

/// The transfers of the pack countries `codes`, at their 2000 GDP `gdp` (the `gdp` column).
pub fn transfers(
    codes: &[String],
    iso: &Names,
    gdp: &[f64],
    current_account: &BTreeMap<String, ByYear>,
    gdp_by_year: &BTreeMap<String, ByYear>,
) -> Transfers {
    let year = |series: &BTreeMap<String, ByYear>, y: i64| -> Vec<Option<f64>> {
        codes
            .iter()
            .map(|c| of_country(series, c, iso, y))
            .collect()
    };
    let ca = year(current_account, FIRST);
    let pack_gdp: Vec<Option<f64>> = gdp.iter().map(|&g| Some(g)).collect();
    Transfers {
        ca_2000: ca.iter().map(|x| x.unwrap_or(0.0)).collect(),
        missing: ca.iter().map(Option::is_none).collect(),
        share: shares(&ca, &pack_gdp),
        path: (FIRST..=LAST)
            .map(|y| shares(&year(current_account, y), &year(gdp_by_year, y)))
            .collect(),
    }
}

/// `econ_countries.arrow` columns: `ca_2000`, `ca_missing`, `transfer` and `transfer_2000` to
/// `transfer_2024`.
pub fn columns(t: &Transfers) -> Vec<(String, ArrayRef)> {
    let floats = |v: &[f64]| Arc::new(v.iter().copied().collect::<Float64Array>()) as ArrayRef;
    let mut out = vec![
        ("ca_2000".into(), floats(&t.ca_2000)),
        (
            "ca_missing".into(),
            Arc::new(t.missing.iter().map(|&m| Some(m)).collect::<BooleanArray>()) as ArrayRef,
        ),
        ("transfer".into(), floats(&t.share)),
    ];
    for (y, shares) in (FIRST..=LAST).zip(&t.path) {
        out.push((format!("transfer_{y}"), floats(shares)));
    }
    out
}

/// One line of the build summary: coverage of the 2000 current accounts and the largest shares.
pub fn report(t: &Transfers, codes: &[String], gdp: &[f64]) -> String {
    let world = pairwise_sum(gdp);
    let missing: Vec<f64> = (0..codes.len())
        .filter(|&c| t.missing[c])
        .map(|c| gdp[c])
        .collect();
    let mut order: Vec<usize> = (0..codes.len()).collect();
    order.sort_by(|&a, &b| t.share[b].total_cmp(&t.share[a]).then(a.cmp(&b)));
    let name = |c: usize| format!("{} {:+.5}", codes[c], t.share[c]);
    format!(
        "transfers: 2000 current accounts for {} of {} countries (missing {:.4} of world GDP), \
         their sum {:+.5} of world GDP; largest deficits over GDP {}; largest surpluses {}",
        codes.len() - missing.len(),
        codes.len(),
        pairwise_sum(&missing) / world,
        pairwise_sum(&t.ca_2000) / world,
        order
            .iter()
            .take(3)
            .map(|&c| name(c))
            .collect::<Vec<_>>()
            .join(", "),
        order
            .iter()
            .rev()
            .take(3)
            .map(|&c| name(c))
            .collect::<Vec<_>>()
            .join(", "),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR 0315: rho_c = -CA_c / GDP_c, 0 where the current account (ADR 0296) or the GDP is
    /// missing.
    #[test]
    fn transfer_ratios_are_minus_the_current_account_over_gdp() {
        let ca = [Some(-40.0), Some(10.0), None, Some(20.0), Some(5.0)];
        let gdp = [Some(500.0), Some(300.0), Some(200.0), None, Some(0.0)];
        assert_eq!(shares(&ca, &gdp), [0.08, -10.0 / 300.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_country_of_several_economies_sums_them() {
        let mut series = BTreeMap::new();
        series.insert("USA".to_string(), ByYear::from([(2000, 1.0), (2001, 2.0)]));
        let iso = Names::new();
        assert_eq!(of_country(&series, "USA", &iso, 2000), Some(1.0));
        assert_eq!(of_country(&series, "USA", &iso, 2005), None);
        assert_eq!(of_country(&series, "FRA", &iso, 2000), None);
    }
}
