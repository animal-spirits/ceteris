# 0221. Colour-vision-safe map modes and UI scale

Status: proposed
Date: 2026-10-28
Design sections: 14.5, 20.1; amends ADR 0137; ADRs 0088, 0140, 0188
Phase: App shell (scale, ramps), P3 (difference maps)

## Context

Covered already: keyboard-only use and screen-reader names through AccessKit (ADR 0188), reduced
motion (0137), WCAG AA contrast for chrome (0140). Missing: map-mode palettes that read under the
common colour-vision deficiencies, difference maps whose robustness mask (14.5) does not rely on
colour alone, and a UI scale. The map is the product, and difference maps carry its results.
Audit gap 14.

## Options

1. Leave palettes to taste.
2. Check every ramp under simulated deficiencies in CI, encode the mask by pattern, and add a scale
   setting.

## Decision

Option 2.

- **Ramps.** Every sequential and diverging ramp in 0137's token set passes a check: under
  simulated protanopia, deuteranopia and tritanopia (Machado, Oliveira and Fernandes 2009),
  adjacent classes differ by at least a set CIEDE2000 distance, and lightness is monotonic on
  sequential ramps.
- **Categories.** Categorical modes (owner, groups) use at most 12 checked colours; beyond that,
  patterns and labels on hover.
- **Robustness mask.** Provinces where the sign is not robust across seeds (14.5) are hatched, not
  only greyed.
- **Scale.** UI scale from 85% to 200%, following the macOS text-size setting by default; body text
  never below 11 px at 100%.

## Migration

- ADR 0137: status "amended by 0221"; its ramps are re-checked.
- App shell: ramp check in CI and the scale setting. P3: hatching on difference maps.

## Tests

- The ramp check runs in CI and fails on a ramp that collapses two classes under any simulation.
- Every panel renders at 200% without clipping.

## Consequences

Results read correctly for about one in twelve men who would otherwise misread the difference maps.
Some 0137 colours may shift.
