//! The late-game flatness gate (ADR 0234): the median tick time of the last five years of a
//! run against the first five, and the row count of every state table per decade.

#[derive(Debug, Clone, PartialEq)]
pub struct Decade {
    pub year: u32,
    /// Row count per state table, in a fixed table order.
    pub rows: Vec<(String, usize)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Slowdown {
        ratio: f64,
    },
    /// A table with a declared bound passed it.
    OverBound {
        table: String,
        rows: usize,
        bound: usize,
    },
    /// A table without a bound changed its row count.
    Grew {
        table: String,
        first: usize,
        last: usize,
    },
}

pub const MAX_RATIO: f64 = 1.25;

pub fn median(times: &[f64]) -> f64 {
    let mut t = times.to_vec();
    t.sort_by(f64::total_cmp);
    let n = t.len();
    if n % 2 == 1 {
        t[n / 2]
    } else {
        (t[n / 2 - 1] + t[n / 2]) / 2.0
    }
}

/// `ticks_per_year` tick times in seconds per tick over the whole run: ratio of the median over
/// years 96 to 100 to the median over years 1 to 5 (the last five and first five of the run).
pub fn ratio(tick_secs: &[f64], ticks_per_year: usize) -> f64 {
    let window = 5 * ticks_per_year;
    let (head, tail) = (&tick_secs[..window], &tick_secs[tick_secs.len() - window..]);
    median(tail) / median(head)
}

/// Checks the time ratio and every table of every decade. A table listed in `bounds` may grow
/// up to its bound (0236); any other table must keep the row count it had at the first decade.
pub fn check(ratio: f64, decades: &[Decade], bounds: &[(&str, usize)]) -> Vec<Failure> {
    let mut out = Vec::new();
    if ratio > MAX_RATIO {
        out.push(Failure::Slowdown { ratio });
    }
    let Some(first) = decades.first() else {
        return out;
    };
    for d in decades {
        for (table, rows) in &d.rows {
            match bounds.iter().find(|(t, _)| t == table) {
                Some(&(_, bound)) if *rows > bound => out.push(Failure::OverBound {
                    table: table.clone(),
                    rows: *rows,
                    bound,
                }),
                Some(_) => {}
                None => {
                    let start = first.rows.iter().find(|(t, _)| t == table).map(|r| r.1);
                    if start != Some(*rows) {
                        out.push(Failure::Grew {
                            table: table.clone(),
                            first: start.unwrap_or(0),
                            last: *rows,
                        });
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Row counts at each decade of a run where `grow` rows are appended per tick.
    fn decades(grow: usize) -> Vec<Decade> {
        (0..=10)
            .map(|d| Decade {
                year: d * 10,
                rows: vec![
                    ("provinces".into(), 4000),
                    ("synthetic".into(), 1 + grow * d as usize * 10 * 12),
                ],
            })
            .collect()
    }

    #[test]
    fn median_of_even_and_odd_windows() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), 2.5);
    }

    #[test]
    fn ratio_compares_last_window_to_first() {
        let times: Vec<f64> = (0..120).map(|i| if i < 60 { 1.0 } else { 1.5 }).collect();
        assert_eq!(ratio(&times, 6), 1.5);
    }

    #[test]
    fn flat_run_passes() {
        assert!(check(1.1, &decades(0), &[]).is_empty());
    }

    #[test]
    fn slowdown_beyond_a_quarter_fails() {
        assert_eq!(
            check(1.26, &decades(0), &[]),
            [Failure::Slowdown { ratio: 1.26 }]
        );
    }

    #[test]
    fn synthetic_table_appending_a_row_per_tick_fails_the_gate() {
        let fails = check(1.0, &decades(1), &[]);
        assert!(
            fails
                .iter()
                .any(|f| matches!(f, Failure::Grew { table, .. } if table == "synthetic"))
        );
    }

    #[test]
    fn bounded_table_may_grow_to_its_bound_only() {
        let d = decades(1);
        assert!(check(1.0, &d, &[("synthetic", 1201)]).is_empty());
        assert!(matches!(
            check(1.0, &d, &[("synthetic", 600)])[0],
            Failure::OverBound { .. }
        ));
    }
}
