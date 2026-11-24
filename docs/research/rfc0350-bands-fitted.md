# RFC 0350 bands at the fitted epsilon and theta_2 (#667, #668)

Status: measured 2026-11-24, 2000 pack recalibrated at epsilon 0.39003 and theta_2 9.0531
(`crates/ceteris-calibrate/fits/epsilon-d5e2.json`, 300 passes, residual 1e-13, 474 s), band
run `cargo test -p ceteris-calibrate --release --test benchmark -- --ignored` (335 s). Bands
were fixed in advance by RFC 0350 and are not moved. Logs: `~/ceteris-logs/calib-fitted-667.log`,
`~/ceteris-logs/bands-667.log` (owner Mac).

| Moment | Band | Measured | Result |
|---|---|---|---|
| Hard Brexit, UK welfare | [-4.0%, -1.33%] | -0.758% | miss |
| Soft Brexit, UK welfare | [-2.0%, -0.67%] | -0.283% | miss |
| EU27 and Japan sign, border x1.1 | negative, both | JPN -0.496%, EU27 -2.15% | pass |
| ACR-IO, border x1.1 | no wrong sign, median [0.8, 1.25], correlation >= 0.9 | 0 wrong, median 1.129, corr 0.939 | pass |
| Openness per class vs ICIO | [1/1.5, 1.5] | 0.985 to 1.087 | pass |
| Border x10 | survives 1,500 ticks, index <= 1.07, gain within 0.5x to 2x ACR | panic "payment does not fit the ledger" (ledger overflow) | miss |
| US-China pass-through | [0.8, 1.2] | 0.941 (proxy) | pass |
| US-China welfare | [-0.1%, -0.02%] of GDP | USA +0.0044% | miss (wrong sign) |
| PPML theta spread, short-run elasticity | >= 3; [0.5, 1.5] | not run by this harness | open |

Misses: both Brexit welfare losses are about 0.3x to 0.4x of Dhingra et al., so the model
under-transmits border costs to welfare. Border x10 overflows the i64 ledger on a payment of
9.3e14. Reasons are not established; no tuning was done. Next step is Antonio's call on model
structure (option 3, distance cost on domestic legs, is the candidate). Owner of each miss: Antonio.
