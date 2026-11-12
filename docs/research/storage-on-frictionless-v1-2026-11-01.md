# Storage-on frictionless V1 misses (RFC 0280 "Misses"): diagnosis and fix screen, 2026-11-01

Setup: B8f scratch reused (storage-on pack copy, manifest 3c7895d1, B8f tables; storage-off B8e
tables), code 31116a4 plus the stv1 storage-on switch and a probe, on Antonio's Mac (worktree
../ceteris-stv1diag, local branch claude/storage-misses-diag-31116a4; commits cb137ea, 55fb84e,
5544098, 30fa4aa, 10bad36, 371677e, ab10410; nothing pushed). Full tables and commands:
../ceteris-stv1diag/target/diag/REPORT.md on the Mac. Reproduction exact: frictionless seed 20 at
1,200 ticks 0.4777; every other checkpoint of the RFC 0280 table matches.

## Cause of both misses

At a stored good's origin with Q = 0 (frictionless eta 1 takes output to 0 in one tick) the gap
divides by Q_floor (1e-9 of the world mean), so every step hits the 0.3 clip. At theta 16.9 a
clipped step moves orders between near-identical origins by e^5; output collapses, the stock
overshoots (28x target in Switzerland), and its release drives the next collapse.

- Switzerland, frictionless seed 20: a nonlinear cycle beside a stable fixed point. Tangent map
  at the fixed point: spectral radius 0.642 (prices), 0.994 = 1 - g_s with S and pi^e. Shocks of
  1e-4, 1e-2, 0.1 to the converged seed-21 state decay at about 0.99 a tick; seed 22 rides the
  same cycle to about tick 1,100, then decays to 1e-12. In the cycle Q = 0 in 12% of cell-ticks,
  S = 0 in 10%, clip binds 17%. Stored block limit infinite; per-market cap kappa1_cell J = 1.
- France 1094, frictionless data state: the same cycle on its good-1 exports (period about 3
  ticks, Q = 0 in 44 of the last 101 ticks); labour swings between good 1 and services, wage and
  local goods ride at about 2x reference. The frictionless wage is the unique root from tick 9
  (the flat-stretch hypothesis is refuted). Storage off, 1094 converges to 5e-5.
- Ablations on seed 20 (2,400 ticks): storable clip 0.1 or g_s 0 each converge.

## Fix screen

| Round | Rule (stored origins unless noted) | Seed 20 frictionless | Data state frictionless | Other |
|---|---|---|---|---|
| 1 | gap over max(Q, D + b, Q_floor) | 3.5e-5 at 1,200 | ledger overflow tick 136 (FRA 1102) | 7 of 8 storage-on V1 pass |
| 1b | gap over max(Q, D) | Swiss cycle gone | overflow tick 133 (CHE 526) | |
| 2 | old rule, clip 0.1 | 3.3e-5 | overflow tick 123 | |
| 2 | old rule, clip 0.15 | 0.151 (Senegal) | overflow tick 124 | |
| 2 | bounded on excess side only | 0.297, cycles SVN SEN PRT | 0.564, exit 0 | |
| 2 | bounded on shortage side only (V4) | 3.5e-5 | overflow tick 135 | |
| 3 | V4 + local gap over max(Q, D) (T1) | 2.8e-5 | exit 0, 0.54 at 1,200, 1.5e-7 at 6,000 | all 8 storage-on starts converge; storage-off data states break; fixture spiral reopens |
| 3 | V4 + local caps in frictionless wage root (T2) | 3.4e-5 | overflow tick 387 | |
| 3 | T1 + T2 (T3) | 2.8e-5 | exit 0, 0.45 at 1,200 | 10 fixture tests fail |

- The shortage side drives the Swiss cycle; any slower shortage step lets the data state fall
  into a local-goods trap.
- The trap (France 1102 under V4, confirmed by a break test): stored inputs rationed for about 18
  ticks; local good 9's price runs up; the frictionless wage is set by good 9 alone at the whole
  labour force; goods 7 and 8 buy good 9 for about 60% of cost, their value added is squeezed,
  L* = 0, Q7 = 0; good 9's Leontief chain needs good 7, so Q9 stays near 0 and its price clips +0.1
  every tick; all three prices and the wage climb at the clip, so relative prices freeze. Holding
  goods 7 and 8 at L0 for ticks 18-30 frees 1102 by tick 50, but France 1106 and Switzerland 526
  enter the same trap (overflow tick 155). It needs the frictionless wage; the default mode never
  enters it.
- The storage-off data-state failures are other mechanisms: Taiwan 3903 an export-led labour
  shift (wage 0.15 to 0.32 w_ref to tick 300, back to 1.0000 by 6,000); Albania 62 a second
  stationary state or very slow convergence (D = Q everywhere, wage 0.788 at 6,000).
- T1 and T3 against storage off (B8e, 1,200 ticks): frictionless data state T1 overflows before
  tick 100, T3 1.0 (Uganda 3958), baseline 0.76; default data state both 0.70 (Rwanda), baseline
  9.3e-3. Fixture (econ + trade, baseline 261 passed): T1 6 failures
  (frictionless_labour_spiral_is_bounded and frictionless_price_sweep_stays_bounded overflow, the
  golden hash, a pinned rationed-tick list, two market tests of the old local gap); T3 10
  (spiral index 40.6 against 3, natural-rate, fixed-point and stability tests).
- partial_adjustment_keeps_the_storage_loop misses by 1e-5 under the round-1 rule: central
  differences straddle the max kink at the steady state; one-sided slopes agree.

## Reading

Every round that slowed a shortage price step to remove a cycle moved the failure elsewhere:
stored goods, then local goods, then the storage-off starts. A fix that holds is a redesign of the
price step at Q = 0 for every good, judged on all sixteen V1 starts and the fixture's spiral and
sweep tests, not a storage patch.
