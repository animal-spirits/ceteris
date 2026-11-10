# P5 E2 spike: the price step at Q = 0 under the money anchor, 2026-11-10

Spike before build (development laws) for E2 (RFC 0286, ADR 0339). Fixture only: the pack is
built owner-side. Code: project files `p5-e2/spike.patch` (an env switch `GAP` in
`Markets::reprice_anchored` and harness `crates/ceteris-econ/tests/e2_spike.rs`; not for merge).
All runs on main at 40e6c26 with `PriceAnchor::default()` (kappa_r 0.25 / 0.05, kappa_b 0.3,
kappa_m 0.2), one thread, from the calibrated steady state unless noted. x10 is every crossing's
non-tariff iceberg times 10 (`Wedges::ntb`), as in the E1 spike.

## Cause of the E1 x10 deflation

At an origin with D > 0 and Q near 0 the as-built gap l_o = (D + b - Q) / max(Q, Q_floor), Q_floor
1e-9 of the good's mean, reaches 1e4 to 1e11 (tick 3 onward). The ADR 0338 weights p (Q + D) / 2
include D, so that one cell carries weight and its gap dominates lbar_g and lbar. Every other origin
of the good then gets a_o (l_o - lbar_g) far below -c_g, and every other good gets kappa_b
(lbar_g - lbar) far below -c_g: every price steps at -0.3 from tick 3, the index falls 0.95,
0.66 (tick 10), 0.12 (30), 1e-4 (100), and the run reaches the price bound. The anchor term cannot
offset a mean that is not bounded. So the E1 x10 failure was the Q = 0 step, as RFC 0338 read it,
but through the good-level and relative means, not only the own-cell clip.

## Gaps tried

- 0 as built: l = (D + b - Q) / max(Q, Q_floor).
- 1 midpoint: l = 2 (D + b - Q) / (D + b + Q), 0 when both are 0. Bounded in [-2, 2], smooth
  except at D + b = Q = 0, first order equal to as built at the steady state, and with the ADR 0338
  weight v l = p (D + b - Q): lbar_g is the value of excess demand over traded value.
- 2 max: l = (D + b - Q) / max(Q, D + b, Q_floor). Bounded in [-1, 1], kinked at D + b = Q (RFC
  0286's bounded stored gap, here for every good).

## Results

End state at 1,200 ticks (I world index; Q/Q0 world output over the steady state's; zero = active
cells with Q < 1e-6 Q0, of 177; dmax the largest |dlnp| at the last tick).

| NTB | gap | frictionless | default |
| --- | --- | --- | --- |
| 1.5 | 0 | converged, I 1.0105, dmax 4e-9 | converged, dmax 5.6e-4 |
| 1.5 | 1 | same, dmax 2e-9 | same, dmax 6.0e-4 |
| 2 | 0 | converged, I 1.0148, dmax 1e-8 | dmax 1.7e-3 |
| 2 | 1 | same, dmax 3e-9 | dmax 4.5e-4 |
| 3 | 0 | collapse: I 2e-4, all cells Q = 0 | runs, Q/Q0 0.97, dmax 0.16 |
| 3 | 1 | runs, I 1.019, Q/Q0 0.99, 2 cells at 0, dmax 0.07 | runs, Q/Q0 1.00, dmax 0.12 |
| 5 | 0 | collapse to the price bound (I 0) | collapse (I 0) |
| 5 | 1 | I 1.058, Q/Q0 0.21, 63 cells at 0 | runs, Q/Q0 0.99, dmax 0.10 |
| 10 | 0 | collapse to the price bound (I 0) | collapse (I 0) |
| 10 | 1 | I 1.036 held; output to 0 at tick 500 to 600 | I 1.039 held; output to 0.01 at 1,200 |
| 10 | 2 | as gap 1, output to 0 by tick 1,000 | as gap 1, output to 0 at 1,200 |

Near the steady state the bounded gap changes nothing measurable: `p5_price_anchor` passes with gap
1 and 2 (steady state, nominal shock), and the fixture data state ends at V1 6.7e-2 price / 8.9e-2
wage (gap 1) against 7.1e-2 / 9.1e-2 (gap 0), I 1.0149 both. These data-state runs went through
the x10 harness with `set_wedges` at NTB 1, and their V1 is far from the trade SPEC's E1 figure
(1.9e-9 frictionless); the harness, not the gap, is the likely cause (same V1 with both gaps), but
it is not checked: rerun the data state with the E1 harness before relying on it.

## The second failure: a real trap at x10

With gap 1 the world index holds (1.03 to 1.09 the whole run), so the nominal mode is closed. Output
then dies province by province:

1. Frictionless, NTB 3, tick 1,190 to 1,200: province 24's Walrasian wage jumps 0.48, 2.47, 0.73,
   1.19, 1.12 per tick, and its goods take turns at Q = 0 and Q at 1.5 to 5 times Q0. All its prices
   sit within 0.32 to 0.41, so at eta 1 a small price change moves the whole labour force between
   goods. The bounded step keeps the cycle at 0.07 to 0.1 a tick but does not damp it.
2. Default, NTB 10, tick 1,000: province 24's outputs are 1e-21 to 1e-25 and halve each tick while D
   stays 5e6 per good; prices step +0.056 to +0.075 and the wage follows (1,007 to 1,113 against
   about 1 at t0). From tick 1,090 the same happens in other provinces; zero cells go from 0 to 69
   in 40 ticks and world output to 0.03 of Q0.

Reading (hypothesis, not proven): output that decays geometrically while D > 0 is the zero state
of the Leontief chain being absorbing. Y = min(VA / a_VA, min_k M_k / coef_k), and every input but
the binding one is ordered on constrained output (econ SPEC, E1 binding inputs), so when x10 cuts
the imported inputs, an enterprise orders its other inputs in proportion to the output it can no
longer make, and a province whose goods feed each other runs down together. The price step cannot
restart it: a higher price raises the value of output that the missing inputs prevent. This is ADR
0090's own-good and local-input reading of the pack x10 run, now seen without the nominal spiral on
top.

## Go / no-go

- Bounded gap for every good under the anchored rule: **go**. It is what removes the deflation to
  the price bound, it extends the fixture range from x2 to x3 (frictionless) and x5 (default), and it
  leaves the steady state, the nominal shock and the data state as they were. Midpoint over max: same
  results, smooth at D = Q (RFC 0286 needed one-sided differences across the max kink).
- x10 by the price step alone: **no-go**. The remaining failure is in production and labour, not in
  the step. Next: diagnose the zero-output trap (input orders on constrained output, the local chain
  at zero, the eta 1 labour flip) on the fixture, then the pack.
- Pending on the Mac: the RFC 0286 screen (eight storage-on and eight storage-off V1 starts at 6,000
  ticks) with the anchor and gap 1, and the pack data state and x10.
