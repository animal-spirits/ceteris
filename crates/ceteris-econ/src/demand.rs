//! CES final demand.

use ceteris_trade::N_GOODS;

/// x_g = Omega_g p_g^-sigma m / sum_k Omega_k p_k^(1 - sigma). Goods with Omega_g = 0 get 0,
/// and so do goods at an infinite price (no reachable origin): the money goes to the rest. A
/// nonpositive balance buys nothing.
pub fn ces(
    omega: &[f64; N_GOODS],
    price: &[f64; N_GOODS],
    money: f64,
    sigma: f64,
    out: &mut [f64; N_GOODS],
) {
    *out = [0.0; N_GOODS];
    if money <= 0.0 {
        return;
    }
    let buys = |g: usize| omega[g] > 0.0 && price[g].is_finite();
    let mut denom = 0.0;
    for g in (0..N_GOODS).filter(|&g| buys(g)) {
        denom += omega[g] * libm::pow(price[g], 1.0 - sigma);
    }
    if denom <= 0.0 {
        return;
    }
    let scale = money / denom;
    for g in (0..N_GOODS).filter(|&g| buys(g)) {
        out[g] = omega[g] * libm::pow(price[g], -sigma) * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OMEGA: [f64; N_GOODS] = [0.1, 0.0, 0.2, 0.05, 0.15, 0.0, 0.2, 0.1, 0.1, 0.1];

    #[test]
    fn ces_spends_all_money_and_matches_shares_at_unit_prices() {
        let mut out = [0.0; N_GOODS];
        let ones = [1.0; N_GOODS];
        ces(&OMEGA, &ones, 200.0, 0.6, &mut out);
        let total: f64 = OMEGA.iter().sum();
        for g in 0..N_GOODS {
            assert!(
                (out[g] - 200.0 * OMEGA[g] / total).abs() < 1e-12,
                "good {g}"
            );
        }
        assert_eq!(out[1], 0.0);
        assert_eq!(out[5], 0.0);

        let prices = [1.0, 3.0, 0.7, 2.2, 1.4, 5.0, 0.9, 1.1, 2.0, 0.5];
        for sigma in [0.3, 0.6, 1.0, 1.5] {
            ces(&OMEGA, &prices, 123.0, sigma, &mut out);
            let spent: f64 = (0..N_GOODS).map(|g| prices[g] * out[g]).sum();
            assert!((spent - 123.0).abs() < 1e-10, "sigma {sigma}: {spent}");
        }
    }

    #[test]
    fn ces_spends_the_rest_when_a_good_is_unreachable() {
        let mut prices = [1.0, 3.0, 0.7, 2.2, 1.4, 5.0, 0.9, 1.1, 2.0, 0.5];
        prices[4] = f64::INFINITY;
        let mut out = [0.0; N_GOODS];
        let mut without = OMEGA;
        without[4] = 0.0;
        let mut want = [0.0; N_GOODS];
        for sigma in [0.6, 1.5] {
            ces(&OMEGA, &prices, 80.0, sigma, &mut out);
            ces(&without, &prices, 80.0, sigma, &mut want);
            assert_eq!(out, want);
            assert_eq!(out[4], 0.0);
            let spent: f64 = (0..N_GOODS)
                .filter(|&g| g != 4)
                .map(|g| prices[g] * out[g])
                .sum();
            assert!((spent - 80.0).abs() < 1e-10, "sigma {sigma}: {spent}");
        }
    }

    #[test]
    fn ces_buys_nothing_without_money() {
        let mut out = [1.0; N_GOODS];
        ces(&OMEGA, &[1.0; N_GOODS], 0.0, 0.6, &mut out);
        assert_eq!(out, [0.0; N_GOODS]);
        ces(&OMEGA, &[1.0; N_GOODS], -5.0, 0.6, &mut out);
        assert_eq!(out, [0.0; N_GOODS]);
    }
}
