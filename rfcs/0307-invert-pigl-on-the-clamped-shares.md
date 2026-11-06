# RFC 0307. Invert PIGL on the shares the run spends, clamps included (amends ADR 0094)

Status: accepted
Date: 2026-11-04
Design sections: 8.6; econ SPEC (Final demand, Inversion)
Research: none (diagnosis of the C6 pack gate, below)

## Summary

A pop's PIGL category shares are s_c = sB_c + k (sD_c - sB_c), k the income-effect weight of its
income. Where k > 1, a pop far below its country's base, a share can be negative; `pigl_shares`
sets it to 0 and rescales the rest (ADR 0021). The joint inversion of ADR 0094 meets each
country's shares on the unclamped shares, so where some pop clamps, what the run spends differs
from what was inverted. On the C6 pack 15 countries have such pops (Albania's poorest -0.65 in
services, Burundi, Kyrgyzstan, Liberia, Sudan, Ethiopia and others), their traded spending falls
1% to 5% short of the balance's targets, the calibration's fixed point keeps a per-good miss of
-2.7e-5 to +1.0e-5 (`balance_residual`), and the pack gate's reference check fails.

## Decisions

### D1. On which shares does the inversion meet the data?

Options:

- 1. The shares the run spends, clamp included: lambda keeps its closed form where no pop clamps
  at it (every country that inverts today, unchanged to the bit); else lambda meets the binding
  category's aggregate of the clamped shares by a bracketed secant, and the split and the
  within-category rounds use the clamped shares too.
- 2. Keep the unclamped inversion and widen the gate's tolerance to the reported residual.
- 3. Change the clamp (a smooth floor), which moves every far-from-base pop's demand.

Recommendation: 1, because the inversion then reproduces what the run spends, and nothing moves
where no pop clamps.

Decision: 1, Antonio, 2026-11-04.
Row: ADR 0316

## Spec changes

- 8.6: the inversion meets the data on the shares the run spends, a negative one at 0 (ADR 0316).
- Econ SPEC, Final demand, Inversion: lambda by secant where some pop clamps.

## Tests

- `inversion_meets_the_shares_with_clamped_pops` (econ `demand.rs`; reference
  `tests/reference.rs`): three pops, the poorest at 2% of base with its services share clamped
  to 0; the clamped shares meet the country's (1e-11).
- Existing inversion tests unchanged (no pop clamps there).

## Consequences

- The C6 calibration's per-good miss closes where it came from clamps; countries without
  clamping pops keep their weights bit for bit.
- The inversion costs a secant on lambda per split evaluation, only in the countries that clamp.
