# RFC 0350 bands at the fitted epsilon and theta_2 (#667, #668)

Status: measured 2026-11-24, 2000 pack recalibrated at epsilon 0.39003 and theta_2 9.0531
(`crates/ceteris-calibrate/fits/epsilon-d5e2.json`, 300 passes, residual 1e-13, 474 s), band
run `cargo test -p ceteris-calibrate --release --test benchmark -- --ignored` (335 s). Bands
were fixed in advance by RFC 0350 and are not moved. Logs: `~/ceteris-logs/calib-fitted-667.log`,
`~/ceteris-logs/bands-667.log` (owner Mac).

| Moment | Band | Default price rule | Result | E3 price rule (not scorable) |
|---|---|---|---|---|
| Hard Brexit, UK welfare | [-4.0%, -1.33%] | -0.758% | miss | -56% |
| Soft Brexit, UK welfare | [-2.0%, -0.67%] | -0.283% | miss | -54% |
| EU27 and Japan sign, border x1.1 | negative, both | JPN -0.496%, EU27 -2.15% | pass | JPN -1157%, EU27 -19.9% |
| ACR-IO, border x1.1 | no wrong sign, median [0.8, 1.25], correlation >= 0.9 | 0 wrong, median 1.129, corr 0.939 | pass | 14 of 22 wrong, median -2.77, corr -0.48 (58 NaN) |
| Openness per class vs ICIO | [1/1.5, 1.5] | 0.985 to 1.087 | pass | 2.56 to 3.20 |
| Border x10 | survives 1,500 ticks, index <= 1.07, gain within 0.5x to 2x ACR | panic "payment does not fit the ledger" | miss | tick 1112: price bounds violated, province 2463 wage 0 |
| US-China pass-through | [0.8, 1.2] | 0.941 (proxy) | pass | -6.2 |
| US-China welfare | [-0.1%, -0.02%] of GDP | USA +0.0044% | miss (wrong sign) | USA -16.7% |
| PPML theta spread, short-run elasticity | >= 3; [0.5, 1.5] | not run by this harness | open | not run |

E3 run (`CETERIS_PRICE_ANCHOR=e3`, `~/ceteris-logs/bands-667-e3.log`, 663 s, 3 tests passed and
`eu_loses_from_soft_brexit` failed): the unshocked base run is not stationary (max |ln p|
change over the last 50 ticks 4.9999, against 1.6e-7 under the default rule), so no E3 number is
a band score. The pack tables are solved under the default rule. Related: #561.

Border x10 under the default rule: the largest price ratio to the steady state is 20 at tick 10,
390 at tick 20 and 3.0e6 at tick 50, so the ledger overflow is a symptom of a price runaway.
Related: #216, #673. Not patched.

Misses: both Brexit welfare losses are about 0.3x to 0.4x of Dhingra et al., so the model
under-transmits border costs to welfare. Reasons are not established; no tuning was done. Next step is Antonio's call on model
structure (option 3, distance cost on domestic legs, is the candidate). Owner of each miss: Antonio.
