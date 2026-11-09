//! W3 fan chart geometry (specs/p3/PLAN.md Branch W): a branch and its baseline as quantile
//! bands across ensemble members, laid out in pixel space for the renderer. Pure arithmetic with
//! no GPU or ensemble code: inputs are per-tick quantiles from the `series` and `effects` tables.

/// Quantiles of one series across members at one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub tick: f64,
    pub lo: f64,
    pub mid: f64,
    pub hi: f64,
}

pub type Point = [f64; 2];

pub struct Chart {
    /// Closed polygon: the upper edge left to right, then the lower edge right to left.
    pub branch_band: Vec<Point>,
    pub branch_mid: Vec<Point>,
    pub baseline_band: Vec<Point>,
    pub baseline_mid: Vec<Point>,
    /// Value and pixel y of each horizontal grid line.
    pub y_ticks: Vec<(f64, f64)>,
    pub x_ticks: Vec<(f64, f64)>,
}

/// Ticks at 1, 2 or 5 times a power of ten covering `[min, max]`, about `target` of them.
pub fn nice_ticks(min: f64, max: f64, target: usize) -> Vec<f64> {
    if !(min.is_finite() && max.is_finite()) || max < min || target == 0 {
        return Vec::new();
    }
    let span = (max - min).max(f64::MIN_POSITIVE);
    let raw = span / target as f64;
    let mut mag = 1.0;
    while mag > raw {
        mag /= 10.0;
    }
    while mag * 10.0 <= raw {
        mag *= 10.0;
    }
    let step = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * mag)
        .find(|&s| s >= raw)
        .unwrap_or(10.0 * mag);
    let first = (min / step).floor() as i64;
    let last = (max / step).ceil() as i64;
    (first..=last).map(|k| k as f64 * step).collect()
}

/// Lays both series out in a `width` by `height` box, y down. The y range covers both bands;
/// a flat range is widened so the lines stay inside the box. Empty input gives `None`.
pub fn layout(branch: &[Band], baseline: &[Band], width: f64, height: f64) -> Option<Chart> {
    let all = || branch.iter().chain(baseline);
    let (mut t0, mut t1) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut v0, mut v1) = (f64::INFINITY, f64::NEG_INFINITY);
    for b in all() {
        t0 = t0.min(b.tick);
        t1 = t1.max(b.tick);
        v0 = v0.min(b.lo.min(b.mid).min(b.hi));
        v1 = v1.max(b.lo.max(b.mid).max(b.hi));
    }
    if !(t0.is_finite() && v0.is_finite()) {
        return None;
    }
    if v1 <= v0 {
        v0 -= 0.5;
        v1 += 0.5;
    }
    let x_ticks = nice_ticks(t0, t1, 6);
    let y_ticks = nice_ticks(v0, v1, 5);
    // Round the value range out to the grid so the top and bottom lines are drawn.
    let ylo = y_ticks.first().copied().unwrap_or(v0).min(v0);
    let yhi = y_ticks.last().copied().unwrap_or(v1).max(v1);
    let tspan = if t1 > t0 { t1 - t0 } else { 1.0 };
    let px = |t: f64| (t - t0) / tspan * width;
    let py = |v: f64| height - (v - ylo) / (yhi - ylo) * height;
    let side = |s: &[Band]| {
        let mut poly: Vec<Point> = s.iter().map(|b| [px(b.tick), py(b.hi)]).collect();
        poly.extend(s.iter().rev().map(|b| [px(b.tick), py(b.lo)]));
        let mid = s.iter().map(|b| [px(b.tick), py(b.mid)]).collect();
        (poly, mid)
    };
    let (branch_band, branch_mid) = side(branch);
    let (baseline_band, baseline_mid) = side(baseline);
    Some(Chart {
        branch_band,
        branch_mid,
        baseline_band,
        baseline_mid,
        y_ticks: y_ticks.iter().map(|&v| (v, py(v))).collect(),
        x_ticks: x_ticks
            .iter()
            .filter(|&&t| t >= t0 && t <= t1)
            .map(|&t| (t, px(t)))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(tick: f64, lo: f64, mid: f64, hi: f64) -> Band {
        Band { tick, lo, mid, hi }
    }

    #[test]
    fn ticks_are_one_two_five_steps_covering_the_range() {
        assert_eq!(nice_ticks(0.0, 10.0, 5), [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(
            nice_ticks(3.0, 97.0, 5),
            [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        );
        let t = nice_ticks(0.012, 0.047, 4);
        assert!(t[0] <= 0.012 && *t.last().unwrap() >= 0.047);
        assert!(nice_ticks(f64::NAN, 1.0, 5).is_empty());
    }

    #[test]
    fn higher_values_sit_higher_on_screen_and_bands_close() {
        let b = [band(0.0, 1.0, 2.0, 3.0), band(10.0, 2.0, 4.0, 6.0)];
        let base = [band(0.0, 1.0, 1.5, 2.0), band(10.0, 1.0, 1.5, 2.0)];
        let c = layout(&b, &base, 100.0, 50.0).unwrap();
        assert_eq!(c.branch_band.len(), 4);
        assert!(c.branch_mid[1][1] < c.baseline_mid[1][1]);
        assert_eq!(c.branch_mid[0][0], 0.0);
        assert_eq!(c.branch_mid[1][0], 100.0);
        // The upper edge is above the lower edge at every tick.
        assert!(c.branch_band[0][1] < c.branch_band[3][1]);
        assert!(c.branch_band[1][1] < c.branch_band[2][1]);
        for p in c.branch_band.iter().chain(&c.baseline_band) {
            assert!(
                (0.0..=50.0).contains(&p[1]) && (0.0..=100.0).contains(&p[0]),
                "{p:?}"
            );
        }
    }

    #[test]
    fn a_flat_series_stays_inside_the_box_and_empty_input_is_none() {
        let flat = [band(0.0, 5.0, 5.0, 5.0), band(1.0, 5.0, 5.0, 5.0)];
        let c = layout(&flat, &flat, 10.0, 10.0).unwrap();
        assert!(c.branch_mid.iter().all(|p| (0.0..=10.0).contains(&p[1])));
        assert!(layout(&[], &[], 10.0, 10.0).is_none());
        let one = [band(3.0, 1.0, 2.0, 3.0)];
        assert!(layout(&one, &[], 10.0, 10.0).is_some());
    }
}
