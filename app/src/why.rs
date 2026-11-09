//! W4 why panel data (ADR 0049, specs/p3/PLAN.md Branch W): the channel shares of an effect
//! with their intervals, ranked and formatted for the panel. Inputs are the C4 attribution
//! rows (share of the total effect per channel, with a bootstrap interval).

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub name: String,
    /// Share of the total effect; negative when the channel works against it.
    pub share: f64,
    pub lo: f64,
    pub hi: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub name: String,
    pub text: String,
    /// Bar length as a fraction of the largest absolute share, signed.
    pub bar: f64,
    /// The interval spans zero, so the sign of the share is not established.
    pub uncertain: bool,
}

fn pct(x: f64) -> String {
    format!("{:.0}%", x * 100.0)
}

/// Channels ranked by absolute share (ties by name), non-finite rows dropped. The text reads
/// "name 35% (28% to 41%)"; `uncertain` flags intervals that cross zero.
pub fn lines(channels: &[Channel]) -> Vec<Line> {
    let mut rows: Vec<&Channel> = channels
        .iter()
        .filter(|c| c.share.is_finite() && c.lo.is_finite() && c.hi.is_finite())
        .collect();
    rows.sort_by(|a, b| {
        b.share
            .abs()
            .total_cmp(&a.share.abs())
            .then_with(|| a.name.cmp(&b.name))
    });
    let top = rows.first().map_or(0.0, |c| c.share.abs());
    rows.iter()
        .map(|c| Line {
            name: c.name.clone(),
            text: format!(
                "{} {} ({} to {})",
                c.name,
                pct(c.share),
                pct(c.lo),
                pct(c.hi)
            ),
            bar: if top > 0.0 { c.share / top } else { 0.0 },
            uncertain: c.lo <= 0.0 && c.hi >= 0.0,
        })
        .collect()
}

/// What the shares leave unexplained: one minus their sum (interaction or residual).
pub fn unexplained(channels: &[Channel]) -> f64 {
    1.0 - channels
        .iter()
        .filter(|c| c.share.is_finite())
        .map(|c| c.share)
        .sum::<f64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(name: &str, share: f64, lo: f64, hi: f64) -> Channel {
        Channel {
            name: name.into(),
            share,
            lo,
            hi,
        }
    }

    #[test]
    fn ranked_by_absolute_share_with_signed_bars() {
        let l = lines(&[
            ch("price", 0.2, 0.1, 0.3),
            ch("wage", -0.5, -0.6, -0.4),
            ch("trade", 0.25, 0.2, 0.3),
        ]);
        let names: Vec<_> = l.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["wage", "trade", "price"]);
        assert_eq!(l[0].bar, -1.0);
        assert_eq!(l[1].bar, 0.5);
        assert_eq!(l[0].text, "wage -50% (-60% to -40%)");
    }

    #[test]
    fn intervals_crossing_zero_are_flagged_and_bad_rows_dropped() {
        let l = lines(&[
            ch("a", 0.1, -0.05, 0.2),
            ch("b", f64::NAN, 0.0, 0.0),
            ch("c", 0.1, 0.0, 0.2),
        ]);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|x| x.uncertain));
        assert_eq!(l[0].name, "a");
        assert!(lines(&[]).is_empty());
    }

    #[test]
    fn unexplained_is_one_minus_the_sum() {
        let u = unexplained(&[ch("a", 0.6, 0.5, 0.7), ch("b", 0.3, 0.2, 0.4)]);
        assert!((u - 0.1).abs() < 1e-12);
        assert_eq!(unexplained(&[]), 1.0);
    }
}
