//! The panel behind `ceteris calibrate rho` (ADR 0027, specs/p3/PLAN.md Task E2): BACI flows by
//! theta class and ordered country pair, joined to bilateral applied tariffs.
//!
//! BACI is read year by year from the package zip. A tariff file is owner-side input; which
//! source fills it is a register matter (`sources.toml`: `unctad-trains` is the fetch-only
//! fallback, never in the paid build). Pairs and classes absent from it carry a tariff of 0 in
//! every year, that is no change, which is the control group of the projection.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use ceteris_pack::econ::goods::{TRADED, hs_goods};
use ceteris_pack::econ::theta::class_table;
use ceteris_pack::read::baci::{country_codes, for_each_flow_in, read_hs_cpa};

use crate::rho::{Design, Row};

/// Class label of a theta class index.
pub fn class_label(c: u8) -> String {
    format!("c{c}")
}

/// Class label of each HS 6-digit line (BACI's integer code) that belongs to a traded good, by
/// the ADR 0023 classes of the 2000 BACI flows.
pub fn hs_classes(baci: &Path, hs_key: &Path) -> Result<BTreeMap<i64, String>, String> {
    let (of, _, _) = class_table(baci, hs_key)?;
    let hs_cpa = read_hs_cpa(hs_key)?;
    Ok(hs_goods(&hs_cpa)
        .into_iter()
        .filter(|&(_, g)| g < TRADED)
        .filter_map(|(hs, g)| Some((hs.parse::<i64>().ok()?, class_label(of[g]))))
        .collect())
}

/// Years the panel covers for a design: two before the base to five after the shock, within BACI.
pub fn years(design: Design, last: i32) -> Vec<i32> {
    ((design.base - 2)..=(design.shock + 5).min(last)).collect()
}

/// Applied tariffs (ad valorem, as a fraction) by exporter, importer, year and class; the class
/// `*` applies to every class without its own line.
#[derive(Default)]
pub struct Tariffs(BTreeMap<(String, String, i32, String), f64>);

impl Tariffs {
    /// Parses `exporter,importer,year,class,tariff` lines (ISO3 codes); a header and `#`
    /// comments are skipped.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut t = BTreeMap::new();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("exporter,") {
                continue;
            }
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            let [e, m, y, c, v] = f[..] else {
                return Err(format!(
                    "line {}: expected exporter,importer,year,class,tariff",
                    i + 1
                ));
            };
            let bad = |what: &str| format!("line {}: bad {what}", i + 1);
            t.insert(
                (
                    e.into(),
                    m.into(),
                    y.parse().map_err(|_| bad("year"))?,
                    c.into(),
                ),
                v.parse().map_err(|_| bad("tariff"))?,
            );
        }
        Ok(Tariffs(t))
    }

    fn get(&self, exporter: &str, importer: &str, year: i32, class: &str) -> f64 {
        let key = |c: &str| (exporter.into(), importer.into(), year, c.into());
        self.0
            .get(&key(class))
            .or_else(|| self.0.get(&key("*")))
            .copied()
            .unwrap_or(0.0)
    }
}

/// Days since 1970-01-01 of a civil date.
fn days(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(y) - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468
}

impl Tariffs {
    /// Reads the Global Tariff Database trade-war extension (Teti, HS6 bilateral file): one row
    /// per importer, exporter and HS6 line with the applied rate in percent on each date a rate
    /// changed (`t_YYYYMMDD`) and the 2017 baseline `v2017`. The tariff of a class and year is
    /// the mean over its lines of the line's time-weighted rate within the year; years before
    /// 2018 take the 2017 baseline. Lines outside `classes` are left out, and the file's HS2017
    /// codes are matched to BACI's HS92 codes as they stand.
    pub fn from_gtd_war(
        path: &Path,
        classes: &BTreeMap<i64, String>,
        years: &[i32],
    ) -> Result<Self, String> {
        let origin = path.display();
        let mut lines = std::io::BufReader::new(
            std::fs::File::open(path).map_err(|e| format!("{origin}: {e}"))?,
        )
        .lines();
        let header = lines
            .next()
            .ok_or(format!("{origin}: empty"))?
            .map_err(|e| e.to_string())?;
        let cols: Vec<&str> = header.split(',').collect();
        let find = |n: &str| {
            cols.iter()
                .position(|c| *c == n)
                .ok_or(format!("{origin}: no column {n}"))
        };
        let (imp, exp, hs, base) = (
            find("importer")?,
            find("exporter")?,
            find("hs6")?,
            find("v2017")?,
        );
        let steps: Vec<(usize, i64)> = cols
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let d = c.strip_prefix("t_")?;
                let (y, m, dd) = (d.get(0..4)?, d.get(4..6)?, d.get(6..8)?);
                Some((i, days(y.parse().ok()?, m.parse().ok()?, dd.parse().ok()?)))
            })
            .collect();
        // weight of each step column within each year: days it is in force over days in the year
        let weights: Vec<Vec<(usize, f64)>> = years
            .iter()
            .map(|&y| {
                let (a, b) = (days(y, 1, 1), days(y + 1, 1, 1));
                steps
                    .iter()
                    .enumerate()
                    .filter_map(|(k, &(col, start))| {
                        let end = steps.get(k + 1).map_or(i64::MAX, |s| s.1);
                        let overlap = (end.min(b) - start.max(a)).max(0);
                        (overlap > 0).then(|| (col, overlap as f64 / (b - a) as f64))
                    })
                    .collect()
            })
            .collect();
        let mut acc: BTreeMap<(String, String, i32, String), (f64, u32)> = BTreeMap::new();
        for (n, line) in lines.enumerate() {
            let line = line.map_err(|e| e.to_string())?;
            let f: Vec<&str> = line.split(',').collect();
            let bad = || format!("{origin}: line {}", n + 2);
            let Some(class) = f
                .get(hs)
                .and_then(|h| h.parse::<i64>().ok())
                .and_then(|h| classes.get(&h))
            else {
                continue;
            };
            let cell = |i: usize| f.get(i).and_then(|v| v.parse::<f64>().ok());
            let (e, m) = (f.get(exp).ok_or_else(bad)?, f.get(imp).ok_or_else(bad)?);
            for (&year, w) in years.iter().zip(&weights) {
                let rate = if year < 2018 {
                    cell(base)
                } else {
                    w.iter()
                        .map(|&(c, wt)| cell(c).map(|v| v * wt))
                        .sum::<Option<f64>>()
                };
                let Some(rate) = rate else { continue };
                let slot = acc
                    .entry((e.to_string(), m.to_string(), year, class.clone()))
                    .or_insert((0.0, 0));
                slot.0 += rate / 100.0;
                slot.1 += 1;
            }
        }
        Ok(Tariffs(
            acc.into_iter()
                .map(|(k, (s, n))| (k, s / f64::from(n)))
                .collect(),
        ))
    }
}

/// The panel: BACI value (thousand USD) summed over the lines of each class, by ordered pair
/// `EXP>IMP` and year, with the tariff of that pair and class. Rows come in class, pair, year
/// order.
pub fn extract(
    baci: &Path,
    classes: &BTreeMap<i64, String>,
    tariffs: &Tariffs,
    years: &[i32],
) -> Result<Vec<Row>, String> {
    let iso = country_codes(baci)?;
    let mut sums: BTreeMap<(String, String, i32), f64> = BTreeMap::new();
    for &year in years {
        for_each_flow_in(baci, &format!("Y{year}_V202601.csv"), |flow| {
            let Some(fl) = flow else { return };
            let (Some(class), Some(e), Some(m)) =
                (classes.get(&fl.k), iso.get(&fl.i), iso.get(&fl.j))
            else {
                return;
            };
            if fl.v.is_finite() {
                *sums
                    .entry((class.clone(), format!("{e}>{m}"), year))
                    .or_insert(0.0) += fl.v;
            }
        })?;
    }
    Ok(sums
        .into_iter()
        .map(|((class, pair, year), flow)| {
            let (e, m) = pair.split_once('>').unwrap_or_default();
            let tariff = tariffs.get(e, m, year, &class);
            Row {
                class,
                pair,
                year,
                flow,
                tariff,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    use crate::rho::estimate;

    const DESIGN: Design = Design {
        base: 2017,
        shock: 2019,
    };

    /// A BACI package of `pairs` exporters selling to one importer in two classes (HS lines
    /// 100000 and 200000, the second split over two lines), whose flows follow partial
    /// adjustment at `rho` to a tariff change that varies by pair, and the tariff file.
    fn simulated(dir: &Path, rho: f64, theta: f64, pairs: usize) -> (std::path::PathBuf, String) {
        let path = dir.join("BACI_HS92_V202601.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("country_codes_V202601.csv", opts).unwrap();
        let mut codes = String::from("country_code,country_name,country_iso2,country_iso3\n");
        codes += "900,Importer,II,IMP\n";
        for p in 0..pairs {
            codes += &format!("{},E{p},E,E{p}\n", p + 1);
        }
        zip.write_all(codes.as_bytes()).unwrap();
        let mut tariffs = String::from("exporter,importer,year,class,tariff\n");
        let mut seed = 99u64;
        let mut noise = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / f64::from(1u32 << 31) - 0.5) * 0.02
        };
        for year in 2015..=2024 {
            zip.start_file(format!("BACI_HS92_Y{year}_V202601.csv"), opts)
                .unwrap();
            let mut csv = String::from("t,i,j,k,v,q\n");
            for p in 0..pairs {
                let shock = 0.02 + 0.1 * (p as f64 / pairs as f64);
                let months = (12.0 * f64::from(year - DESIGN.shock) + 6.0).max(0.0);
                let g = if year >= DESIGN.shock {
                    1.0 - (1.0 - rho).powf(months)
                } else {
                    0.0
                };
                let ln_v = 10.0 + (p % 7) as f64 - theta * g * (1.0 + shock).ln() + noise();
                let v = ln_v.exp();
                csv += &format!("{year},{},900,100000,{v},1\n", p + 1);
                csv += &format!("{year},{},900,200000,{},1\n", p + 1, v / 4.0);
                csv += &format!("{year},{},900,200001,{},1\n", p + 1, v / 4.0 * 3.0);
                if year >= DESIGN.shock {
                    tariffs += &format!("E{p},IMP,{year},*,{shock}\n");
                }
            }
            zip.write_all(csv.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        (path, tariffs)
    }

    fn classes() -> BTreeMap<i64, String> {
        [(100000, "a"), (200000, "b"), (200001, "b")]
            .into_iter()
            .map(|(k, c)| (k, c.to_string()))
            .collect()
    }

    #[test]
    fn extraction_sums_lines_by_class_and_attaches_tariffs() {
        let dir = tempfile::tempdir().unwrap();
        let (baci, tariffs) = simulated(dir.path(), 0.05, 4.0, 5);
        let rows = extract(
            &baci,
            &classes(),
            &Tariffs::parse(&tariffs).unwrap(),
            &[2018, 2019],
        )
        .unwrap();
        assert_eq!(rows.len(), 2 * 5 * 2);
        let a = rows
            .iter()
            .find(|r| r.class == "a" && r.year == 2019 && r.pair == "E0>IMP");
        let b = rows
            .iter()
            .find(|r| r.class == "b" && r.year == 2019 && r.pair == "E0>IMP");
        let (a, b) = (a.unwrap(), b.unwrap());
        assert!((b.flow / a.flow - 1.0).abs() < 1e-9, "two lines sum to one");
        assert!(a.tariff > 0.0);
        assert_eq!(rows.iter().find(|r| r.year == 2018).unwrap().tariff, 0.0);
    }

    #[test]
    fn fit_recovers_known_rho_from_simulated_baci() {
        let dir = tempfile::tempdir().unwrap();
        let rho = 1.0 - 0.5f64.powf(1.0 / 24.0);
        let (baci, tariffs) = simulated(dir.path(), rho, 4.0, 300);
        let rows = extract(
            &baci,
            &classes(),
            &Tariffs::parse(&tariffs).unwrap(),
            &years(DESIGN, 2024),
        )
        .unwrap();
        let (fits, skipped) = estimate(&rows, DESIGN);
        assert!(skipped.is_empty());
        for f in &fits {
            assert!((f.half_life_months - 24.0).abs() <= 3.0, "{f:?}");
            assert!((f.theta - 4.0).abs() < 0.5, "{f:?}");
        }
    }

    #[test]
    fn gtd_war_averages_lines_and_weights_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("war.csv");
        std::fs::write(
            &path,
            "importer,exporter,hs6,t_20180101,t_20180701,v2017\n\
             USA,CHN,100000,0,10,0\n\
             USA,CHN,200000,20,20,0\n\
             USA,CHN,999999,50,50,0\n",
        )
        .unwrap();
        let t = Tariffs::from_gtd_war(&path, &classes(), &[2017, 2018, 2019]).unwrap();
        assert_eq!(t.get("CHN", "USA", 2017, "a"), 0.0);
        // 181 of 365 days at 0, the rest at 10 percent
        assert!((t.get("CHN", "USA", 2018, "a") - 0.1 * 184.0 / 365.0).abs() < 1e-9);
        assert!((t.get("CHN", "USA", 2019, "a") - 0.1).abs() < 1e-9);
        assert!((t.get("CHN", "USA", 2019, "b") - 0.2).abs() < 1e-9);
    }

    #[test]
    fn tariff_parse_rejects_bad_lines() {
        assert!(Tariffs::parse("A,B,2018,*\n").is_err());
        assert!(Tariffs::parse("A,B,x,*,0.1\n").is_err());
    }
}
