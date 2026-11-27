# Loosened calibration tolerances (#600, RFC 0355)

Record required by the calibration rule: each gap carries a reason, a pinning test and an owner.
Owner of all rows: Antonio (decides whether a bound is kept or the cause is fixed). Dropped on Antonio's card 2026-11-27: the "nu independence 1e-10" item of #671, which matches no edit in the #600 diff. Source: the
diff of #600 (f14e7a3) and `crates/ceteris-calibrate/tests/steady_state.rs`.

| Bound | Was | Now | Reason in the commit history | Pinning test | Status |
|---|---|---|---|---|---|
| Fixture column factor, `abs(f - 1)` | 1e-9 | 1e-7 | The fixture balance was re-derived and rounded to 14 digits (exact ties flip frictionless rationed-wage branches); the settled factor sits at that rounding. | `r_g_removed_and_residual_reported` | Reason inferred from the same PR, not stated at the edit. Tighten once the balance is carried at full precision, or keep. |
| Per-good balance residual `abs(r_g - 1)` | 1e-12 | 1e-13 (re-enabled 2026-11-27, Antonio's card) | #600 had put the check under `if false` without a reason. Largest fixture residual is 1.0e-14 (good 1); the bound is ten times that. | `r_g_removed_and_residual_reported` | Closed. |
| Data-state pass budget (`Limits::max_passes`) | plain passes, 17 on the fixture | 14 | Anderson mixing needs 11 passes; the adaptive inner tolerance (RFC 0361) adds three full-tolerance passes. The budget is the observed count plus no slack. | `the_data_states_passes_settle_within_a_small_budget` | Kept: it fails if the mixing is lost. |
| Joint theta_2 recovery on made targets | exact | 1e-3 | theta_2 is refitted to `THETA_SETTLE` 1e-4 inside the scale loop (RFC 0355 D5), so the recovered value carries that settle. | the joint theta tests in `crates/ceteris-calibrate/tests/` (`on.theta2` against `truth`) | Kept at 1e-3, an order above the settle. |

Fixture fuels H2 gate (#671, Antonio 2026-11-27: record the reason, pin, run on the Mac): class 4
(fuels alone) has six crossings on the fixture, under `MIN_CLASS_ROWS` (8), so its tariff regression is
not identified and is skipped; classes 1 and 3 sit at eight. Pinned by
`kernel_controlled_tariff_regression_recovers_theta_per_class`, which asserts the skipped set is
exactly class 4 with six rows. Owner: Antonio. The real-pack `p2_gate` for the five-class fit runs in
the owner-pack Mac task with #669 and #684.
