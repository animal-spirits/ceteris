//! The Yotov, Piermartini, Monteiro and Larch (2016) reproduction: the published PPML estimates
//! of their Tables 1 to 3 and the fits of their samples.

use std::cmp::Ordering;
use std::path::Path;

use ceteris_pack::read::read_member;
use ceteris_reference::ppml::{self, Fit};

use crate::dta::{Column, Dataset, read_dta, select};

pub const YOTOV: &str = "AdvancedGuideFiles.zip";
pub const YOTOV_YEARS: [f64; 6] = [1986.0, 1990.0, 1994.0, 1998.0, 2002.0, 2006.0];
pub const GRAVITY_TERMS: [&str; 3] = ["CNTG", "LANG", "CLNY"];

/// One published column: (term, estimate, pair-clustered s.e.), observations, R2 where printed.
pub struct Published {
    pub column: &'static str,
    pub terms: &'static [(&'static str, f64, f64)],
    pub n: usize,
    pub r2: Option<f64>,
}

/// Published PPML estimates and pair-clustered standard errors, transcribed from Yotov,
/// Piermartini, Monteiro and Larch (2016), An Advanced Guide to Trade Policy Analysis: The
/// Structural Gravity Model, WTO and UNCTAD: Table 1 column (4), p. 42; Table 2 columns (2) to
/// (5), p. 47; Table 3 columns (2) to (4), p. 51.
pub const YOTOV_PUBLISHED: [Published; 8] = [
    Published {
        column: "table1_ppml",
        terms: &[
            ("ln_DIST", -0.841, 0.032),
            ("CNTG", 0.437, 0.083),
            ("LANG", 0.247, 0.077),
            ("CLNY", -0.222, 0.116),
        ],
        n: 28152,
        r2: Some(0.614),
    },
    Published {
        column: "table2_ppml",
        terms: &[
            ("ln_DIST_1986", -0.859, 0.037),
            ("ln_DIST_1990", -0.834, 0.038),
            ("ln_DIST_1994", -0.835, 0.035),
            ("ln_DIST_1998", -0.847, 0.035),
            ("ln_DIST_2002", -0.848, 0.032),
            ("ln_DIST_2006", -0.836, 0.031),
            ("CNTG", 0.437, 0.083),
            ("LANG", 0.248, 0.077),
            ("CLNY", -0.222, 0.116),
        ],
        n: 28152,
        r2: None,
    },
    Published {
        column: "table2_intra",
        terms: &[
            ("ln_DIST_1986", -0.980, 0.072),
            ("ln_DIST_1990", -0.940, 0.073),
            ("ln_DIST_1994", -0.915, 0.072),
            ("ln_DIST_1998", -0.887, 0.071),
            ("ln_DIST_2002", -0.884, 0.071),
            ("ln_DIST_2006", -0.872, 0.071),
            ("CNTG", 0.371, 0.140),
            ("LANG", 0.337, 0.168),
            ("CLNY", 0.019, 0.156),
            ("ln_DIST_INTRA", -0.488, 0.101),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table2_brdr",
        terms: &[
            ("ln_DIST_1986", -0.857, 0.063),
            ("ln_DIST_1990", -0.819, 0.063),
            ("ln_DIST_1994", -0.796, 0.063),
            ("ln_DIST_1998", -0.770, 0.063),
            ("ln_DIST_2002", -0.767, 0.063),
            ("ln_DIST_2006", -0.754, 0.062),
            ("CNTG", 0.574, 0.155),
            ("LANG", 0.352, 0.137),
            ("CLNY", 0.027, 0.125),
            ("ln_DIST_INTRA", -0.602, 0.109),
            ("SMCTRY", 1.689, 0.574),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table2_fes",
        terms: &[
            ("ln_DIST_1986", -0.910, 0.032),
            ("ln_DIST_1990", -0.879, 0.032),
            ("ln_DIST_1994", -0.860, 0.032),
            ("ln_DIST_1998", -0.833, 0.032),
            ("ln_DIST_2002", -0.829, 0.032),
            ("ln_DIST_2006", -0.811, 0.032),
            ("CNTG", 0.442, 0.082),
            ("LANG", 0.241, 0.076),
            ("CLNY", -0.220, 0.117),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table3_ppml",
        terms: &[
            ("ln_DIST", -0.822, 0.031),
            ("CNTG", 0.416, 0.083),
            ("LANG", 0.250, 0.077),
            ("CLNY", -0.205, 0.114),
            ("RTA", 0.191, 0.066),
        ],
        n: 28152,
        r2: None,
    },
    Published {
        column: "table3_intra",
        terms: &[
            ("ln_DIST", -0.800, 0.030),
            ("CNTG", 0.393, 0.079),
            ("LANG", 0.244, 0.077),
            ("CLNY", -0.182, 0.113),
            ("RTA", 0.409, 0.069),
        ],
        n: 28566,
        r2: None,
    },
    Published {
        column: "table3_endg",
        terms: &[("RTA", 0.557, 0.102)],
        n: 28482,
        r2: None,
    },
];

/// A float ordered as numpy sorts finite values.
#[derive(Clone, Copy, PartialEq)]
struct Num(f64);

impl Eq for Num {}

impl PartialOrd for Num {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Num {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Group codes of `keys`: each key's rank among the distinct keys (numpy's `unique` inverse).
fn codes<K: Ord + Clone>(keys: &[K]) -> Vec<i64> {
    let distinct: Vec<K> = keys
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<K>>()
        .into_iter()
        .collect();
    keys.iter()
        .map(|k| distinct.binary_search(k).expect("a key of the set") as i64)
        .collect()
}

fn numbers<'a>(d: &'a Dataset, name: &str) -> Result<&'a [f64], String> {
    d.get(name)
        .and_then(Column::numbers)
        .ok_or_else(|| format!("no numeric column {name}"))
}

fn strings<'a>(d: &'a Dataset, name: &str) -> Result<&'a [String], String> {
    d.get(name)
        .and_then(Column::strings)
        .ok_or_else(|| format!("no string column {name}"))
}

fn yotov_data(path: &Path, member: &str) -> Result<Dataset, String> {
    let suffix = format!("Chapter1/Datasets/{member}");
    let d = read_dta(&read_member(path, |n| n.ends_with(&suffix))?)?;
    let keep: Vec<bool> = numbers(&d, "year")?
        .iter()
        .map(|y| YOTOV_YEARS.contains(y))
        .collect();
    Ok(d.into_iter().map(|(k, v)| (k, v.select(&keep))).collect())
}

/// Named regressor columns, in order.
pub(crate) type Columns = Vec<(String, Vec<f64>)>;

pub(crate) fn hdfe(
    flows: &[f64],
    fixed_effects: &[&[i64]],
    regressors: &Columns,
    cluster: Option<&[i64]>,
) -> Result<Fit, String> {
    let reg: Vec<(&str, &[f64])> = regressors
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_slice()))
        .collect();
    ppml::hdfe(flows, fixed_effects, &reg, cluster).map_err(|e| e.to_string())
}

/// The application's exporter-time and importer-time codes, pair codes, foreign mask and ln
/// distance.
struct Yotov {
    d: Dataset,
    exp_t: Vec<i64>,
    imp_t: Vec<i64>,
    foreign: Vec<bool>,
    ln_dist: Vec<f64>,
}

impl Yotov {
    fn read(path: &Path, member: &str) -> Result<Yotov, String> {
        let d = yotov_data(path, member)?;
        let year = numbers(&d, "year")?;
        let (exporter, importer) = (strings(&d, "exporter")?, strings(&d, "importer")?);
        let by_year = |c: &[String]| -> Vec<i64> {
            let keys: Vec<(String, Num)> = c
                .iter()
                .zip(year)
                .map(|(c, y)| (c.clone(), Num(*y)))
                .collect();
            codes(&keys)
        };
        let (exp_t, imp_t) = (by_year(exporter), by_year(importer));
        let foreign = exporter.iter().zip(importer).map(|(e, i)| e != i).collect();
        let ln_dist = numbers(&d, "DIST")?.iter().map(|x| x.ln()).collect();
        Ok(Yotov {
            d,
            exp_t,
            imp_t,
            foreign,
            ln_dist,
        })
    }

    fn col(&self, name: &str) -> Result<Vec<f64>, String> {
        numbers(&self.d, name).map(<[f64]>::to_vec)
    }

    fn gravity(&self) -> Result<Columns, String> {
        GRAVITY_TERMS
            .iter()
            .map(|k| Ok(((*k).to_owned(), self.col(k)?)))
            .collect()
    }

    /// One group per country's internal trade, one for all international.
    fn intra_fe(&self) -> Result<Vec<i64>, String> {
        let keys: Vec<&str> = strings(&self.d, "exporter")?
            .iter()
            .zip(&self.foreign)
            .map(|(e, f)| if *f { "" } else { e.as_str() })
            .collect();
        Ok(codes(&keys))
    }

    fn foreign_fit(&self, terms: &Columns, pair: &[i64]) -> Result<Fit, String> {
        let f = &self.foreign;
        let sub: Columns = terms
            .iter()
            .map(|(k, v)| (k.clone(), select(v, f)))
            .collect();
        hdfe(
            &select(&self.col("trade")?, f),
            &[&select(&self.exp_t, f), &select(&self.imp_t, f)],
            &sub,
            Some(&select(pair, f)),
        )
    }
}

/// The PPML columns of [`YOTOV_PUBLISHED`], in its order, estimated as in the authors' Stata
/// files (`TraditionalGravity.do`, `DistancePuzzle.do`, `RTAsEffects.do`): exporter-time and
/// importer-time fixed effects, standard errors clustered by country pair.
pub fn yotov(path: &Path) -> Result<Vec<(String, Fit)>, String> {
    let mut fits = Vec::new();
    let y = Yotov::read(path, "Chapter1Application1.dta")?;
    let pair = codes(
        &numbers(&y.d, "pair_id")?
            .iter()
            .map(|&p| Num(p))
            .collect::<Vec<_>>(),
    );
    let mut terms: Columns = vec![("ln_DIST".into(), y.ln_dist.clone())];
    terms.extend(y.gravity()?);
    fits.push(("table1_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));

    let y = Yotov::read(path, "Chapter1Application2.dta")?;
    let pair = codes(
        &numbers(&y.d, "pair_id")?
            .iter()
            .map(|&p| Num(p))
            .collect::<Vec<_>>(),
    );
    let smctry: Vec<f64> = y.foreign.iter().map(|f| f64::from(u8::from(!f))).collect();
    let year = y.col("year")?;
    let by_year: Columns = YOTOV_YEARS
        .iter()
        .map(|&t| {
            let v = year
                .iter()
                .zip(&y.ln_dist)
                .map(|(&yr, &l)| if yr == t { l } else { 0.0 })
                .collect();
            (format!("ln_DIST_{t}"), v)
        })
        .collect();
    let gravity = y.gravity()?;
    let mut terms = by_year.clone();
    terms.extend(gravity.clone());
    fits.push(("table2_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));
    let mut intra_years: Columns = by_year
        .iter()
        .map(|(k, v)| {
            let v = v.iter().zip(&smctry).map(|(x, s)| x * (1.0 - s)).collect();
            (k.clone(), v)
        })
        .collect();
    let trade = y.col("trade")?;
    let fe: [&[i64]; 2] = [&y.exp_t, &y.imp_t];
    let mut intra = intra_years.clone();
    intra.extend(gravity.clone());
    intra.push((
        "ln_DIST_INTRA".into(),
        y.ln_dist.iter().zip(&smctry).map(|(l, s)| l * s).collect(),
    ));
    fits.push((
        "table2_intra".to_owned(),
        hdfe(&trade, &fe, &intra, Some(&pair))?,
    ));
    let mut brdr = intra;
    brdr.push(("SMCTRY".into(), smctry));
    fits.push((
        "table2_brdr".to_owned(),
        hdfe(&trade, &fe, &brdr, Some(&pair))?,
    ));
    let intra_fe = y.intra_fe()?;
    intra_years.extend(gravity);
    fits.push((
        "table2_fes".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &intra_fe],
            &intra_years,
            Some(&pair),
        )?,
    ));

    let y = Yotov::read(path, "Chapter1Application3.dta")?;
    // As RTAsEffects.do: internal pairs apart.
    let pair_keys: Vec<Num> = numbers(&y.d, "pair_id")?
        .iter()
        .zip(&y.foreign)
        .map(|(&p, &f)| Num(p + if f { 0.0 } else { 100_000.0 }))
        .collect();
    let pair = codes(&pair_keys);
    let mut terms: Columns = vec![("ln_DIST".into(), y.ln_dist.clone())];
    terms.extend(y.gravity()?);
    terms.push(("RTA".into(), y.col("RTA")?));
    fits.push(("table3_ppml".to_owned(), y.foreign_fit(&terms, &pair)?));
    let trade = y.col("trade")?;
    let intra_fe = y.intra_fe()?;
    fits.push((
        "table3_intra".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &intra_fe],
            &terms,
            Some(&pair),
        )?,
    ));
    fits.push((
        "table3_endg".to_owned(),
        hdfe(
            &trade,
            &[&y.exp_t, &y.imp_t, &pair],
            &vec![("RTA".into(), y.col("RTA")?)],
            Some(&pair),
        )?,
    ));
    Ok(fits)
}
