//! The ICIO reader (`aggregate`, `exports`) on hand-made tables and on the 2000 file, and the
//! income series.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use ceteris_pack::numeric::repr_f64;
use ceteris_pack::read::icio::{CountryIo, N_GOODS, aggregate, exports, home_use, income_by_year};
use common::{Uniform, approx, cached, write_zip};

// Industries of the hand-made table and their goods: A01 and A02 are good 0, B06 good 1, C10T12 good 3, G and K good 9.
const INDUSTRIES: [&str; 6] = ["A01", "A02", "B06", "C10T12", "G", "K"];
const GOOD: [usize; 6] = [0, 0, 1, 3, 9, 9];
const FINAL: [&str; 6] = ["HFCE", "NPISH", "GGFC", "GFCF", "INVNT", "DPABR"];

struct Parts {
    z: Vec<Vec<f64>>,
    fd: Vec<Vec<f64>>,
    out: Vec<f64>,
}

/// An ICIO-shaped CSV (industry block, final demand block, OUT; rows then TLS, VA, OUT) and its parts.
fn table(invnt: Option<f64>, deplete: Option<f64>, seed: u64) -> (String, Parts) {
    table_of(&INDUSTRIES, invnt, deplete, seed)
}

fn table_of(
    industries: &[&str],
    invnt: Option<f64>,
    deplete: Option<f64>,
    seed: u64,
) -> (String, Parts) {
    let economies = ["AAA", "BBB"];
    let mut rng = Uniform::new(seed);
    let cols: Vec<String> = economies
        .iter()
        .flat_map(|e| industries.iter().map(move |i| format!("{e}_{i}")))
        .collect();
    let fd_cols: Vec<String> = economies
        .iter()
        .flat_map(|e| FINAL.map(|f| format!("{e}_{f}")))
        .collect();
    let n = cols.len();
    let z: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..n).map(|_| rng.next(1.0, 10.0)).collect())
        .collect();
    let mut fd: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..fd_cols.len()).map(|_| rng.next(1.0, 10.0)).collect())
        .collect();
    if let Some(v) = invnt {
        for (j, c) in fd_cols.iter().enumerate() {
            if c.ends_with("INVNT") {
                for (i, row) in fd.iter_mut().enumerate() {
                    row[j] = if i == 0 { deplete.unwrap_or(v) } else { v };
                }
            }
        }
    }
    let tls: Vec<f64> = (0..n).map(|_| rng.next(1.0, 3.0)).collect();
    let va: Vec<f64> = (0..n).map(|_| rng.next(10.0, 30.0)).collect();
    let out: Vec<f64> = (0..n)
        .map(|j| (0..n).map(|i| z[i][j]).sum::<f64>() + tls[j] + va[j])
        .collect();
    let f = |xs: &[f64]| {
        xs.iter()
            .map(|x| repr_f64(*x))
            .collect::<Vec<_>>()
            .join(",")
    };
    let zeros = vec!["0"; fd_cols.len()].join(",");
    let mut lines = vec![format!("V1,{},{},OUT", cols.join(","), fd_cols.join(","))];
    for (i, c) in cols.iter().enumerate() {
        let total = z[i].iter().sum::<f64>() + fd[i].iter().sum::<f64>();
        lines.push(format!(
            "{c},{},{},{}",
            f(&z[i]),
            f(&fd[i]),
            repr_f64(total)
        ));
    }
    for (label, row) in [("TLS", &tls), ("VA", &va), ("OUT", &out)] {
        lines.push(format!(
            "{label},{},{zeros},{}",
            f(row),
            repr_f64(row.iter().sum())
        ));
    }
    (lines.join("\n") + "\n", Parts { z, fd, out })
}

fn write_table(
    dir: &Path,
    name: &str,
    invnt: Option<f64>,
    deplete: Option<f64>,
) -> (PathBuf, Parts) {
    let (text, parts) = table(invnt, deplete, 1);
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    (path, parts)
}

fn col_sum(io: &[[f64; N_GOODS]; N_GOODS], g: usize) -> f64 {
    io.iter().map(|row| row[g]).sum()
}

#[test]
fn aggregation_preserves_output_totals() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let got = aggregate(&path, 2000).unwrap();
    assert_eq!(got.keys().collect::<Vec<_>>(), ["AAA", "BBB"]);
    for (k, code) in ["AAA", "BBB"].iter().enumerate() {
        let block = k * 6..k * 6 + 6;
        let c = &got[*code];
        let out: f64 = parts.out[block.clone()].iter().sum();
        assert!(approx(c.output.iter().sum(), out, 1e-9));
        // VA is output less intermediate inputs (ICIO VA plus taxes less subsidies on products).
        let used: f64 = block
            .clone()
            .map(|j| parts.z.iter().map(|r| r[j]).sum::<f64>())
            .sum();
        assert!(approx(c.va.iter().sum(), out - used, 1e-9));
        for g in 0..N_GOODS {
            let want: f64 = block
                .clone()
                .filter(|j| GOOD[j % 6] == g)
                .map(|j| parts.out[j])
                .sum();
            assert!(approx(c.output[g], want, 1e-9));
        }
    }
}

/// Refined petroleum (C19) aggregates with crude oil and gas into good 1, not with chemicals into
/// good 5.
#[test]
fn good_1_and_5_totals_follow_the_c19_move() {
    let industries = ["B06", "C19", "C20", "C24A", "G", "K"];
    let goods = [1, 1, 5, 5, 9, 9];
    let dir = tempfile::tempdir().unwrap();
    let (text, parts) = table_of(&industries, None, None, 3);
    let path = dir.path().join("2000_SML.csv");
    fs::write(&path, text).unwrap();
    let got = aggregate(&path, 2000).unwrap();
    let n = industries.len();
    for (k, code) in ["AAA", "BBB"].iter().enumerate() {
        let c = &got[*code];
        for g in [1, 5] {
            let cols: Vec<usize> = (0..n)
                .filter(|j| goods[*j] == g)
                .map(|j| k * n + j)
                .collect();
            let out: f64 = cols.iter().map(|&j| parts.out[j]).sum();
            let used: f64 = cols
                .iter()
                .map(|&j| parts.z.iter().map(|r| r[j]).sum::<f64>())
                .sum();
            assert!(approx(c.output[g], out, 1e-9), "{code} good {g} output");
            assert!(approx(c.va[g], out - used, 1e-9), "{code} good {g} va");
        }
    }
}

#[test]
fn exports_sum_sales_to_foreign_columns() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let got = exports(&path, 2000).unwrap();
    let n_fd = FINAL.len();
    for (k, code) in ["AAA", "BBB"].iter().enumerate() {
        let other = 1 - k;
        let mut want = [0.0; N_GOODS];
        for i in k * 6..k * 6 + 6 {
            let abroad: f64 = (other * 6..other * 6 + 6)
                .map(|j| parts.z[i][j])
                .sum::<f64>()
                + (other * n_fd..(other + 1) * n_fd)
                    .map(|j| parts.fd[i][j])
                    .sum::<f64>();
            want[GOOD[i % 6]] += abroad;
        }
        for (g, w) in got[*code].iter().zip(want) {
            assert!(approx(*g, w, 1e-12), "{code}");
        }
    }
}

/// ADR 0092's home share: each economy's use (intermediate plus the `FINAL_DEMAND` columns, not
/// DPABR) of goods produced at home over its use from every source economy, per good.
#[test]
fn home_shares_follow_the_icio_aggregation() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let got = home_use(&path, 2000).unwrap();
    assert_eq!(got.keys().collect::<Vec<_>>(), ["AAA", "BBB"]);
    let n_fd = FINAL.len();
    for (k, code) in ["AAA", "BBB"].iter().enumerate() {
        let (mut home, mut total) = ([0.0; N_GOODS], [0.0; N_GOODS]);
        for (i, (z, fd)) in parts.z.iter().zip(&parts.fd).enumerate() {
            let used: f64 = (k * 6..k * 6 + 6).map(|j| z[j]).sum::<f64>()
                + (k * n_fd..k * n_fd + 5).map(|j| fd[j]).sum::<f64>();
            total[GOOD[i % 6]] += used;
            if i / 6 == k {
                home[GOOD[i % 6]] += used;
            }
        }
        let u = &got[*code];
        for g in 0..N_GOODS {
            assert!(approx(u.home[g], home[g], 1e-12), "{code} good {g} home");
            assert!(approx(u.total[g], total[g], 1e-12), "{code} good {g} total");
        }
        assert!(u.share(0) > 0.0 && u.share(0) < 1.0);
        assert!(approx(u.share(0), home[0] / total[0], 1e-12));
        assert!(u.share(2).is_nan(), "a good without use has no share");
    }
}

#[test]
fn value_added_closes_the_identity() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_table(dir.path(), "2000_SML.csv", None, None);
    for c in aggregate(&path, 2000).unwrap().values() {
        for g in 0..N_GOODS {
            assert!(approx(
                c.va[g] + col_sum(&c.io, g) * c.output[g],
                c.output[g],
                1e-9
            ));
        }
    }
}

#[test]
fn intermediates_count_inputs_from_every_source_economy() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let a = &aggregate(&path, 2000).unwrap()["AAA"];
    // Input of good 9 (rows G, K of both economies) per unit of AAA's good 0 (columns A01, A02).
    let rows: Vec<usize> = (0..12).filter(|i| GOOD[i % 6] == 9).collect();
    let used: f64 = rows
        .iter()
        .flat_map(|&i| [parts.z[i][0], parts.z[i][1]])
        .sum();
    assert!(approx(
        a.io[9][0],
        used / (parts.out[0] + parts.out[1]),
        1e-12
    ));
}

#[test]
fn io_coefficients_below_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_table(dir.path(), "2000_SML.csv", None, None);
    for c in aggregate(&path, 2000).unwrap().values() {
        assert!(c.io.iter().flatten().all(|x| *x >= 0.0));
        assert!((0..N_GOODS).all(|g| col_sum(&c.io, g) < 1.0));
    }
}

#[test]
fn goods_without_output_have_an_empty_column() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_table(dir.path(), "2000_SML.csv", None, None);
    let a = &aggregate(&path, 2000).unwrap()["AAA"];
    // Goods 2, 4 to 8 have no industry in the hand table.
    assert_eq!(a.output[2], 0.0);
    assert!(a.io.iter().all(|row| row[2] == 0.0));
    assert!(a.io.iter().flatten().all(|x| x.is_finite()));
}

#[test]
fn final_demand_shares_sum_to_one() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let got = &aggregate(&path, 2000).unwrap()["BBB"];
    assert!((got.final_demand_shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
    assert!(
        [2, 4, 5, 6, 7, 8]
            .iter()
            .all(|&g| got.final_demand_shares[g] == 0.0)
    );
    // BBB columns 6 to 11 of the final block, all five demand kinds but purchases abroad, every source economy.
    let fd =
        |rows: &[usize]| -> f64 { rows.iter().flat_map(|&i| parts.fd[i][6..11].to_vec()).sum() };
    let total = fd(&(0..12).collect::<Vec<_>>());
    assert!(approx(
        got.final_demand_shares[0],
        fd(&[0, 1, 6, 7]) / total,
        1e-12
    ));
    assert!(approx(
        got.final_demand_shares[3],
        fd(&[3, 9]) / total,
        1e-12
    ));
}

/// ADRs 0099, 0302: households (HFCE), government (NPISH and GGFC) and investment (GFCF)
/// split each good's final demand, inventory change shared in proportion to them, so the parts
/// add up to the final demand shares.
#[test]
fn final_demand_splits_into_three_demanders() {
    let dir = tempfile::tempdir().unwrap();
    let (path, parts) = write_table(dir.path(), "2000_SML.csv", None, None);
    let got = &aggregate(&path, 2000).unwrap()["BBB"];
    for (g, split) in got.final_demand_split.iter().enumerate() {
        assert!((split.iter().sum::<f64>() - 1.0).abs() < 1e-12, "good {g}");
        assert!(split.iter().all(|s| *s >= 0.0), "good {g}");
    }
    // good 0 is rows 0, 1, 6 and 7; BBB's HFCE, NPISH, GGFC, GFCF are columns 6 to 9
    let col = |j: usize| -> f64 { [0, 1, 6, 7].iter().map(|&i| parts.fd[i][j]).sum() };
    let bought = [col(6), col(7) + col(8), col(9)];
    let sum: f64 = bought.iter().sum();
    for (d, b) in bought.iter().enumerate() {
        assert!(
            approx(got.final_demand_split[0][d], b / sum, 1e-12),
            "demander {d}"
        );
    }
    let parts_total: f64 = (0..N_GOODS)
        .map(|g| {
            (0..3)
                .map(|d| got.final_demand_shares[g] * got.final_demand_split[g][d])
                .sum::<f64>()
        })
        .sum();
    assert!((parts_total - 1.0).abs() < 1e-12);
}

#[test]
fn inventory_changes_enter_final_demand() {
    let dir = tempfile::tempdir().unwrap();
    let (a, _) = write_table(dir.path(), "a.csv", Some(0.0), None);
    let (b, _) = write_table(dir.path(), "b.csv", Some(5.0), None);
    let share = |p: &Path| aggregate(p, 2000).unwrap()["AAA"].final_demand_shares;
    let (a, b) = (share(&a), share(&b));
    assert!(a.iter().zip(b).any(|(x, y)| !approx(*x, y, 1e-5)));
}

#[test]
fn negative_final_demand_share_stops_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let (path, _) = write_table(dir.path(), "neg.csv", Some(1.0), Some(-1e6));
    let e = aggregate(&path, 2000).unwrap_err();
    assert!(e.contains("negative final demand"), "{e}");
}

#[test]
fn aggregate_reads_a_zip_of_yearly_files() {
    let dir = tempfile::tempdir().unwrap();
    let (text, _) = table(None, None, 1);
    let plain = dir.path().join("2000_SML.csv");
    fs::write(&plain, &text).unwrap();
    let z = write_zip(
        &dir.path().join("1995-2000_SML.zip"),
        &[("1999_SML.csv", "wrong\n"), ("2000_SML.csv", &text)],
    );
    let total = |m: &BTreeMap<String, CountryIo>| m["AAA"].output.iter().sum::<f64>();
    assert_eq!(
        total(&aggregate(&z, 2000).unwrap()),
        total(&aggregate(&plain, 2000).unwrap())
    );
}

#[test]
fn combined_coal_industry_is_not_supported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.csv");
    fs::write(
        &path,
        "V1,AAA_B05_06,AAA_HFCE,OUT\nAAA_B05_06,1,1,2\nTLS,1,0,1\nVA,1,0,1\nOUT,3,0,3\n",
    )
    .unwrap();
    assert!(aggregate(&path, 2000).unwrap_err().contains("B05_06"));
}

#[test]
fn income_is_gdp_per_person_of_the_year_nearest_2000_within_1995_to_2010() {
    let series = |m: &[(&str, &[(i64, f64)])]| -> BTreeMap<String, BTreeMap<i64, f64>> {
        m.iter()
            .map(|(k, v)| ((*k).to_owned(), v.iter().copied().collect()))
            .collect()
    };
    let gdp = series(&[
        ("AAA", &[(2000, 100.0), (2001, 200.0)]),
        ("BBB", &[(1994, 50.0), (2008, 80.0), (2012, 90.0)]),
        ("CCC", &[(2012, 5.0)]),
    ]);
    let pop = series(&[
        ("AAA", &[(2000, 10.0), (2001, 10.0)]),
        ("BBB", &[(1994, 5.0), (2008, 8.0), (2012, 9.0)]),
        ("CCC", &[(2012, 5.0)]),
    ]);
    // AAA: 2000 (10 per person); BBB: 1994 and 2012 are outside the window, 2008 is inside (10); CCC: only 2012, none.
    assert_eq!(
        income_by_year(&gdp, &pop),
        [("AAA".to_owned(), 10.0), ("BBB".to_owned(), 10.0)].into()
    );
}

fn real() -> Option<(PathBuf, BTreeMap<String, CountryIo>)> {
    let path = cached("1995-2000_SML.zip")?;
    let got = aggregate(&path, 2000).unwrap();
    Some((path, got))
}

#[test]
fn real_2000_economies_coefficients_totals_and_goods() {
    let Some((path, real)) = real() else { return };
    assert_eq!(real.len(), 81);
    assert!(
        ["ROW", "USA", "CHN", "DEU", "TWN", "ZAF"]
            .iter()
            .all(|c| real.contains_key(*c))
    );
    for (code, c) in &real {
        assert!(
            c.io.iter().flatten().all(|x| x.is_finite() && *x >= 0.0),
            "{code}"
        );
        assert!((0..N_GOODS).all(|g| col_sum(&c.io, g) < 1.0), "{code}");
        assert!(
            (c.final_demand_shares.iter().sum::<f64>() - 1.0).abs() < 1e-12,
            "{code}"
        );
        assert!(c.final_demand_shares.iter().all(|s| *s > 0.0), "{code}");
        assert!(
            (0..N_GOODS).all(|g| c.va[g] >= 0.0 && c.output[g] >= c.va[g]),
            "{code}"
        );
        for g in 0..N_GOODS {
            let lhs = c.va[g] + col_sum(&c.io, g) * c.output[g];
            assert!(
                (lhs - c.output[g]).abs() <= 1e-9 + 1e-9 * c.output[g].abs(),
                "{code}"
            );
        }
    }
    // The OUT row of the file, summed over its industry columns (the final demand columns hold zeros there).
    let total: f64 = real.values().map(|c| c.output.iter().sum::<f64>()).sum();
    let file = fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let member = zip.by_name("2000_SML.csv").unwrap();
    let line = BufReader::new(member)
        .lines()
        .map(Result::unwrap)
        .find(|l| l.starts_with("OUT,"))
        .unwrap();
    let row: f64 = line
        .split(',')
        .skip(1)
        .map(|x| x.parse::<f64>().unwrap())
        .sum();
    assert!(approx(total, row, 1e-9));
    assert!((5e7..1e8).contains(&total));
    let (usa, deu) = (&real["USA"], &real["DEU"]);
    assert!((8e6..2.5e7).contains(&usa.output.iter().sum::<f64>()));
    assert!(usa.output[9] > usa.output[..7].iter().sum::<f64>()); // services dominate
    assert!(usa.io[5][5] > 0.05 && usa.io[1][5] > 0.001); // chemicals use chemicals and oil
    assert!(real["SAU"].va[1] > real["SAU"].va[6]); // oil and gas against machinery
    assert!(deu.final_demand_shares[9] > 0.3);
}

#[test]
fn extraction_sums_b06_and_b09_without_refining() {
    // One economy: A01, B06, B09, C19, G and one final demand column.
    let cols = ["AAA_A01", "AAA_B06", "AAA_B09", "AAA_C19", "AAA_G"];
    let goods = [0, 1, 1, 1, 9];
    let n = cols.len();
    let mut rng = Uniform::new(3);
    let z: Vec<Vec<f64>> = (0..n)
        .map(|_| (0..n).map(|_| rng.next(1.0, 10.0)).collect())
        .collect();
    let va: Vec<f64> = (0..n).map(|_| rng.next(10.0, 30.0)).collect();
    let out: Vec<f64> = (0..n)
        .map(|j| (0..n).map(|i| z[i][j]).sum::<f64>() + va[j])
        .collect();
    let join = |xs: &[f64]| {
        xs.iter()
            .map(|x| repr_f64(*x))
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut lines = vec![format!("V1,{},AAA_HFCE,OUT", cols.join(","))];
    for (i, c) in cols.iter().enumerate() {
        lines.push(format!("{c},{},1,0", join(&z[i])));
    }
    for (label, row) in [("TLS", &vec![0.0; n]), ("VA", &va), ("OUT", &out)] {
        lines.push(format!("{label},{},0,0", join(row)));
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("2000_SML.csv");
    fs::write(&path, lines.join("\n") + "\n").unwrap();
    let got = &aggregate(&path, 2000).unwrap()["AAA"];
    assert!(approx(got.extraction_output, out[1] + out[2], 1e-12));
    for g in 0..N_GOODS {
        let want: f64 = (0..n)
            .filter(|&i| goods[i] == g)
            .map(|i| z[i][1] + z[i][2])
            .sum();
        assert!(approx(got.extraction[g], want, 1e-12), "{g}");
    }
}
