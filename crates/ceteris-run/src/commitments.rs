//! The commitments table (ADR 0241 D2, specs/workbench/SPEC.md): schedules legally adopted at
//! the start of a run, each a scheduled edit with an offset in ticks and a completion hazard
//! (zero by default). At its offset a commitment executes as an `Edit { source: Commitment }`
//! unless its hazard fires, in which case it is cancelled; either way the row leaves the table.
//! The hazard draw is one uniform per (commitment, tick) whether or not the rate is positive
//! (`ceteris_core::hazard`), so a zero rate changes nothing and branches share every draw.

use ceteris_core::entity::EntityId;
use ceteris_core::hazard::{COMMITMENT_COMPLETION, fires};
use ceteris_core::rng::Seed;

use crate::scenario::{Edit, EditSource};

/// The bound of the table (SPEC, Bounds).
pub const MAX_ROWS: usize = 4_096;

#[derive(Debug, Clone, PartialEq)]
pub struct Commitment {
    /// Keys the hazard draw; never reused within a run.
    pub id: u32,
    /// Ticks from the start of the run at which it executes.
    pub offset: u32,
    /// The edit it executes; `tick` and `source` are set when it fires.
    pub edit: Edit,
    /// Probability of cancellation at its offset.
    pub rate: f64,
}

#[derive(Debug, PartialEq)]
pub struct TableFull;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Commitments {
    rows: Vec<Commitment>,
}

impl Commitments {
    pub fn add(&mut self, c: Commitment) -> Result<(), TableFull> {
        if self.rows.len() >= MAX_ROWS {
            return Err(TableFull);
        }
        self.rows.push(c);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Removes the rows due at `tick` (offset reached; a row whose offset has passed is due
    /// too) and returns the edits of those whose hazard did not fire, in id order.
    pub fn due(&mut self, seed: Seed, tick: u32) -> Vec<Edit> {
        let (mut due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.rows)
            .into_iter()
            .partition(|c| c.offset <= tick);
        self.rows = rest;
        due.sort_by_key(|c| c.id);
        due.into_iter()
            .filter(|c| !fires(seed, tick, COMMITMENT_COMPLETION, EntityId(c.id), c.rate))
            .map(|c| Edit {
                tick,
                source: EditSource::Commitment,
                ..c.edit
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::EditKind;

    fn commitment(id: u32, offset: u32, rate: f64) -> Commitment {
        Commitment {
            id,
            offset,
            edit: Edit {
                tick: 0,
                source: EditSource::User,
                god: false,
                kind: EditKind::Policy,
                target: "tariff.0.1.0".into(),
                value: 0.1,
                query: None,
            },
            rate,
        }
    }

    #[test]
    fn commitment_with_zero_hazard_executes_at_offset() {
        let mut t = Commitments::default();
        t.add(commitment(1, 5, 0.0)).unwrap();
        for tick in 0..5 {
            assert!(t.due(Seed(1), tick).is_empty());
        }
        let edits = t.due(Seed(1), 5);
        assert_eq!(edits.len(), 1);
        assert_eq!(
            (edits[0].tick, edits[0].source),
            (5, EditSource::Commitment)
        );
        assert!(t.is_empty(), "executed rows leave the table");
        assert!(t.due(Seed(1), 6).is_empty());
    }

    #[test]
    fn certain_hazard_cancels_and_the_row_leaves() {
        let mut t = Commitments::default();
        t.add(commitment(1, 3, 1.0)).unwrap();
        assert!(t.due(Seed(1), 3).is_empty());
        assert!(t.is_empty());
    }

    #[test]
    fn a_rate_does_not_move_other_commitments_draws() {
        let outcome = |rate_of_first: f64| {
            let mut t = Commitments::default();
            t.add(commitment(1, 2, rate_of_first)).unwrap();
            (2..40u32)
                .map(|k| t.add(commitment(100 + k, 2, 0.5)).map(|()| k))
                .for_each(|r| {
                    r.unwrap();
                });
            t.due(Seed(9), 2).len()
        };
        // commitment 1 draws its own uniform: with rate 0 it executes, with rate 1 it does not,
        // and every other row's outcome is the same either way
        assert_eq!(outcome(0.0), outcome(1.0) + 1);
    }

    #[test]
    fn the_table_is_bounded() {
        let mut t = Commitments::default();
        for i in 0..MAX_ROWS as u32 {
            t.add(commitment(i, 1, 0.0)).unwrap();
        }
        assert_eq!(t.add(commitment(u32::MAX, 1, 0.0)), Err(TableFull));
    }
}
