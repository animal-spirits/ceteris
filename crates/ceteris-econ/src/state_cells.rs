//! State energy cells (P5 Branch C4, ADR 0278 applying ADR 0201's ownership split to crude oil and
//! gas extraction and power only). A (province, sector) cell splits where the state holds at least
//! 5% of its 2000 value added; state cells' profits go to the government account instead of pops.
//! In the tick inside `distribute`, off by default (`EconWorld::with_state_cells`).

use ceteris_core::snapshot::{DecodeError, Decoder, Encoder};

use crate::sfc::{DIVIDENDS, ENTERPRISES, GOVERNMENT, HOUSEHOLDS, Transactions};

/// Opens the state cells section of a snapshot, which follows the energy policy one.
pub const SNAPSHOT_TAG: u32 = 0x3A6E_6F03;

/// The state's share of each enterprise's cell in 2000 value added, one per enterprise; below
/// [`SPLIT_THRESHOLD`] the enterprise stays private.
#[derive(Clone, Debug, PartialEq)]
pub struct StateCells {
    pub share: Vec<f64>,
}

impl StateCells {
    /// The part of enterprise `e`'s positive payout `paid` (minor units) that is the state cell's.
    pub fn state_part(&self, e: usize, paid: i64) -> i64 {
        split_cell(i128::from(paid), self.share[e], true).state as i64
    }

    pub fn encode(&self, e: &mut Encoder) {
        e.u32(SNAPSHOT_TAG);
        e.count(self.share.len());
        for &s in &self.share {
            e.f64(s);
        }
    }

    pub fn decode(d: &mut Decoder) -> Result<Self, DecodeError> {
        if d.u32()? != SNAPSHOT_TAG {
            return Err(DecodeError("state cells section tag".into()));
        }
        let n = d.count()?;
        let share = (0..n).map(|_| d.f64()).collect::<Result<_, _>>()?;
        Ok(StateCells { share })
    }
}

/// ADR 0201: a cell splits only where the state holds at least this share of (province, sector) VA.
pub const SPLIT_THRESHOLD: f64 = 0.05;

/// Result of splitting one (province, sector) cell; amounts in money units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellSplit {
    pub private: i128,
    pub state: i128,
}

/// Split value added between the private and state cells. Below the threshold, or with
/// `enabled` false, there is one private cell and the status quo is untouched.
pub fn split_cell(value_added: i128, state_share: f64, enabled: bool) -> CellSplit {
    if !enabled || state_share < SPLIT_THRESHOLD {
        return CellSplit {
            private: value_added,
            state: 0,
        };
    }
    let state = ((value_added as f64) * state_share.min(1.0)).round() as i128;
    CellSplit {
        private: value_added - state,
        state,
    }
}

/// Post one tick of profits: private-cell profits are dividends to households, state-cell profits
/// dividends to the government.
pub fn post_profits(tx: &mut Transactions, private_profit: i128, state_profit: i128) {
    tx.post(DIVIDENDS, ENTERPRISES, HOUSEHOLDS, private_profit);
    tx.post(DIVIDENDS, ENTERPRISES, GOVERNMENT, state_profit);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfc::N_SECTOR;

    #[test]
    fn state_cell_split_off_equals_status_quo() {
        let va = 1_234_567_891_011i128;
        for share in [0.0, 0.04, 0.5, 1.0] {
            let off = split_cell(va, share, false);
            assert_eq!(
                off,
                CellSplit {
                    private: va,
                    state: 0
                }
            );
        }
        assert_eq!(split_cell(va, 0.049, true).state, 0);
    }

    #[test]
    fn split_at_threshold_conserves_value_added() {
        let va = 1_000_000_000_007i128;
        let s = split_cell(va, SPLIT_THRESHOLD, true);
        assert!(s.state > 0);
        assert_eq!(s.private + s.state, va);
        let all = split_cell(va, 2.0, true);
        assert_eq!(
            all,
            CellSplit {
                private: 0,
                state: va
            }
        );
    }

    #[test]
    fn state_cell_profits_reach_the_government_account() {
        let profit = 9_000_000_000i128;
        let s = split_cell(profit, 0.4, true);
        let mut tx = Transactions::default();
        post_profits(&mut tx, s.private, s.state);
        assert!(tx.rows_close());
        assert_eq!(tx.net_lending(GOVERNMENT), s.state);
        assert_eq!(tx.net_lending(HOUSEHOLDS), s.private);
        assert_eq!(tx.net_lending(ENTERPRISES), -profit);
        assert!((0..N_SECTOR).map(|i| tx.net_lending(i)).sum::<i128>() == 0);
    }
}
