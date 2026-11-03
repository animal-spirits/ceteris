//! Dividend shares by wealth (specs/econ/SPEC.md, Income; ADR 0104 as accepted by RFC 0302 D2):
//! a country's profit pool is paid by the WID wealth shares of its bottom 50%, middle 40% and top
//! 10%, ranked by income with perfect rank correlation, so a pop's share is the wealth share of
//! each bracket times the part of the bracket's population that is the pop's.

/// Population fractions of the wealth brackets, bottom 50%, middle 40%, top 10%.
pub const BRACKETS: [f64; 3] = [0.5, 0.4, 0.1];

/// Bisection steps for an income quantile: the bracket of 80 log units halves to below 1e-20.
const STEPS: usize = 100;

fn phi(z: f64) -> f64 {
    0.5 * libm::erfc(-z / std::f64::consts::SQRT_2)
}

/// Each pop's share of its country's profit pool, summing to 1 over pops with people and
/// income. `wealth` are the brackets' shares of wealth (each at least 0, summing to 1), `pops`
/// each pop's (population, mean income per person), `sigma` the within-pop log dispersion of
/// incomes: the country's income distribution is the mixture of the pops' lognormals (point
/// masses where `sigma` is 0, ranked by income and then by pop order). A pop without people or
/// income gets 0.
pub fn dividend_shares(wealth: &[f64; 3], pops: &[(f64, f64)], sigma: f64) -> Vec<f64> {
    let counted: Vec<usize> = (0..pops.len())
        .filter(|&k| pops[k].0 > 0.0 && pops[k].1 > 0.0)
        .collect();
    let mut out = vec![0.0; pops.len()];
    if counted.is_empty() {
        return out;
    }
    let mass = if sigma > 0.0 {
        continuous_masses(pops, &counted, sigma)
    } else {
        point_masses(pops, &counted)
    };
    let mut total = [0.0; 3];
    for m in &mass {
        for b in 0..3 {
            total[b] += m[b];
        }
    }
    for (&k, m) in counted.iter().zip(&mass) {
        out[k] = (0..3)
            .filter(|&b| total[b] > 0.0)
            .map(|b| wealth[b] * m[b] / total[b])
            .sum();
    }
    out
}

/// Population of each counted pop in each bracket, its lognormal split at the quantiles 0.5 and
/// 0.9 of the mixture.
fn continuous_masses(pops: &[(f64, f64)], counted: &[usize], sigma: f64) -> Vec<[f64; 3]> {
    let mu: Vec<f64> = counted
        .iter()
        .map(|&k| libm::log(pops[k].1) - 0.5 * sigma * sigma)
        .collect();
    let n: Vec<f64> = counted.iter().map(|&k| pops[k].0).collect();
    let people: f64 = n.iter().sum();
    let below = |t: f64| -> f64 {
        mu.iter()
            .zip(&n)
            .map(|(&m, &n)| n * phi((t - m) / sigma))
            .sum::<f64>()
    };
    let lo = mu.iter().copied().fold(f64::INFINITY, f64::min) - 40.0 * sigma;
    let hi = mu.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 40.0 * sigma;
    let quantile = |q: f64| {
        let (mut a, mut b) = (lo, hi);
        for _ in 0..STEPS {
            let mid = 0.5 * (a + b);
            if below(mid) < q * people {
                a = mid;
            } else {
                b = mid;
            }
        }
        0.5 * (a + b)
    };
    let (t1, t2) = (quantile(BRACKETS[0]), quantile(BRACKETS[0] + BRACKETS[1]));
    mu.iter()
        .zip(&n)
        .map(|(&m, &n)| {
            let (f1, f2) = (phi((t1 - m) / sigma), phi((t2 - m) / sigma));
            [n * f1, n * (f2 - f1), n * (1.0 - f2)]
        })
        .collect()
}

/// Population of each counted pop in each bracket where every person of a pop has its mean
/// income: pops in order of income (ties by pop order) fill the ranks.
fn point_masses(pops: &[(f64, f64)], counted: &[usize]) -> Vec<[f64; 3]> {
    let mut order: Vec<usize> = (0..counted.len()).collect();
    order.sort_by(|&a, &b| {
        pops[counted[a]]
            .1
            .total_cmp(&pops[counted[b]].1)
            .then(a.cmp(&b))
    });
    let people: f64 = counted.iter().map(|&k| pops[k].0).sum();
    let edges = [
        0.0,
        BRACKETS[0] * people,
        (BRACKETS[0] + BRACKETS[1]) * people,
        people,
    ];
    let mut out = vec![[0.0; 3]; counted.len()];
    let mut start = 0.0;
    for &j in &order {
        let end = start + pops[counted[j]].0;
        for b in 0..3 {
            out[j][b] = (end.min(edges[b + 1]) - start.max(edges[b])).max(0.0);
        }
        start = end;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_wealth_pays_by_population() {
        let pops = [(1.0, 1.0), (3.0, 2.0), (2.0, 5.0)];
        for sigma in [0.0, 0.7] {
            let d = dividend_shares(&BRACKETS, &pops, sigma);
            for (k, &(n, _)) in pops.iter().enumerate() {
                assert!(
                    (d[k] - n / 6.0).abs() < 1e-12,
                    "sigma {sigma} pop {k}: {}",
                    d[k]
                );
            }
        }
    }

    #[test]
    fn point_masses_fill_the_ranks_in_income_order() {
        // the rich pop of 1 person is the top 10% of 10; the poor 5 the bottom half
        let pops = [(4.0, 2.0), (5.0, 1.0), (1.0, 9.0), (0.0, 3.0)];
        let d = dividend_shares(&[0.1, 0.3, 0.6], &pops, 0.0);
        assert!((d[1] - 0.1).abs() < 1e-15);
        assert!((d[0] - 0.3).abs() < 1e-15);
        assert!((d[2] - 0.6).abs() < 1e-15);
        assert_eq!(d[3], 0.0);
    }
}
