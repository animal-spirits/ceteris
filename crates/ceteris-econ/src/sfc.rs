//! Sector balance sheets and the stock-flow-consistent matrices (P5 Branch A1, spec/09 9.1,
//! `specs/money/SPEC.md`). Positions are signed `i128` minor units: assets positive, liabilities
//! negative, so every instrument row sums to zero across sectors. The kernel is not yet in the tick.

/// Households, enterprises, banks, government, central bank, rest of world.
pub const N_SECTOR: usize = 6;
pub const HOUSEHOLDS: usize = 0;
pub const ENTERPRISES: usize = 1;
pub const BANKS: usize = 2;
pub const GOVERNMENT: usize = 3;
pub const CENTRAL_BANK: usize = 4;
pub const ROW: usize = 5;

/// Cash and reserves, deposits, loans, bonds, equity, official FX reserves.
pub const N_INSTRUMENT: usize = 6;
pub const CURRENCY: usize = 0;
pub const DEPOSITS: usize = 1;
pub const LOANS: usize = 2;
pub const BONDS: usize = 3;
pub const EQUITY: usize = 4;
pub const FX_RESERVES: usize = 5;

/// Current-account flows of the transactions-flow matrix.
pub const N_FLOW: usize = 8;
pub const CONSUMPTION: usize = 0;
pub const WAGES: usize = 1;
pub const TAXES: usize = 2;
pub const TRANSFERS: usize = 3;
pub const INTEREST: usize = 4;
pub const DIVIDENDS: usize = 5;
pub const INVESTMENT: usize = 6;
pub const NET_EXPORTS: usize = 7;

/// One agent-level claim: the holder's asset and the issuer's liability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claim {
    pub instrument: usize,
    pub holder: usize,
    pub issuer: usize,
    pub amount: i128,
}

/// Balance-sheet matrix, `positions[instrument][sector]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BalanceSheets {
    pub positions: [[i128; N_SECTOR]; N_INSTRUMENT],
}

impl BalanceSheets {
    /// Aggregate agent claims into the sector matrix, in slice order.
    pub fn from_claims(claims: &[Claim]) -> Self {
        let mut b = Self::default();
        for c in claims {
            b.positions[c.instrument][c.holder] += c.amount;
            b.positions[c.instrument][c.issuer] -= c.amount;
        }
        b
    }

    /// Net financial worth of a sector (column sum).
    pub fn net_financial(&self, sector: usize) -> i128 {
        self.positions.iter().map(|row| row[sector]).sum()
    }

    /// Every instrument row sums to zero: each asset is someone's liability.
    pub fn rows_close(&self) -> bool {
        self.positions
            .iter()
            .all(|row| row.iter().sum::<i128>() == 0)
    }

    /// Add acquisitions of financial assets; revaluations are passed as a second matrix.
    pub fn apply(
        &mut self,
        acquired: &[[i128; N_SECTOR]; N_INSTRUMENT],
        reval: &[[i128; N_SECTOR]; N_INSTRUMENT],
    ) {
        for i in 0..N_INSTRUMENT {
            for s in 0..N_SECTOR {
                self.positions[i][s] += acquired[i][s] + reval[i][s];
            }
        }
    }
}

/// Transactions-flow matrix, `flows[flow][sector]`: receipts positive, payments negative.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Transactions {
    pub flows: [[i128; N_SECTOR]; N_FLOW],
}

impl Transactions {
    /// Post a payment of `amount` from `payer` to `payee` under `flow`.
    pub fn post(&mut self, flow: usize, payer: usize, payee: usize, amount: i128) {
        self.flows[flow][payer] -= amount;
        self.flows[flow][payee] += amount;
    }

    /// Every flow row sums to zero across sectors.
    pub fn rows_close(&self) -> bool {
        self.flows.iter().all(|row| row.iter().sum::<i128>() == 0)
    }

    /// Net lending of a sector: the sum of its flow column.
    pub fn net_lending(&self, sector: usize) -> i128 {
        self.flows.iter().map(|row| row[sector]).sum()
    }

    /// Every sector's net lending equals the net financial assets it acquired.
    pub fn columns_close(&self, acquired: &[[i128; N_SECTOR]; N_INSTRUMENT]) -> bool {
        (0..N_SECTOR)
            .all(|s| self.net_lending(s) == acquired.iter().map(|row| row[s]).sum::<i128>())
    }
}

/// Both matrices close for one tick: flow rows, instrument rows before and after, and columns.
pub fn closes(
    before: &BalanceSheets,
    after: &BalanceSheets,
    tx: &Transactions,
    acquired: &[[i128; N_SECTOR]; N_INSTRUMENT],
    reval: &[[i128; N_SECTOR]; N_INSTRUMENT],
) -> bool {
    let mut expect = *before;
    expect.apply(acquired, reval);
    expect == *after && after.rows_close() && tx.rows_close() && tx.columns_close(acquired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(s: &mut u64) -> u64 {
        *s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *s >> 33
    }

    /// Random agent claims over 40 agents mapped onto sectors.
    fn claims(seed: u64, n: usize) -> Vec<Claim> {
        let mut s = seed;
        (0..n)
            .map(|_| Claim {
                instrument: lcg(&mut s) as usize % N_INSTRUMENT,
                holder: lcg(&mut s) as usize % N_SECTOR,
                issuer: lcg(&mut s) as usize % N_SECTOR,
                amount: (lcg(&mut s) % 1_000_000) as i128 * 1_000_000_000_000,
            })
            .collect()
    }

    #[test]
    fn agent_claims_aggregate_to_sector_matrices_exactly() {
        let cs = claims(7, 500);
        let b = BalanceSheets::from_claims(&cs);
        assert!(b.rows_close());
        for i in 0..N_INSTRUMENT {
            for s in 0..N_SECTOR {
                let assets: i128 = cs
                    .iter()
                    .filter(|c| c.instrument == i && c.holder == s)
                    .map(|c| c.amount)
                    .sum();
                let liabs: i128 = cs
                    .iter()
                    .filter(|c| c.instrument == i && c.issuer == s)
                    .map(|c| c.amount)
                    .sum();
                assert_eq!(b.positions[i][s], assets - liabs);
            }
        }
        let rev: Vec<Claim> = cs.iter().rev().copied().collect();
        assert_eq!(BalanceSheets::from_claims(&rev), b);
    }

    #[test]
    fn matrices_sum_to_zero_across_rows_and_columns_every_tick() {
        let mut b = BalanceSheets::from_claims(&claims(3, 200));
        let mut s = 11u64;
        for _ in 0..100 {
            let before = b;
            let mut tx = Transactions::default();
            let mut acquired = [[0i128; N_SECTOR]; N_INSTRUMENT];
            for _ in 0..20 {
                let f = lcg(&mut s) as usize % N_FLOW;
                let (p, q) = (
                    lcg(&mut s) as usize % N_SECTOR,
                    lcg(&mut s) as usize % N_SECTOR,
                );
                let a = (lcg(&mut s) % 10_000) as i128 * 1_000_000_000;
                tx.post(f, p, q, a);
                // Payment settles in deposits: payee gains, payer loses; banks are the counterparty.
                acquired[DEPOSITS][q] += a;
                acquired[DEPOSITS][p] -= a;
            }
            // Net-lending sectors lend to the rest: the deposit rows net to zero by construction.
            let mut reval = [[0i128; N_SECTOR]; N_INSTRUMENT];
            let r = (lcg(&mut s) % 1000) as i128;
            reval[BONDS][GOVERNMENT] = -r;
            reval[BONDS][CENTRAL_BANK] = r;
            let mut after = before;
            after.apply(&acquired, &reval);
            assert!(closes(&before, &after, &tx, &acquired, &reval));
            b = after;
        }
    }

    #[test]
    fn a_leak_is_detected() {
        let mut tx = Transactions::default();
        tx.post(WAGES, ENTERPRISES, HOUSEHOLDS, 100);
        tx.flows[WAGES][HOUSEHOLDS] += 1;
        assert!(!tx.rows_close());
        let mut acquired = [[0i128; N_SECTOR]; N_INSTRUMENT];
        let mut ok = Transactions::default();
        ok.post(WAGES, ENTERPRISES, HOUSEHOLDS, 100);
        acquired[DEPOSITS][HOUSEHOLDS] = 100;
        acquired[DEPOSITS][ENTERPRISES] = -100;
        assert!(ok.columns_close(&acquired));
        acquired[DEPOSITS][BANKS] = 5;
        assert!(!ok.columns_close(&acquired));
    }
}
