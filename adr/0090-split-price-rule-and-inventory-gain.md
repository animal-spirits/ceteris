# 0090. Split price rule and inventory gain

Status: proposed (owner chose the options, 2026-10-25); deferred to P5 (owner, 2026-10-30)
Date: 2026-10-25
Design sections: 8.2, 17 (P02, P03a, P03b); trade SPEC Origin market, Stability region; ADRs 0016, 0019

## Context

The price rule `dlnp = pi^e + kappa1 (D - Q)/Q - kappa2 (S' - S*)/S*` uses one gain for two
modes of very different speed:

- **Relative prices.** A small origin faces an order elasticity E = theta_g + 1, up to 11.57 at
  the current classes (13.55 at the first thetas). J2 caps kappa1 at about 0.17 for the
  theta-10.57 goods and 0.51 for food (trade SPEC, Stability region; 0.14 at theta 12.55), so one global kappa1 makes
  food and manufactures adjust 2 to 4 times slower than they could.
- **Price level.** The world price level faces only household demand (input orders are in
  units, money is fixed) and returns at kappa1 h, with h about 0.24. With pi^e the mode has a
  modulus of 0.994 to 0.996 per tick, a half-life of about 190 months.

With kappa2 = 0 the stock has a unit root (4.7 to 47 times target cover on the fixture), and
P02 has no effect.

## Options

(1) Keep one kappa1. (2) Split the excess-demand term into an elasticity-normalized relative
term and a world aggregate term, each with its own gain.

For the stocks of goods outside competitive storage (ADR 0091): (a) kappa2 = 0; (b) storage
loss only; (c) kappa2 > 0 plus a storage cost.

## Decision

Option 2, with inventory option (c) (owner, 2026-10-25).

With l_o = ln(D_o / Q_o), lbar its value-weighted mean over all markets, E_o = theta_g + 1 for
traded goods and 1 for local goods, and h the steady-state household share of orders:

```
dlnp_o = clip( pi^e + (kappa_r / E_o)(l_o - lbar) + (kappa_n / h) lbar
               - (kappa2 / E_o)(S'_o - S*_o)/S*_o,  -c_max, c_max )
```

- kappa_r (P03a, redefined): start 0.5. For a small origin kappa_r = 1 is one exact Sinkhorn
  step, since orders move with p^-(theta+1).
- kappa_n (P143): start 0.2. The price level returns at kappa_n per tick instead of kappa1 h.
- kappa2 (P03b): normalized like kappa_r; start set inside the re-derived region.
- lbar is one world reduction per tick, summed in a fixed order (determinism).
- h is fixed at its steady-state value, so the rule stays homogeneous of degree zero.

The stock of goods outside ADR 0091 (food 3, light manufacturing 4, machinery 6) also loses a
carrying cost: S' = (1 - delta_g) S + Q - sales, with delta_g the storage cost of P56 (ADR
0029), shared with ADR 0091. The loss is physical (destroyed, tallied as spoilage). Merchants
replace it, so the excess-demand ratio reads l_o = ln((D_o + delta_g S*_o) / Q_o), and at the
steady state output covers sales plus delta_g S* (ADR 0089 stage 1 adds it to V_o). Local
goods keep s_g = 0.

The as-built rule is the special case kappa_r / E = kappa_n / h = kappa1, kappa2 = 0, delta = 0.

## Tests

- Jury conditions re-derived for the normalized rule. J2 becomes
  kappa_r (1 + eta eps / ((2 - eta) E)) < 2 when kappa2 = 0: the same for every theta.
  `price_rule_stability.rs` checks 10 pairs inside and 10 outside per good, as now.
- Nominal mode: a 5% shock to the money stock returns the world index within 1% in under 30
  ticks at the defaults.
- Special case: with the gains mapped as above, state hashes equal the as-built rule over 120
  ticks.
- Stocks: from a 3x stock shock, cover returns to P02 within its half-life prediction.

## Consequences

- Stability no longer depends on theta, and the nominal half-life falls from about 190 months
  to about 3 ticks.
- Every market reads one world mean per tick: a global coupling, cheap but serial.
- "Merchants see world excess demand" is a modelling shortcut. P5 money and expectations
  (12.2) can replace the aggregate term with a micro-founded anchor.
- Register: P03a redefined, P143 added; trade SPEC Stability region rewritten.

## Trial before the P2 gate (2026-10-26), parked

The decision was built in full before the gate (PLAN Task 20e) and parked by owner decision. The code,
tests and records are on the local branch `p2-adr0090` (56b8e7f, ec17126), and the detail is in
`.superpowers/sdd/PLAN/task-20e-report.md`. p2-econ keeps the round 3 model.

- **kappa_r 0.5 is unstable on the pack.**
  - J2 limits on the pack's enterprises at kappa2 = 0 (default mode / frictionless): food 0.999 / 0.300,
    machinery 1.255 / 0.458. For local goods (E = 1) the frictionless limit goes down to 0.162, so at 0.5
    J2 fails for 1,237, 1,114 and 96 local markets of goods 7, 8 and 9, and for 12 food and 7 machinery
    markets.
  - In the default mode, rounding noise around the calibrated steady state grew to a 0.35 price drift by
    tick 120, in a province mode that one-market J2 does not see (province 3474: an oil producer with
    eps 24 and a_VA 0.07, its local goods, wage and employment).
  - Traded 0.25 with local 0.05 was stable in both modes for 300 ticks, and kappa2 by the midpoint rule
    was then 0.0591.
- **The nominal anchor fails.**
  - The mean of log excess demand turns a local real shortage into world inflation. From the fixture's
    data state, local services short by ln(D / Q) of 1 to 2 hold lbar up, and every price clips at
    +c_max. Relative prices then freeze, so the shortage never closes and the index explodes.
  - Money does not anchor the price level, since enterprises may run negative balances.
  - Fixture data state, default mode, world index at tick 60 and 1,200 (+ = without bound; kappa2 and
    storage loss on / off):

    | Measure | kappa_r | kappa_n | On | Off |
    |---|---|---|---|---|
    | log | traded 0.25, local 0.05 | 0.032 to 0.2 | + | + |
    | log | as built (0.05 E) | 0.032 | 1.79, 1.09 | 2.59, 1.06 |
    | log | as built (0.05 E) | 0.2 | + | + |
    | linear | traded 0.25, local 0.05 | 0.032 | 10.7, 1.09 | 26.6, 1.07 |
    | linear | either | 0.2 | + | + |
    | linear | as built (0.05 E) | 0.032 | 1.26, 1.09 | 1.25, 1.06 |

    kappa_n 0.032 is the as-built equivalent, kappa1 h with h = 0.64.
- **Pack data state** (ruled starts, frictionless):
  - The world index ran 1.22 at tick 0, 6.5 at 36, 1.92 at 120 and 1.42 at 360. The as-built run is
    2.38 at 120 and 2.02 at 360.
  - The partial gate, stopped by the owner decision, gave these results.
    - V1 from the data state: price 0.847, wage 0.593, flows 1.38, with up to 8,839 pops in negative
      money.
    - V1 from the perturbed steady state, which passes as built at 9.1e-3, missed at 0.177.
    - 100 years and the thread test passed.
- **What held.**
  - The special case (a linear measure with kappa_r / E = kappa_n / h = kappa1, kappa2 = 0 and no
    storage loss) reproduced the as-built state hashes over 120 ticks, which p2-econ records in
    `tests/golden/p2-as-built-rule.txt`.
  - The nominal-mode, stock-shock and thread tests passed on the fixture, and the storage-loss
    calibration held the fixed point.
- **Owner decision (2026-10-26).** Close the gate on the round 3 model and redesign the anchor after it.
  As Consequences notes, P5 money and expectations can replace the aggregate term with a micro-founded
  anchor.

## Deferred to P5 (2026-10-30)

**Owner ruling (2026-10-30, "Do the recommended"):** pre-P3 task B6 (the split rule rebuilt on the
P2 model) is deferred to P5. The as-built price rule stays through P3 and P4; the aggregate anchor
is redesigned around money in P5 (12.2), as Consequences anticipated.

**Why.** The split rule inflates from the data state, and no variant probed before P3 fixes it
(`.superpowers/sdd/PLAN-pre-p3-rust/b6-report.md`, `b6-options.md`).

- The input-stock refill is over by tick 3. The aggregate traded D/Q then rebuilds to 2.03 by tick
  15: a second wave that the rule makes itself. Its aggregate term (kappa_n / h, 0.41) pushes every
  price up together, and relative gains of kappa_r / E 0.006 to 0.026 are too slow to ration the
  orders this creates. No single gain removes it.
- Every variant fails:

  | Variant | Fixture, data state: world index max (tick) | Other result |
  |---|---|---|
  | Split rule as specified | 7.76 (23) | pack: Leontief stop at tick 15 |
  | A: household-only aggregate | 13.1 (123) | does not nest the as-built special case |
  | B: lower kappa_n (0.1, 0.05, 0.032) | 10 to 11 (0.1: i64 panic at 305) | fails the nominal shock test (37 to 57 ticks) |
  | C: money anchor ln(M / M0) - ln(I / I0) | 1.78 (660) | exact for nominal shocks (8 ticks); traded-versus-local level drifts, NaN price at 682 |
  | D: start from the calibrated steady state | flat | stock shock (3x merchant stock) spikes 1.22 (fixture), 1.55 (pack) |
  | Traded kappa_r 0.15 to 0.05 | Log: diverges (i64 panic about 195); Linear: 6.5 to 6.9 | pack: Leontief stop at 15 or 16 |

  For reference, the as-built rule peaks at 1.18 on the same run.
- C is the only principled nominal anchor, but it needs a separate, slow traded-versus-local term:
  a design change for P5, not a gain.

The work in progress is parked on local branch `pre-p3/b6-wip`. The as-built rule is unchanged.

**The x10 border shock is beyond P2's valid range (owner ruling 2026-10-31T01:37Z, "A").** Every
crossing's tau x 10, frictionless, on the scratch pack (pre-P3 v1 diagnosis and write-off
round, `.superpowers/sdd/PLAN-pre-p3-rust/v1-diagnosis.md`, `writeoff-fill-report.md`), does not
converge. One producer's own-good input draws all of its province's labour, the other
producers fall to near 0, local inputs run out, and from tick 18 supply is 0 nearly everywhere.
Prices with no supply rise at the clip every tick, the Walrasian wage follows them, and the
wages of enterprises that produce nothing are paid from overdraft. The world index doubles
about every 2.5 ticks after tick 40. Nothing in the P2 model anchors the nominal level, so this
is the missing anchor of the section above, not a defect of the chain, the write-off or the
dispatch. After the i128 write-off total (ADR 0106) and the stock floor (ADR 0093) the run
passed the write-off and stopped at tick 54 on the dispatch-mass invariant. That stop was a
tolerance defect: flows computed through partial products below the smallest normal f64
(trade SPEC, Invariants). With the tolerance fixed, the run stops later on a payment that
does not fit the ledger (the econ SPEC records where). No restart floor or other new mechanism
is added for it in P2 (option B, not taken).

Known limits left for P5, where the money anchor is designed:

- the ADR 0207 x10 benchmark point: it is reported (`benchmark_cases_are_reported`), not
  asserted, until P5;
- shocks that collapse a country's output: the same no-anchor spiral, wages paid for no output
  from overdraft;
- the structured perturbations that still overflow the ledger or market cash: 4 of 24 in the
  default mode and 3 of 24 frictionless (write-off round, 2026-10-30).
