# 0103. Initial drift: 2000 is not a rest point

Status: proposed
Date: 2026-10-26
Design sections: 3.3, 7.5, 15.1, 15.4; amends ADR 0089

## Context

ADR 0089 treats the 2000 data state as a steady state: values are fixed, prices solve, and A
and coefficients absorb everything else. The 2000 world was visibly in transition: Chinese and
Indian capital deepening, recovery from the 1997-98 crises, a commodity price trough, and
widening imbalances (ADR 0100). In a steady-state calibration each of these becomes a
permanent level effect, and the only sources of 2000-2010 dynamics are catch-up (7.5) and
shocks. Expected symptoms: fast growers under-predicted, holdout error front-loaded, and
catch-up parameters inflated to compensate.

## Options

1. Keep the steady state.
2. Keep the 0089 inversion and add per-country exogenous drifts estimated from pre-2000 data,
   fading with a common half-life.
3. Dynamic hat algebra from observed 1995-2000 changes (Caliendo, Dvorkin and Parro 2019),
   which requires forward-looking agents P2 does not have.

## Decision

Option 2. Option 3 is the target once P5 adds forward-looking saving and P4 adds investment.

- Each country gets two drifts at t = 0:
  - TFP drift g_A,c: the 1995-2000 trend of PWT TFP (rtfpna) relative to the world, by
    country, net of ADR 0108's catch-up term so the same convergence is not counted twice,
    shrunk toward its regional mean (empirical Bayes) to damp noise.
  - Capital deepening drift g_K,c: the gap between the country's 2000 capital-output ratio
    (PWT) and its 1995-2000 trend steady-state value, entering as growth of the fixed factor F.
- Both fade as exp(-t / h) with one global half-life h (new register entry, prior 3 to 15
  years, free).
- The steady-state calibration is unchanged: drifts are zero at the exact data state's first
  tick and act from then on.
- Switch (new 17.2 entry): drift on (default) or off; the holdout scorecard reports both.

## Migration

- Pack: `econ_countries.arrow` gains `g_A`, `g_K` and their source flags.
- Econ: A and F of each enterprise grow by the country drift each tick.
- ADR 0089's tests apply to the drift-off run unchanged.

## Tests

- With drift off, hashes equal the current golden.
- With drift on and h to infinity, a toy country's output grows at g_A + s_L-weighted g_K.
- In the 2000-2010 fit, catch-up parameters with drift on are reported against drift off; a
  catch-up rate that falls when drift is on is the expected sign.

## Consequences

- The steady-state assumption remains for prices and coefficients, where it does least harm.
- One free parameter (h) is added; the 60-parameter cap counts it.
- Drifts are exogenous and fade, so they cannot explain the holdout by themselves; the
  ablation (with drift off as the alternative) decides whether they stay.
