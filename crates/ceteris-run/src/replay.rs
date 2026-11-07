//! The replay loader and the calendar-blind rule (ADR 0241, `specs/workbench/SPEC.md`).
//!
//! A tape is a table of dated rows. It is read once, at t0, and turned into ticks and
//! `Edit { source: replay }` entries; nothing reads a date after that.

use std::fmt;

use crate::scenario::{Edit, EditKind, EditSource, ReplaySel, Scenario};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    Tape { line: usize, why: String },
    Read(String),
    CalendarRead { table: String, tick: u32 },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Tape { line, why } => write!(f, "tape line {line}: {why}"),
            Self::Read(e) => write!(f, "tape: {e}"),
            Self::CalendarRead { table, tick } => write!(
                f,
                "table `{table}` is keyed by calendar date and read at tick {tick}; \
                 dates are read at t0 only"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

/// Fails a read of a date-keyed table after t0: no system reads a calendar date, so the start
/// year can be relabelled without changing a run.
pub fn check_table_read(table: &str, keyed_by_date: bool, tick: u32) -> Result<(), ReplayError> {
    if keyed_by_date && tick > 0 {
        return Err(ReplayError::CalendarRead {
            table: table.to_string(),
            tick,
        });
    }
    Ok(())
}

/// Tape rows as `date,family,target,value` lines, date `YYYY-MM`, family `shocks` or
/// `policy`; a header line, blank lines and `#` comments are skipped. Edits come out in tick
/// order, ties in file order. Rows before `start_year` are dropped. Shock rows become
/// `Event` edits, which have no target at P3.
pub fn replay_edits(
    sel: ReplaySel,
    start_year: u32,
    read: impl FnOnce() -> Result<String, String>,
) -> Result<Vec<Edit>, ReplayError> {
    if sel == ReplaySel::Off {
        return Ok(Vec::new());
    }
    let text = read().map_err(ReplayError::Read)?;
    let mut edits = Vec::new();
    for (i, line) in text.lines().enumerate().map(|(i, l)| (i + 1, l.trim())) {
        if line.is_empty() || line.starts_with('#') || line.starts_with("date,") {
            continue;
        }
        let bad = |why: &str| ReplayError::Tape {
            line: i,
            why: why.to_string(),
        };
        let [date, family, target, value]: [&str; 4] = line
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| bad("want date,family,target,value"))?;
        let (year, month) = date.split_once('-').ok_or_else(|| bad("date is YYYY-MM"))?;
        let year: u32 = year.parse().map_err(|_| bad("date is YYYY-MM"))?;
        let month: u32 = month.parse().map_err(|_| bad("date is YYYY-MM"))?;
        if !(1..=12).contains(&month) {
            return Err(bad("month is 1 to 12"));
        }
        let kind = match (family, sel) {
            ("policy", ReplaySel::Policy | ReplaySel::All) => EditKind::Policy,
            ("shocks", ReplaySel::Shocks | ReplaySel::All) => EditKind::Event,
            ("policy" | "shocks", _) => continue,
            _ => return Err(bad("family is shocks or policy")),
        };
        let value: f64 = value.parse().map_err(|_| bad("value is a number"))?;
        let Some(tick) = year.checked_sub(start_year).map(|y| y * 12 + month - 1) else {
            continue;
        };
        edits.push(Edit {
            tick,
            source: EditSource::Replay,
            god: false,
            kind,
            target: target.to_string(),
            value,
            query: None,
        });
    }
    edits.sort_by_key(|e| e.tick);
    Ok(edits)
}

impl Scenario {
    /// Appends replay edits to every root branch's log, where they are logged and hashed
    /// like any other edit.
    pub fn add_replay(&mut self, edits: &[Edit]) {
        for b in self.branches.iter_mut().filter(|b| b.parent.is_none()) {
            b.edits.extend(edits.iter().cloned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAPE: &str = "date,family,target,value\n\
        2000-03,policy,tariff.0.1.0,0.1\n\
        2000-01,policy,tariff.0.1.0,0.05\n\
        # comment\n\
        2001-01,shocks,drought.3,1\n\
        2002-06,policy,tariff.0.1.0,0.2\n";

    fn load(sel: ReplaySel, year: u32) -> Vec<Edit> {
        replay_edits(sel, year, || Ok(TAPE.to_string())).unwrap()
    }

    #[test]
    fn replay_off_reads_no_tape() {
        let edits = replay_edits(ReplaySel::Off, 2000, || panic!("read")).unwrap();
        assert!(edits.is_empty());
    }

    #[test]
    fn families_select_rows_and_dates_become_ticks() {
        let ticks = |s| load(s, 2000).iter().map(|e| e.tick).collect::<Vec<_>>();
        assert_eq!(ticks(ReplaySel::Policy), [0, 2, 29]);
        assert_eq!(ticks(ReplaySel::Shocks), [12]);
        assert_eq!(ticks(ReplaySel::All), [0, 2, 12, 29]);
        assert!(
            load(ReplaySel::All, 2000)
                .iter()
                .all(|e| e.source == EditSource::Replay)
        );
        // rows before the pack's origin are dropped
        assert_eq!(load(ReplaySel::Policy, 2002).len(), 1);
    }

    #[test]
    fn bad_rows_name_their_line() {
        let bad = |t: &'static str| replay_edits(ReplaySel::All, 2000, || Ok(t.to_string()));
        assert!(matches!(
            bad("2000-13,policy,x,1"),
            Err(ReplayError::Tape { line: 1, .. })
        ));
        assert!(matches!(
            bad("2000-01,war,x,1"),
            Err(ReplayError::Tape { line: 1, .. })
        ));
        assert!(matches!(
            bad("2000-01,policy,x"),
            Err(ReplayError::Tape { line: 1, .. })
        ));
    }

    #[test]
    fn date_keyed_table_after_t0_is_load_error() {
        assert!(check_table_read("tariff_tape", true, 0).is_ok());
        assert!(check_table_read("province_geometry", false, 5).is_ok());
        assert_eq!(
            check_table_read("tariff_tape", true, 1),
            Err(ReplayError::CalendarRead {
                table: "tariff_tape".into(),
                tick: 1
            })
        );
    }
}
