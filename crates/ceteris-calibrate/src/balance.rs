//! Stage 4's spending targets balanced against each good's world sales (ADR 0100 as amended by
//! ADR 0295, RFC 0287 D1; `specs/econ/SPEC.md`, Calibration, Stage 1): a biproportional (RAS,
//! Stone 1961) rescale of every country's traded final demand, F'_cg = a_c b_g F_cg, that keeps
//! each country's traded total and gives each good's world net spending sum_c nu_cg F'_cg the
//! same ratio mu to its target C_g.

use ceteris_reference::io::Mat;

/// Largest relative spread of the column ratios col_g / C_g that ends the rounds: the local fixed
/// point's tolerance (`steady::LOCAL_TOL`), a decade below the passes' `PASS_TOL`, so the
/// rescale is settled before the passes judge their change.
pub const RAS_TOL: f64 = 1e-14;
/// The cap on rounds: an iteration cap, not a tolerance.
pub const MAX_RAS: usize = 10_000;

/// The rescale and how it ended.
#[derive(Clone, Debug, PartialEq)]
pub struct Balanced {
    /// (C, G) a_c b_g of traded goods with final demand, 1 elsewhere.
    pub factor: Mat,
    /// (G,) b_g, 1 for goods left out; the warm start of the next pass.
    pub b: Vec<f64>,
    /// (G,) beta_g = sum_c F'_cg / sum_c F_cg, 1 for goods left out: the column factors, the
    /// residual of the old r_g.
    pub column_factor: Vec<f64>,
    /// sum_g col_g / sum_g C_g over the balanced goods: 1 at the calibration's fixed point.
    pub mu: f64,
    pub rounds: usize,
    /// Goods with final demand whose target is not positive, held at their `start` factor and
    /// left out of the balance: a transient of the calibration's passes, an error where they
    /// settle.
    pub held: Vec<usize>,
}

/// Balances `seed` F (C, G), traded final demand by country (0 for local goods), so that every
/// row keeps its sum and every good g with final demand somewhere (`traded[g]` and a positive
/// column) meets sum_c `net`[c, g] F'_cg = mu `target`[g]. `start` is the b of a previous
/// solve. Goods without final demand are left out (factor 1); a good with final demand whose
/// target is not positive is held at its `start` factor (`Balanced::held`). Errors when the
/// rounds run out.
pub fn balance(
    seed: &Mat,
    net: &Mat,
    target: &[f64],
    traded: &[bool],
    start: Option<&[f64]>,
) -> Result<Balanced, String> {
    let (n_c, n_g) = (seed.rows, seed.cols);
    let column: Vec<f64> = (0..n_g)
        .map(|g| (0..n_c).map(|c| seed[(c, g)]).sum())
        .collect();
    let demanded: Vec<usize> = (0..n_g).filter(|&g| traded[g] && column[g] > 0.0).collect();
    let (goods, held): (Vec<usize>, Vec<usize>) = demanded.iter().partition(|&&g| target[g] > 0.0);
    let row: Vec<f64> = (0..n_c)
        .map(|c| demanded.iter().map(|&g| seed[(c, g)]).sum())
        .collect();
    let mut b = vec![1.0; n_g];
    if let Some(s) = start {
        for &g in &demanded {
            b[g] = s[g];
        }
    }
    let world: f64 = goods.iter().map(|&g| column[g]).sum();
    let mut a = vec![1.0; n_c];
    let mut spread = f64::INFINITY;
    for rounds in 1..=MAX_RAS {
        for c in 0..n_c {
            let have: f64 = demanded.iter().map(|&g| b[g] * seed[(c, g)]).sum();
            a[c] = if have > 0.0 { row[c] / have } else { 1.0 };
        }
        let col: Vec<f64> = goods
            .iter()
            .map(|&g| {
                (0..n_c)
                    .map(|c| net[(c, g)] * a[c] * b[g] * seed[(c, g)])
                    .sum()
            })
            .collect();
        let ratio: Vec<f64> = goods
            .iter()
            .zip(&col)
            .map(|(&g, &x)| x / target[g])
            .collect();
        let (lo, hi) = ratio.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), &r| {
            (lo.min(r), hi.max(r))
        });
        spread = hi / lo - 1.0;
        if spread <= RAS_TOL || goods.is_empty() {
            let mu = col.iter().sum::<f64>() / goods.iter().map(|&g| target[g]).sum::<f64>();
            let mut factor = Mat::filled(n_c, n_g, 1.0);
            let mut column_factor = vec![1.0; n_g];
            for &g in &demanded {
                let mut moved = 0.0;
                for c in 0..n_c {
                    factor[(c, g)] = a[c] * b[g];
                    moved += factor[(c, g)] * seed[(c, g)];
                }
                column_factor[g] = moved / column[g];
            }
            return Ok(Balanced {
                factor,
                b,
                column_factor,
                mu: if goods.is_empty() { 1.0 } else { mu },
                rounds,
                held,
            });
        }
        for (&g, &r) in goods.iter().zip(&ratio) {
            b[g] /= r;
        }
        // F' does not see a common scale of b (the rows take it back): hold b near 1
        let level: f64 = goods.iter().map(|&g| b[g] * column[g]).sum::<f64>() / world;
        for &g in &goods {
            b[g] /= level;
        }
    }
    Err(format!(
        "final demand balance did not settle in {MAX_RAS} rounds, column spread {spread:e}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(rows: usize, cols: usize, v: &[f64]) -> Mat {
        Mat {
            rows,
            cols,
            data: v.to_vec(),
        }
    }

    /// Rows keep their sums; the columns meet their targets up to one common factor, which is 1
    /// where the targets add up to the rows (net shares 1).
    #[test]
    fn keeps_rows_and_meets_columns() {
        let seed = mat(3, 3, &[4.0, 1.0, 0.0, 2.0, 2.0, 0.0, 1.0, 5.0, 0.0]);
        let net = Mat::filled(3, 3, 1.0);
        let target = [9.0, 6.0, 0.0];
        let traded = [true, true, true];
        let out = balance(&seed, &net, &target, &traded, None).unwrap();
        for c in 0..3 {
            let before: f64 = (0..3).map(|g| seed[(c, g)]).sum();
            let after: f64 = (0..3).map(|g| out.factor[(c, g)] * seed[(c, g)]).sum();
            assert!((after / before - 1.0).abs() < 1e-14, "row {c}");
        }
        for g in 0..2 {
            let after: f64 = (0..3).map(|c| out.factor[(c, g)] * seed[(c, g)]).sum();
            assert!((after / target[g] - 1.0).abs() < 1e-13, "column {g}");
        }
        assert!((out.mu - 1.0).abs() < 1e-14);
        assert_eq!(out.column_factor[2], 1.0);
        assert!((out.column_factor[0] - 9.0 / 7.0).abs() < 1e-13);
    }

    /// Net shares below 1 weigh the column condition, and inconsistent totals leave a common mu.
    #[test]
    fn net_shares_and_a_common_ratio() {
        let seed = mat(2, 2, &[3.0, 1.0, 1.0, 3.0]);
        let net = mat(2, 2, &[1.0, 0.9, 0.8, 1.0]);
        let target = [2.0, 2.2];
        let out = balance(&seed, &net, &target, &[true, true], None).unwrap();
        let col = |g: usize| -> f64 {
            (0..2)
                .map(|c| net[(c, g)] * out.factor[(c, g)] * seed[(c, g)])
                .sum()
        };
        assert!((col(0) / target[0] - col(1) / target[1]).abs() < 1e-13);
        assert!((col(0) / target[0] - out.mu).abs() < 1e-13);
    }

    /// A good with final demand and no room left for it is held at its start factor.
    #[test]
    fn a_good_with_final_demand_and_no_room_is_held() {
        let seed = mat(2, 2, &[1.0, 1.0, 2.0, 1.0]);
        let net = Mat::filled(2, 2, 1.0);
        let out = balance(&seed, &net, &[2.0, -1.0], &[true, true], Some(&[1.0, 0.5])).unwrap();
        assert_eq!(out.held, vec![1]);
        assert_eq!(out.b[1], 0.5);
    }
}
