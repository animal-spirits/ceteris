//! Trade elasticity theta of the P2 goods from Caliendo and Parro (2015) sector estimates, and
//! its classes (ADRs 0018 and 0023).

use std::collections::BTreeMap;
use std::path::Path;

use super::goods::{TRADED, hs_goods};
use crate::numeric::pairwise_sum;
use crate::read::baci::{ByLine, baci_values, read_hs_cpa};
use crate::read::icio::N_GOODS;
use crate::read::wdi::Names;

/// A Caliendo and Parro sector.
pub struct Sector {
    pub name: &'static str,
    /// The ISIC Rev 3 divisions of Table A.1.
    pub divisions: &'static [&'static str],
    pub theta: f64,
    /// Heteroskedasticity-robust standard error of theta.
    pub se: f64,
}

const fn sector(
    name: &'static str,
    divisions: &'static [&'static str],
    theta: f64,
    se: f64,
) -> Sector {
    Sector {
        name,
        divisions,
        theta,
        se,
    }
}

/// Caliendo and Parro (2015), Table 1, 99% sample (their benchmark, column 5). Review of
/// Economic Studies 82(1), Advance Access version of 14 November 2014, p. 18 (Table 1) and p. 42
/// (Table A.1). Transcribed in ADR 0023.
pub const CALIENDO_PARRO: [Sector; 20] = [
    sector("Agriculture", &["01", "02", "05"], 9.11, 2.01),
    sector("Mining", &["10", "11", "12", "13", "14"], 13.53, 3.67),
    sector("Food", &["15", "16"], 2.62, 0.61),
    sector("Textile", &["17", "18", "19"], 8.1, 1.28),
    sector("Wood", &["20"], 11.5, 2.87),
    sector("Paper", &["21", "22"], 16.52, 2.65),
    sector("Petroleum", &["23"], 64.85, 15.61),
    sector("Chemicals", &["24"], 3.13, 1.78),
    sector("Plastic", &["25"], 1.67, 2.23),
    sector("Minerals", &["26"], 2.41, 1.6),
    sector("Basic metals", &["27"], 3.28, 2.51),
    sector("Metal products", &["28"], 6.99, 2.12),
    sector("Machinery n.e.c.", &["29"], 1.45, 2.8),
    sector("Office", &["30"], 12.95, 4.53),
    sector("Electrical", &["31"], 12.91, 1.64),
    sector("Communication", &["32"], 3.95, 1.77),
    sector("Medical", &["33"], 8.71, 1.56),
    sector("Auto", &["34"], 1.84, 0.92),
    sector("Other Transport", &["35"], 0.39, 1.08),
    sector("Other", &["36", "37"], 3.98, 1.08),
];

/// ISIC Rev 3 division of each CPA 2.1 activity of the OECD key, from the Rev 3 to Rev 4
/// breaks: weapons (C254) and military vehicles (C304) were Rev 3 2927, computers (C262) Rev 3
/// 30, magnetic and optical media (C268) Rev 3 2429, medical instruments (C325) Rev 3 3311,
/// recycling (E38) Rev 3 37; books and recorded media arrive as C18, Rev 3 22. Domestic
/// appliances (Rev 3 2930) sit inside C27 and count as Electrical. Electricity and the service
/// codes (D35, H53, M71, M74, R90, R91) and unallocated lines have no estimate.
pub const REV3_OF_CPA: [(&str, &str); 45] = [
    ("A01", "01"),
    ("A02", "02"),
    ("A03", "05"),
    ("B05", "10"),
    ("B06", "11"),
    ("B071", "13"),
    ("B072", "13"),
    ("B08", "14"),
    ("C10", "15"),
    ("C11", "15"),
    ("C12", "16"),
    ("C13", "17"),
    ("C14", "18"),
    ("C15", "19"),
    ("C16", "20"),
    ("C17", "21"),
    ("C18", "22"),
    ("C19", "23"),
    ("C20", "24"),
    ("C21", "24"),
    ("C268", "24"),
    ("C22", "25"),
    ("C23", "26"),
    ("C24A", "27"),
    ("C24B", "27"),
    ("C25X254", "28"),
    ("C254", "29"),
    ("C28", "29"),
    ("C304", "29"),
    ("C262", "30"),
    ("C27", "31"),
    ("C261", "32"),
    ("C263", "32"),
    ("C264", "32"),
    ("C265", "33"),
    ("C266", "33"),
    ("C267", "33"),
    ("C325", "33"),
    ("C29", "34"),
    ("C301", "35"),
    ("C302_309", "35"),
    ("C303", "35"),
    ("C31", "36"),
    ("C32X325", "36"),
    ("E38", "37"),
];

/// Largest ratio of two thetas in one class (ADR 0023), and the allowed class counts.
pub const CLASS_SPREAD: f64 = 1.5;
pub const CLASS_COUNTS: [usize; 3] = [3, 4, 5];

fn sector_of_cpa(code: &str) -> Option<usize> {
    let division = REV3_OF_CPA.iter().find(|(c, _)| *c == code)?.1;
    CALIENDO_PARRO
        .iter()
        .position(|s| s.divisions.contains(&division))
}

/// Theta per good, the trade-value-weighted arithmetic mean of the sector estimates over the HS
/// lines of the good (the sector of a line is the ISIC Rev 3 division of its CPA activity), and
/// the trade value of the lines with an estimate. Lines without one, and goods without such
/// lines, weigh nothing (theta 0).
pub fn by_good(
    hs_cpa: &Names,
    lines: &BTreeMap<&str, usize>,
    value: &ByLine,
) -> ([f64; N_GOODS], [f64; N_GOODS]) {
    let (mut num, mut den) = ([0.0; N_GOODS], [0.0; N_GOODS]);
    for (&hs, &good) in lines {
        let v = value.get(hs).copied().unwrap_or(0.0);
        let Some(sector) = sector_of_cpa(&hs_cpa[hs]) else {
            continue;
        };
        if v == 0.0 {
            continue;
        }
        num[good] += v * CALIENDO_PARRO[sector].theta;
        den[good] += v;
    }
    let mut theta = [0.0; N_GOODS];
    for g in 0..N_GOODS {
        if den[g] > 0.0 {
            theta[g] = num[g] / den[g];
        }
    }
    (theta, den)
}

/// Trade value (USD) of each good's lines by Caliendo-Parro sector, `[good][sector]` in the
/// order of `CALIENDO_PARRO`: the weights behind `by_good`, whose theta of good g is the row's
/// mean of the sector thetas.
pub fn sector_values(
    hs_cpa: &Names,
    lines: &BTreeMap<&str, usize>,
    value: &ByLine,
) -> [[f64; 20]; N_GOODS] {
    let mut out = [[0.0; 20]; N_GOODS];
    for (&hs, &good) in lines {
        let v = value.get(hs).copied().unwrap_or(0.0);
        let Some(sector) = sector_of_cpa(&hs_cpa[hs]) else {
            continue;
        };
        if v == 0.0 {
            continue;
        }
        out[good][sector] += v;
    }
    out
}

/// H2 prior band of each class (ADR 0023, Prior band): its theta is the mean of the sector
/// estimates weighted by the class's trade value per sector (the weights of `classes`), and its
/// standard error that of the same weighted mean of independent estimates,
/// sqrt(sum_s w_s^2 se_s^2). `values` is `sector_values` of the traded goods, `of` their
/// classes. Returns each class's theta and standard error; the band is theta +/- 2 se.
pub fn class_bands(values: &[[f64; 20]], of: &[u8]) -> (Vec<f64>, Vec<f64>) {
    let classes = usize::from(of.iter().copied().max().unwrap_or(0)) + 1;
    let (mut centre, mut se) = (Vec::new(), Vec::new());
    for c in 0..classes {
        let mut by_class = [0.0; 20];
        for row in values
            .iter()
            .zip(of)
            .filter(|(_, o)| usize::from(**o) == c)
            .map(|(r, _)| r)
        {
            for (s, x) in by_class.iter_mut().zip(row) {
                *s += x;
            }
        }
        let total = pairwise_sum(&by_class);
        let w = by_class.map(|x| x / total);
        centre.push(dot(w.iter().zip(&CALIENDO_PARRO).map(|(w, s)| w * s.theta)));
        se.push(
            dot(w
                .iter()
                .zip(&CALIENDO_PARRO)
                .map(|(w, s)| (w * w) * (s.se * s.se)))
            .sqrt(),
        );
    }
    (centre, se)
}

fn dot(terms: impl Iterator<Item = f64>) -> f64 {
    terms.fold(0.0, |acc, x| acc + x)
}

/// `caliendo_parro_by_good` and the trade value behind each good's theta: theta per good (0 for
/// the local goods 7 to 9) through the OECD HS to CPA key and BACI 2000 values.
pub fn caliendo_parro_with_value(
    baci_2000: &Path,
    hs_key: &Path,
) -> Result<([f64; N_GOODS], [f64; N_GOODS]), String> {
    let hs_cpa = read_hs_cpa(hs_key)?;
    let lines = hs_goods(&hs_cpa);
    let (mut theta, mut value) = by_good(&hs_cpa, &lines, &baci_values(baci_2000)?);
    for g in TRADED..N_GOODS {
        (theta[g], value[g]) = (0.0, 0.0);
    }
    Ok((theta, value))
}

/// Theta per good (0 for the local goods 7 to 9).
pub fn caliendo_parro_by_good(baci_2000: &Path, hs_key: &Path) -> Result<[f64; N_GOODS], String> {
    Ok(caliendo_parro_with_value(baci_2000, hs_key)?.0)
}

/// Sort the thetas by value and split them into the fewest classes (3 to 5) of consecutive
/// goods whose largest theta is at most `CLASS_SPREAD` times their smallest; among such splits
/// the one with the smallest largest spread, then the first in order. Returns the class of each
/// good (0 for the lowest thetas) and each class's theta, the trade-value-weighted mean of its
/// goods'.
pub fn classes(theta: &[f64], value: &[f64]) -> Result<(Vec<u8>, Vec<f64>), String> {
    let n = theta.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        theta[a]
            .partial_cmp(&theta[b])
            .unwrap_or_else(|| theta[a].is_nan().cmp(&theta[b].is_nan()))
    });
    let ts: Vec<f64> = order.iter().map(|&i| theta[i]).collect();
    for k in CLASS_COUNTS {
        let mut best: Option<(f64, Vec<usize>)> = None;
        for cuts in combinations(n, k - 1) {
            let bounds: Vec<usize> = std::iter::once(0)
                .chain(cuts)
                .chain(std::iter::once(n))
                .collect();
            // As a ratio, exact at the bound: a log spread of ln 1.5 can round either way.
            let spread = bounds
                .windows(2)
                .map(|w| ts[w[1] - 1] / ts[w[0]])
                .fold(f64::NEG_INFINITY, f64::max);
            if spread <= CLASS_SPREAD && best.as_ref().is_none_or(|(s, _)| spread < *s) {
                best = Some((spread, bounds));
            }
        }
        if let Some((_, bounds)) = best {
            let mut of = vec![0u8; n];
            let mut class_theta = Vec::new();
            for (c, w) in bounds.windows(2).enumerate() {
                let members = &order[w[0]..w[1]];
                for &m in members {
                    of[m] = u8::try_from(c).map_err(|e| e.to_string())?;
                }
                let weights: Vec<f64> = members.iter().map(|&m| value[m]).collect();
                let total = pairwise_sum(&weights);
                if total == 0.0 {
                    return Err("Weights sum to zero, can't be normalized".into());
                }
                let products: Vec<f64> = members.iter().map(|&m| theta[m] * value[m]).collect();
                class_theta.push(pairwise_sum(&products) / total);
            }
            return Ok((of, class_theta));
        }
    }
    Err(format!(
        "no split into {} classes keeps theta within a factor {CLASS_SPREAD}: {theta:?}",
        CLASS_COUNTS[CLASS_COUNTS.len() - 1]
    ))
}

/// The cut positions of every choice of `k` of the positions 1 to n-1, in lexicographic order
/// (`itertools.combinations`).
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn rec(from: usize, n: usize, k: usize, at: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if at.len() == k {
            out.push(at.clone());
            return;
        }
        for i in from..n {
            at.push(i);
            rec(i + 1, n, k, at, out);
            at.pop();
        }
    }
    let mut out = Vec::new();
    rec(1, n, k, &mut Vec::new(), &mut out);
    out
}
