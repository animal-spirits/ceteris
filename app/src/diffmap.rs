//! W2 difference map data (specs/p3/PLAN.md Branch W): a variable as branch minus baseline per
//! province, coloured on a diverging ramp that passes the 0221 check, with a mask of provinces
//! where the sign is not robust across ensemble members (drawn hatched by the renderer).
//! Inputs are the paired-effect fields of the `effects` table, so the app needs no ensemble code.

use crate::modes::{LegendItem, NO_DATA, Painted, classify, fmt, quantile_breaks, rgba};

/// Classes per sign.
pub const CLASSES: usize = 4;
/// Share of members that must agree with the mean's sign for the colour to be trusted.
pub const ROBUST_SHARE: f64 = 0.8;

/// Neutral to strong, each side; lightness falls away from the neutral (ramps::check, sequential).
pub const NEGATIVE: [[u8; 3]; CLASSES] = [
    [0xc6, 0xdb, 0xef],
    [0x6b, 0xae, 0xd6],
    [0x31, 0x82, 0xbd],
    [0x08, 0x45, 0x94],
];
pub const POSITIVE: [[u8; 3]; CLASSES] = [
    [0xfd, 0xd0, 0xa2],
    [0xfd, 0x8d, 0x3c],
    [0xd9, 0x48, 0x01],
    [0x7f, 0x27, 0x04],
];
/// A province whose mean difference is exactly zero.
pub const ZERO: [u8; 3] = [0xf2, 0xf2, 0xee];

/// One province's paired effect: mean of branch minus baseline and the share of members whose
/// difference has the mean's sign (`ceteris_run::effects::Effect::sign_share`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub mean: f64,
    pub sign_share: f64,
}

/// Fill colours, a hatch mask (true = sign not robust) and the legend. `None` cells are no data.
pub struct Diff {
    pub painted: Painted,
    pub hatch: Vec<bool>,
}

/// Class breaks are shared by both signs, taken at quantiles of the absolute differences, so
/// equal colours mean equal magnitudes on either side.
pub fn paint(cells: &[Option<Cell>], title: &str, unit: &str) -> Diff {
    let mags: Vec<f64> = cells
        .iter()
        .flatten()
        .map(|c| c.mean.abs())
        .filter(|x| x.is_finite())
        .collect();
    let breaks = quantile_breaks(&mags, CLASSES);
    let mut fill = Vec::with_capacity(cells.len());
    let mut hatch = Vec::with_capacity(cells.len());
    for cell in cells {
        match cell {
            Some(c) if c.mean.is_finite() => {
                let class = classify(c.mean.abs(), &breaks).min(CLASSES - 1);
                let rgb = if c.mean == 0.0 {
                    ZERO
                } else if c.mean < 0.0 {
                    NEGATIVE[class]
                } else {
                    POSITIVE[class]
                };
                fill.push(rgba(rgb));
                hatch.push(c.mean != 0.0 && c.sign_share < ROBUST_SHARE);
            }
            _ => {
                fill.push(rgba(NO_DATA));
                hatch.push(false);
            }
        }
    }
    let mut legend = Vec::new();
    for (class, &colour) in NEGATIVE.iter().enumerate().rev() {
        legend.push(LegendItem {
            colour,
            label: band(class, &breaks, "below", unit),
        });
    }
    legend.push(LegendItem {
        colour: ZERO,
        label: "no change".to_string(),
    });
    for (class, &colour) in POSITIVE.iter().enumerate() {
        legend.push(LegendItem {
            colour,
            label: band(class, &breaks, "above", unit),
        });
    }
    Diff {
        painted: Painted {
            title: title.to_string(),
            fill,
            legend,
            note: format!(
                "Branch minus baseline. Hatched: under {:.0}% of members agree on the sign.",
                ROBUST_SHARE * 100.0
            ),
        },
        hatch,
    }
}

fn band(class: usize, breaks: &[f64], side: &str, unit: &str) -> String {
    let lo = class
        .checked_sub(1)
        .and_then(|k| breaks.get(k))
        .copied()
        .unwrap_or(0.0);
    let sign = if side == "below" { "-" } else { "+" };
    match breaks.get(class) {
        Some(&hi) if class < breaks.len() => {
            format!("{sign}{} to {sign}{} {unit}", fmt(lo), fmt(hi))
        }
        _ => format!("{sign}{}+ {unit}", fmt(lo)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ramps;

    fn cell(mean: f64, sign_share: f64) -> Option<Cell> {
        Some(Cell { mean, sign_share })
    }

    #[test]
    fn both_halves_pass_the_0221_check() {
        for (name, ramp) in [("negative", &NEGATIVE[..]), ("positive", &POSITIVE[..])] {
            let problems = ramps::check(ramp, true);
            assert!(problems.is_empty(), "{name}: {problems:?}");
        }
        // Neutral to the weakest class of each side stays distinguishable.
        for ramp in [[ZERO, NEGATIVE[0]], [ZERO, POSITIVE[0]]] {
            assert!(ramps::check(&ramp, false).is_empty());
        }
    }

    #[test]
    fn sign_picks_the_side_and_magnitude_the_class() {
        let cells: Vec<_> = [-8.0, -4.0, -2.0, -1.0, 0.0, 1.0, 2.0, 4.0, 8.0]
            .iter()
            .map(|&m| cell(m, 1.0))
            .collect();
        let d = paint(&cells, "t", "u");
        let f = &d.painted.fill;
        assert_eq!(f[0], rgba(NEGATIVE[3]));
        assert_eq!(f[8], rgba(POSITIVE[3]));
        assert_eq!(f[4], rgba(ZERO));
        assert_eq!(f[3], rgba(NEGATIVE[0]));
        assert_eq!(f[5], rgba(POSITIVE[0]));
        assert_eq!(d.painted.legend.len(), 2 * CLASSES + 1);
    }

    #[test]
    fn hatch_marks_only_nonrobust_nonzero_provinces() {
        let cells = vec![cell(3.0, 0.5), cell(3.0, 0.9), cell(0.0, 0.0), None];
        let d = paint(&cells, "t", "u");
        assert_eq!(d.hatch, vec![true, false, false, false]);
        assert_eq!(d.painted.fill[3], rgba(NO_DATA));
    }

    #[test]
    fn colours_are_deterministic_and_swap_under_negation() {
        let a: Vec<_> = (1..=12).map(|k| cell(f64::from(k), 1.0)).collect();
        let b: Vec<_> = (1..=12).map(|k| cell(-f64::from(k), 1.0)).collect();
        let (da, db) = (paint(&a, "t", "u"), paint(&b, "t", "u"));
        for (x, y) in da.painted.fill.iter().zip(&db.painted.fill) {
            let ix = POSITIVE.iter().position(|c| rgba(*c) == *x).unwrap();
            let iy = NEGATIVE.iter().position(|c| rgba(*c) == *y).unwrap();
            assert_eq!(ix, iy);
        }
    }
}
