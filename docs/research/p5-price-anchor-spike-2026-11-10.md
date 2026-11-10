# P5 price-anchor spike (Branch E1), 2026-11-10

Spike before build (development laws) for ADR 0090's nominal anchor. Fixture only: the pack is
built owner-side and was not available in the cloud. Code: project files
`p5-anchor/spike.patch` (a switchable `reprice` in `ceteris-econ`, harness
`tests/anchor_spike.rs`; not for merge). All runs on main at 3a8e926, 1,200 ticks, one thread.

## Rule tried

With l_o = (D_o + b_o - Q_o) / Q_eff (the as-built gap), lbar_g the value-weighted mean of l_o
over the active origins of good g (weights p (Q + D) / 2), lbar the same over all markets, I the
world price index of V1 and M the money issued by the mint:

```
dlnp_o = pi^e + clip( a_o (l_o - lbar_g) + kappa_b (lbar_g - lbar)
                      - kappa_m (ln(I / I0) - ln(M / M0)) [- kappa2 stock term], -c_g, c_g )
```

- a_o: "cell" = the as-built per-cell gain kappa1_cell (ADR 0263/0267); "split" = kappa_r / E_g
  with kappa_r 0.25 traded (E = theta + 1) and 0.05 local (E = 1).
- kappa_b: good-level (across-goods) term. kappa_b = 0 is ADR 0090's variant C.
- kappa_m: money anchor, replacing ADR 0090's (kappa_n / h) lbar.
- pi^e on (1) or off (0). I0, M0 are the values at the first repricing.

## Results

Fixture data state (V1 = max relative deviation from the GE reference after dividing by I):

| Rule [a, kappa_b, kappa_m, pi^e] | Mode | max I (tick) | I at 1,200 | V1 price / wage | last 50 max abs dlnp |
|---|---|---|---|---|---|
| as built | frictionless | 1.065 (8) | 1.033 | 1.9e-6 / 6.4e-7 | 9.1e-8 |
| as built | default | 1.064 (14) | 1.033 | 5.2e-6 / 3.5e-6 | 2.2e-7 |
| cell, 0, 0.1, on (variant C) | both | 1.018 (2) | 1.018 | 0.30 / 0.096 | 2e-9 |
| cell, 0, 0.2, off | both | 1.018 (2) | 1.018 | 0.30 / 0.096 | 1e-13 / 2e-10 |
| split, 0, 0.2, off | both | 1.018 (2) | 1.018 | 0.30 / 0.096 | 9e-9 / 2e-8 |
| cell, 0.05, 0.2, off | frictionless / default | 1.018 | 1.018 | 3.6e-3 / 1.8e-3 | 6.9e-11 / 7.5e-4 |
| split, 0.02, 0.1, off | frictionless / default | 1.018 | 1.018 | 3.6e-3 / 1.8e-3 | 3.0e-7 / 2.5e-4 |
| cell, 0.02 or 0.05, 0.1, on | frictionless / default | 1.018 | 1.018 | not taken | 1.4e-5 to 1e-3 |

Nominal shock (5% of every positive pop balance minted at the calibrated steady state; ADR 0090
test: index within 1% of the new level in under 30 ticks): as built 77 / 81 ticks (fails);
kappa_m 0.1 with pi^e 15 / 16 ticks; kappa_m 0.2 without pi^e 9 ticks; split, kappa_b 0.02,
kappa_m 0.1, no pi^e 16 / 17 ticks. Every anchored variant passes; the overshoot with pi^e on is
0.5% (1.0524 at tick 60).

x10 border shock on the fixture (every crossing's non-tariff iceberg 10, from the calibrated
steady state; the P5 criterion runs on the pack): as built, I reaches 5.4e5 and stops at tick 611
(frictionless) or ends at 5.6e5 still moving 0.31 per tick (default). Every anchored variant keeps
max I at or under 1.038 but then deflates to I near 0 and stops on the `price bounds` invariant
between ticks 54 and 1,110 (one payment overflow at 98).

## Reading

1. The money anchor fixes the nominal mode. No variant inflates from the data state, and the
   nominal shock returns in 9 to 17 ticks against 77 as built. ADR 0090's variant C failure on the
   old fixture (NaN at 682) does not recur on today's fixture.
2. Variant C is not enough: with kappa_b = 0 the relative term sums to zero within each good, so
   nothing moves the price of one good against another, and relative prices freeze 30% off the
   reference. That is the "traded-versus-local drift" of ADR 0090. A good-level term at
   kappa_b 0.02 to 0.05 brings V1 to 3.6e-3 by 1,200 ticks; the mode is slow (elasticity across
   goods is about the household share, inputs being Leontief), and in the default mode it is still
   moving (2.5e-4 to 7.5e-4 per tick). As built reaches 2e-6.
3. pi^e on top of a level anchor adds a slow oscillation in the default mode; without pi^e the
   level target alone carries the nominal mode.
4. The x10 failure is real, not nominal. The anchor stops the inflation spiral but output still
   collapses (ADR 0090, x10 ruling: own-good inputs draw all labour, supply goes to 0), and with
   no supply the clipped steps drive prices to the floor. This is RFC 0286's price step at Q = 0,
   not the anchor. No anchor gain fixes it.

## Go / no-go

- Money anchor plus a good-level term: **go** for the nominal mode (data state, nominal shock),
  on the fixture. Pack data state untested (needs the Mac pack).
- Same rule for the P5 x10 criterion: **no-go**. The criterion needs the Q = 0 step (E2,
  RFC 0286) redesigned first, or it stays reported.
- Default-mode convergence (V1 3.6e-3, still moving) is short of the as-built 2e-6: the
  good-level mode needs either a faster gain inside its stability region or a derived gain
  (elasticity across goods), measured before a build.
