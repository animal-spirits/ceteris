# Free-moment rerun results, 11-12 November 2026 (partial)

Run per `docs/free-moment-rerun.md` on the Mac, logs in `~/ceteris-logs/rerun/` (steps.log). Only
the `P` pack columns completed. Two packs failed calibration. H2 was rerun afterwards (below).

## Completed: P (off, e1, e3)

| step | off | e1 | e3 |
|---|---|---|---|
| benchmark (16 tests) | 16 pass | 13 pass, 3 fail | 13 pass, 3 fail |
| RFC 0286 screen, P | ran | ran | ran |

Benchmark failures (`crates/ceteris-calibrate/tests/benchmark.rs`):
- e1: `japan_loses_from_costlier_borders` (:748, JPN welfare +0.0350, wrong sign),
  `eu_loses_from_soft_brexit` (:764, EU27 welfare +0.0049, wrong sign),
  `acr_formula_holds_under_costlier_borders` (:877, 8 countries of the wrong sign).
- e3: `openness_within_1_5_of_icio` (:847, class groups [3,5], [6], [0,4] and all traded goods
  outside [1/1.5, 1.5] of ICIO); `benchmark_cases_are_reported` and
  `acr_formula_holds_under_costlier_borders` (:782, index out of bounds, len 0, index
  1844674407370955264, a usize underflow).
- off: all 16 pass, but the "border x10" case panicked inside `ceteris-econ/src/world.rs:89` and
  was reported as a failed case, not a test failure.

## H2 distance elasticity (P; rerun after fetching the pinned Gravity and Natural Earth zips)

| column | elasticity (s.e.) | contiguity | pairs |
|---|---|---|---|
| off | -0.4720 (0.0107) | -0.1663 | 47,306 |
| e1 | -0.4720 (0.0107) | -0.1663 | 47,306 |
| e3 | -0.4720 (0.0107) | -0.1663 | 47,306 |

Outside the data's -1.1 to -0.7 in every column, and identical across the three anchor settings to
the 10th digit: the anchor does not move H2. The gate prints it as a diagnostic (ADR 0264); a
log-distance leg-cost component is the scheduled fix. Logs: `~/ceteris-logs/rerun2/`.

## Not completed

- `P_NU` (`--no-border-fit`): calibration failed, "the PIGL inversion over the pops of country 222
  did not settle in 1000 rounds" (miss 5.7e-12). `exp_nu` then failed, and every `bench_P_NU_*`
  died at benchmark.rs:705. Known issue, no fix.
- `P_ST` (storage on, ADR 0356): `ceteris calibrate --storage` ran 65 min and failed, "Sinkhorn
  did not converge in 100000 sweeps, residual 1.599e-13". New failure, no fix. Every storage-on
  RFC 0286 screen died at p2_gate.rs:111, "the calibration tables belong to other inputs".
- Brexit, ACR-IO and US-China #245 free-moment rows exist only in the logs' printed tables.

## Next

Calibration fixes for `P_NU` and `P_ST` and the Gravity fetch are separate work. Rerunning the
missing columns unchanged would fail the same way, since calibration is deterministic.
