//! Fiscal state of one country (P5 Branch C1, spec/09 9.6, ADR 0042, `specs/money/SPEC.md`).
//! Revenue `R_i = tau_i Base_i (1 - informality_i) kappa_i compliance`, capacity kappa built by
//! spending and eroded slowly (Besley and Persson 2009), deficit financed by borrowing. The kernel
//! is not yet in the tick.

use crate::sfc::{
    BANKS, BONDS, DEPOSITS, GOVERNMENT, HOUSEHOLDS, INTEREST, N_INSTRUMENT, N_SECTOR, TAXES,
    TRANSFERS, Transactions,
};

/// Personal income, payroll and social contributions, VAT and sales, corporate, tariffs, excises,
/// resource royalties and state-enterprise profits, seigniorage.
pub const N_REVENUE: usize = 8;
/// Pensions, health, education, social transfers, defence, infrastructure, energy subsidies.
pub const N_SPENDING: usize = 7;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instrument {
    pub rate: f64,
    pub base: i128,
    pub informality: f64,
    pub capacity: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fiscal {
    pub instruments: [Instrument; N_REVENUE],
    pub compliance: f64,
    /// Capacity depreciation per tick.
    pub depreciation: f64,
    /// Capacity gain per unit of capacity spending as a share of GDP.
    pub build_rate: f64,
    pub debt: i128,
}

impl Fiscal {
    pub fn revenue(&self, i: usize) -> i128 {
        let x = &self.instruments[i];
        let r = x.rate * (x.base as f64) * (1.0 - x.informality) * x.capacity * self.compliance;
        r.round() as i128
    }

    pub fn total_revenue(&self) -> i128 {
        (0..N_REVENUE).map(|i| self.revenue(i)).sum()
    }

    /// Capacity of each instrument rises with spending on it and decays: `k' = k(1 - d) + g s (1 - k)`.
    pub fn update_capacity(&mut self, capacity_spend_share: &[f64; N_REVENUE]) {
        for (x, s) in self.instruments.iter_mut().zip(capacity_spend_share) {
            x.capacity = (x.capacity * (1.0 - self.depreciation)
                + self.build_rate * s * (1.0 - x.capacity))
                .clamp(0.0, 1.0);
        }
    }

    /// Deficit `= spending + interest - revenue`, financed by borrowing.
    pub fn deficit(&self, spending: &[i128; N_SPENDING], interest: i128) -> i128 {
        spending.iter().sum::<i128>() + interest - self.total_revenue()
    }

    /// Close one tick in the government account and post it to the SFC matrices: taxes paid by
    /// households, spending and interest paid to households, the deficit sold as bonds to banks.
    /// Returns the debt-financing acquisitions and the flows.
    pub fn close_tick(
        &mut self,
        spending: &[i128; N_SPENDING],
        interest: i128,
    ) -> (Transactions, [[i128; N_SECTOR]; N_INSTRUMENT]) {
        let revenue = self.total_revenue();
        let spend: i128 = spending.iter().sum();
        let deficit = spend + interest - revenue;
        let mut tx = Transactions::default();
        tx.post(TAXES, HOUSEHOLDS, GOVERNMENT, revenue);
        tx.post(TRANSFERS, GOVERNMENT, HOUSEHOLDS, spend);
        tx.post(INTEREST, GOVERNMENT, HOUSEHOLDS, interest);
        let mut acq = [[0i128; N_SECTOR]; N_INSTRUMENT];
        acq[BONDS][GOVERNMENT] -= deficit;
        acq[BONDS][BANKS] += deficit;
        acq[DEPOSITS][HOUSEHOLDS] += deficit;
        acq[DEPOSITS][BANKS] -= deficit;
        self.debt += deficit;
        (tx, acq)
    }
}

/// Public capital of one type (core infrastructure or other), ADR 0042. Productivity factor
/// `A / A_cal = (K_g / K_g0)^P173`; investment is scaled by an efficiency factor (P174).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PublicCapital {
    pub stock: f64,
    pub stock0: f64,
    /// P173; zero leaves productivity at its calibrated level.
    pub elasticity: f64,
    pub depreciation: f64,
}

impl PublicCapital {
    pub fn productivity_factor(&self) -> f64 {
        if self.elasticity == 0.0 {
            return 1.0;
        }
        libm::pow(self.stock / self.stock0, self.elasticity)
    }

    /// `K' = K (1 - d) + efficiency * investment`, with efficiency from P174 in `[0, 1]`.
    pub fn invest(&mut self, investment: f64, efficiency: f64) {
        self.stock = self.stock * (1.0 - self.depreciation) + efficiency * investment;
    }
}

/// Debt service minus growth: `(r - g)` reported per state.
pub fn r_minus_g(interest_rate: f64, growth: f64) -> f64 {
    interest_rate - growth
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfc::{BalanceSheets, closes};

    fn fiscal() -> Fiscal {
        let x = |rate, base: i128, informality, capacity| Instrument {
            rate,
            base,
            informality,
            capacity,
        };
        let mut instruments = [x(0.0, 0, 0.0, 0.0); N_REVENUE];
        instruments[0] = x(0.20, 1_000_000_000_000, 0.3, 0.6);
        instruments[2] = x(0.15, 2_000_000_000_000, 0.2, 0.8);
        instruments[4] = x(0.05, 500_000_000_000, 0.0, 1.0);
        Fiscal {
            instruments,
            compliance: 0.9,
            depreciation: 0.001,
            build_rate: 0.5,
            debt: 0,
        }
    }

    #[test]
    fn informality_shrinks_the_base() {
        let mut f = fiscal();
        let r0 = f.revenue(0);
        f.instruments[0].informality = 0.5;
        assert!(f.revenue(0) < r0);
        f.instruments[0].informality = 1.0;
        assert_eq!(f.revenue(0), 0);
    }

    #[test]
    fn capacity_builds_with_spending_and_decays_without() {
        let mut f = fiscal();
        let k = f.instruments[0].capacity;
        let mut s = [0.0; N_REVENUE];
        f.update_capacity(&s);
        assert!(f.instruments[0].capacity < k);
        s[0] = 0.02;
        for _ in 0..200 {
            f.update_capacity(&s);
        }
        assert!(f.instruments[0].capacity > k && f.instruments[0].capacity <= 1.0);
    }

    #[test]
    fn revenue_and_spending_close_in_the_government_account() {
        let mut f = fiscal();
        let mut sheets = BalanceSheets::default();
        for t in 0..50i128 {
            let spending = [3, 1, 2, 4, 2, 1, 0].map(|v| v * 40_000_000_000 + t * 1_000);
            let interest = 10_000_000_000 + f.debt / 100;
            let expect = f.deficit(&spending, interest);
            let before = sheets;
            let debt = f.debt;
            let (tx, acq) = f.close_tick(&spending, interest);
            assert_eq!(f.debt - debt, expect);
            let mut after = before;
            after.apply(&acq, &[[0; N_SECTOR]; N_INSTRUMENT]);
            assert!(closes(
                &before,
                &after,
                &tx,
                &acq,
                &[[0; N_SECTOR]; N_INSTRUMENT]
            ));
            assert_eq!(after.positions[BONDS][GOVERNMENT], -f.debt);
            sheets = after;
        }
    }

    #[test]
    fn public_capital_elasticity_zero_equals_status_quo() {
        let mut k = PublicCapital {
            stock: 100.0,
            stock0: 100.0,
            elasticity: 0.0,
            depreciation: 0.01,
        };
        for _ in 0..50 {
            k.invest(5.0, 0.7);
        }
        assert_eq!(k.productivity_factor(), 1.0);
    }

    #[test]
    fn public_capital_raises_productivity_and_efficiency_scales_investment() {
        let k0 = PublicCapital {
            stock: 100.0,
            stock0: 100.0,
            elasticity: 0.08,
            depreciation: 0.0,
        };
        assert_eq!(k0.productivity_factor(), 1.0);
        let (mut lo, mut hi) = (k0, k0);
        lo.invest(10.0, 0.5);
        hi.invest(10.0, 1.0);
        assert!(lo.productivity_factor() > 1.0);
        assert!(hi.productivity_factor() > lo.productivity_factor());
        assert!((lo.stock - 105.0).abs() < 1e-12);
        let doubled = PublicCapital { stock: 200.0, ..k0 };
        assert!((doubled.productivity_factor() - libm::pow(2.0, 0.08)).abs() < 1e-12);
    }
}
