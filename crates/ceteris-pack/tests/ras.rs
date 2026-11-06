//! Biproportional (RAS) balancing of a donor's table to an economy's own row and column totals
//! (ADRs 0098, 0286).

use ceteris_pack::econ::ras::balance;
use ceteris_pack::read::icio::N_GOODS;

type Table = [[f64; N_GOODS]; N_GOODS];

/// A dense seed with a few structural zeros.
fn seed() -> Table {
    std::array::from_fn(|k| {
        std::array::from_fn(|g| {
            if (k + 2 * g) % 7 == 3 {
                0.0
            } else {
                1.0 + ((k * 31 + g * 17) % 13) as f64
            }
        })
    })
}

fn sums(t: &Table) -> ([f64; N_GOODS], [f64; N_GOODS]) {
    let rows = std::array::from_fn(|k| t[k].iter().sum());
    let cols = std::array::from_fn(|g| t.iter().map(|r| r[g]).sum());
    (rows, cols)
}

/// The seed scaled unevenly by row and column, so a solution with its totals exists.
fn scaled() -> Table {
    let s = seed();
    let r: [f64; N_GOODS] = std::array::from_fn(|k| 0.5 + k as f64 * 0.3);
    let c: [f64; N_GOODS] = std::array::from_fn(|g| 2.0 - g as f64 * 0.15);
    std::array::from_fn(|k| std::array::from_fn(|g| r[k] * s[k][g] * c[g]))
}

fn targets() -> ([f64; N_GOODS], [f64; N_GOODS]) {
    sums(&scaled())
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

#[test]
fn ras_matches_targets() {
    let (rows, cols) = targets();
    let got = balance(&seed(), &rows, &cols, 1e-10, 200).unwrap().table;
    let (r, c) = sums(&got);
    for i in 0..N_GOODS {
        assert!(
            rel(r[i], rows[i]) <= 1e-10,
            "row {i}: {} vs {}",
            r[i],
            rows[i]
        );
        assert!(
            rel(c[i], cols[i]) <= 1e-10,
            "col {i}: {} vs {}",
            c[i],
            cols[i]
        );
    }
    // RAS keeps the seed's zeros and signs.
    let s = seed();
    for k in 0..N_GOODS {
        for g in 0..N_GOODS {
            assert_eq!(s[k][g] == 0.0, got[k][g] == 0.0);
            assert!(got[k][g] >= 0.0);
        }
    }
}

#[test]
fn ras_returns_the_seed_when_it_already_matches() {
    let s = seed();
    let (rows, cols) = sums(&s);
    let got = balance(&s, &rows, &cols, 1e-10, 200).unwrap().table;
    for k in 0..N_GOODS {
        for g in 0..N_GOODS {
            assert!(rel(got[k][g], s[k][g]) <= 1e-12 || s[k][g] == 0.0);
        }
    }
}

#[test]
fn ras_zero_target_empties_its_row() {
    let mut t = scaled();
    t[4] = [0.0; N_GOODS];
    let (rows, cols) = sums(&t);
    let got = balance(&seed(), &rows, &cols, 1e-10, 200).unwrap().table;
    assert!(got[4].iter().all(|&x| x == 0.0));
    let (_, c) = sums(&got);
    assert!((0..N_GOODS).all(|g| rel(c[g], cols[g]) <= 1e-10));
}

#[test]
fn ras_refuses_inconsistent_totals() {
    let (rows, mut cols) = targets();
    cols[0] *= 1.01;
    assert!(balance(&seed(), &rows, &cols, 1e-10, 200).is_err());
}

#[test]
fn ras_refuses_a_target_on_an_empty_row() {
    let mut s = seed();
    s[2] = [0.0; N_GOODS];
    let (rows, cols) = targets();
    assert!(balance(&s, &rows, &cols, 1e-10, 200).is_err());
}

#[test]
fn ras_reports_no_convergence_within_the_cap() {
    let (rows, cols) = targets();
    let err = balance(&seed(), &rows, &cols, 1e-10, 1).unwrap_err();
    assert!(err.contains("iterations"), "{err}");
}

/// A use table [Z | F]: more columns than rows, scaled unevenly so a solution exists.
#[test]
fn ras_balances_a_rectangular_table() {
    let seed: [[f64; 4]; 3] = [
        [1.0, 2.0, 0.0, 4.0],
        [3.0, 1.0, 2.0, 1.0],
        [0.5, 0.0, 1.0, 6.0],
    ];
    let (r, c) = ([2.0, 0.5, 1.5], [1.0, 3.0, 0.7, 1.2]);
    let want: [[f64; 4]; 3] =
        std::array::from_fn(|k| std::array::from_fn(|g| r[k] * seed[k][g] * c[g]));
    let rows: [f64; 3] = std::array::from_fn(|k| want[k].iter().sum());
    let cols: [f64; 4] = std::array::from_fn(|g| want.iter().map(|w| w[g]).sum());
    let got = balance(&seed, &rows, &cols, 1e-10, 200).unwrap();
    assert!(got.passes >= 1 && got.passes <= 200);
    for ((row, w), total) in got.table.iter().zip(&want).zip(rows) {
        assert!(rel(row.iter().sum(), total) <= 1e-10);
        for (x, y) in row.iter().zip(w) {
            // The biproportional solution is unique: the seed scaled by r and c.
            assert!(rel(*x, *y) <= 1e-8 || *y == 0.0);
        }
    }
    for (g, total) in cols.iter().enumerate() {
        assert!(rel(got.table.iter().map(|t| t[g]).sum(), *total) <= 1e-10);
    }
    assert!(balance(&seed, &rows[..2], &cols, 1e-10, 200).is_err());
}
