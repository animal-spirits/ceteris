# Loosened calibration tolerances (#600, RFC 0355)

Record required by the calibration rule: each gap carries a reason, a pinning test and an owner.
Owner of all rows: Antonio (decides whether a bound is kept or the cause is fixed). Source: the
diff of #600 (f14e7a3) and `crates/ceteris-calibrate/tests/steady_state.rs`.

| Bound | Was | Now | Reason in the commit history | Pinning test | Status |
|---|---|---|---|---|---|
| Fixture column factor, `abs(f - 1)` | 1e-9 | 1e-7 | The fixture balance was re-derived and rounded to 14 digits (exact ties flip frictionless rationed-wage branches); the settled factor sits at that rounding. | `r_g_removed_and_residual_reported` | Reason inferred from the same PR, not stated at the edit. Tighten once the balance is carried at full precision, or keep. |
| Per-good balance residual `abs(r_g - 1)` | 1e-12 | not asserted: the check sits under `if false` | None recorded. The residual is still printed. | `r_g_removed_and_residual_reported` | A disabled assertion, not a loosened one. Re-enable at a bound the fixture meets, or delete the check and say why. Open. |
| Data-state pass budget (`Limits::max_passes`) | plain passes, 17 on the fixture | 14 | Anderson mixing needs 11 passes; the adaptive inner tolerance (RFC 0361) adds three full-tolerance passes. The budget is the observed count plus no slack. | `the_data_states_passes_settle_within_a_small_budget` | Kept: it fails if the mixing is lost. |
| Joint theta_2 recovery on made targets | exact | 1e-3 | theta_2 is refitted to `THETA_SETTLE` 1e-4 inside the scale loop (RFC 0355 D5), so the recovered value carries that settle. | the joint theta tests in `crates/ceteris-calibrate/tests/` (`on.theta2` against `truth`) | Kept at 1e-3, an order above the settle. |
| nu independence of the fit, 1e-10 (named in #671) | n/a | n/a | No such edit found in the #600 diff; the 1e-10 bounds in `steady_state.rs` (lines 734 to 740, 971, 1741) predate it. | none | Open: Antonio to say which test #671 meant, or drop the item. |

Also open from #671: the fixture fuels H2 gate is skipped below `MIN_CLASS_ROWS` (eight rows);
accept the skip or replace the gate with a fixture of at least eight crossings. A real-pack
`p2_gate` for the five-class fit is not written (needs the owner-built pack).
