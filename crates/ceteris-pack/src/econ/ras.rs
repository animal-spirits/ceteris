//! Biproportional (RAS) balancing of a table to row and column totals (Stone 1961; ADR 0098 as
//! amended by ADR 0286, the use tables of the economies outside ICIO).

fn row_sums<const R: usize, const C: usize>(t: &[[f64; C]; R]) -> [f64; R] {
    std::array::from_fn(|k| t[k].iter().sum())
}

fn col_sums<const R: usize, const C: usize>(t: &[[f64; C]; R]) -> [f64; C] {
    std::array::from_fn(|g| t.iter().map(|r| r[g]).sum())
}

/// Largest relative miss of `got` against `want`; a zero target counts its absolute miss.
pub fn worst(got: &[f64], want: &[f64]) -> f64 {
    got.iter()
        .zip(want)
        .map(|(g, w)| if *w > 0.0 { (g - w).abs() / w } else { g.abs() })
        .fold(0.0, f64::max)
}

/// A balanced table and the passes it took.
#[derive(Clone, Debug, PartialEq)]
pub struct Balanced<const R: usize, const C: usize> {
    pub table: [[f64; C]; R],
    pub passes: usize,
}

/// `seed` scaled by rows and columns, x_kg = r_k seed_kg s_g, so that its row sums are `rows`
/// and its column sums `cols`, each within `tol` relative, in at most `max_iter` passes of a row
/// then a column scaling. Zeros of the seed stay zero. Errors on negative or non-finite inputs,
/// on totals that disagree by more than `tol`, on a positive target for a row or column the seed
/// leaves empty, and when the passes run out.
pub fn balance<const R: usize, const C: usize>(
    seed: &[[f64; C]; R],
    rows: &[f64],
    cols: &[f64],
    tol: f64,
    max_iter: usize,
) -> Result<Balanced<R, C>, String> {
    if rows.len() != R || cols.len() != C {
        return Err(format!("RAS needs {R} row and {C} column totals"));
    }
    let valid = |x: &f64| x.is_finite() && *x >= 0.0;
    if !(seed.iter().flatten().all(valid) && rows.iter().all(valid) && cols.iter().all(valid)) {
        return Err("RAS inputs must be finite and non-negative".into());
    }
    let (total_rows, total_cols): (f64, f64) = (rows.iter().sum(), cols.iter().sum());
    if (total_rows - total_cols).abs() > tol * total_rows.max(total_cols) {
        return Err(format!(
            "RAS totals disagree: rows {total_rows}, columns {total_cols}"
        ));
    }
    let mut x = *seed;
    let (have_rows, have_cols) = (row_sums(&x), col_sums(&x));
    for i in 0..R {
        if rows[i] > 0.0 && have_rows[i] == 0.0 {
            return Err(format!(
                "RAS row {i} is empty in the seed, target {}",
                rows[i]
            ));
        }
    }
    for g in 0..C {
        if cols[g] > 0.0 && have_cols[g] == 0.0 {
            return Err(format!(
                "RAS column {g} is empty in the seed, target {}",
                cols[g]
            ));
        }
    }
    for pass in 1..=max_iter {
        let have = row_sums(&x);
        for (k, row) in x.iter_mut().enumerate() {
            let f = if have[k] > 0.0 {
                rows[k] / have[k]
            } else {
                0.0
            };
            row.iter_mut().for_each(|v| *v *= f);
        }
        let have = col_sums(&x);
        for row in &mut x {
            for (g, v) in row.iter_mut().enumerate() {
                *v *= if have[g] > 0.0 {
                    cols[g] / have[g]
                } else {
                    0.0
                };
            }
        }
        if worst(&row_sums(&x), rows) <= tol && worst(&col_sums(&x), cols) <= tol {
            return Ok(Balanced {
                table: x,
                passes: pass,
            });
        }
    }
    Err(format!(
        "RAS did not reach {tol} in {max_iter} iterations: rows {:e}, columns {:e}",
        worst(&row_sums(&x), rows),
        worst(&col_sums(&x), cols)
    ))
}
