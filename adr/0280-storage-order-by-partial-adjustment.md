# 0280. Storage order by partial adjustment toward a target stock

Status: proposed
Date: 2026-10-31
Design sections: 8.2, 12.2, 17 (P56, P144 to P146); amends ADR 0091; trade SPEC Competitive
storage, Storage loop

## Context

ADR 0091's merchants order b = delta S + psi D x, x = pi^e + (k + delta)(z^-zeta - 1), z = S /
(s D), floored at -(1 - delta) S. Every check of that rule (the Storage loop's 4 x 4 map, J's
storage factor (1 + psi zeta (k + delta)) / (1 + delta s), the psi limit, the block map) is a
linearisation at z = 1. Storage-on V1 on the B8 pack (scratch stv1, 6,000 ticks) failed away
from it: frictionless runs from the perturbed state overflowed the ledger at ticks 51 to 53 for
seeds 20 to 22, the data state at tick 43 (frictionless) and 82 (default); the default perturbed
runs ended with flows 2.2% to 3.3% off. The diagnosis found no defect in the code; the rule
itself leaves its linear region:

- **The stockout yield is unbounded.** The order's response to buyers' orders is psi (x + c
  z^-zeta), c = zeta (k + delta), so the J factor 1 + psi c z^-zeta grows without bound as cover
  falls. At kappa1 0.05 (J2 = 40), with Algeria's fuel market (E 11.24, eps 0.9, eta 1), J
  crosses J2 at cover 0.81 for fuels (zeta 13), 0.38 for agriculture, 0.40 for coal and mining
  and 0.43 for chemicals and metals: a stocked-out market is outside the region its per-market
  cap was set for.
- **A dead market bids its own price up.** With no buyers (D = 0) the rule orders delta S, and
  where output is also 0 the price step reads delta S / Q_floor, positive, so the price rises at
  the P151 clip (0.3 a tick) while stock spoils and nobody buys.
- **The perturbation reaches the nonlinearity.** V1's 10% perturbation at theta 17.6 is a 5.8x
  swing in buyers' orders between competing origins; 401 of 1,578 good-1 cells stocked out at
  tick 1, and their orders, unbounded through z^-zeta, sent the next price steps to the clip.
- **Frictionless is worse.** At eta 1 output can fall to 0 in one tick, which makes dead markets,
  and the Walrasian wage passes the price spikes to the province.

A toy check: Algeria's fuel market at its target with buyers' orders doubled for one tick peaks
at ln p 0.600 (two clipped steps), ten times the price at which the doubled orders clear against
output alone (ln 2 / 11.24 = 0.062), where Deaton and Laroque (1992) put a stockout's ceiling.

## Options

- **A. Partial adjustment** (Lovell 1961): b = delta S + lambda (S\* - S), S\* = s D max(0, 1 +
  pi^e / (zeta (k + delta))), lambda = psi zeta (k + delta) / s. S\* is the cover at which the
  expected net return is zero to first order in pi^e (Working 1949), and lambda is the
  stock-persistence rate 1 - rho of Gorton, Hayashi and Rouwenhorst (2013) from which P144 psi was
  derived, so there is no new parameter. Same linearisation at the steady state as ADR 0091's
  rule; the order is linear in S and D, so its response to orders is bounded everywhere. On the
  stv1 pack every run exits 0 (240 ticks); about 49 good-1 cells keep a bounded cycle of +-0.3.
- **B. A curved Working target**: keep x, but with a convenience yield bounded as cover falls
  (a supply-of-storage curve with a finite intercept). Needs a new curvature parameter.
- **C. A mean-reverting expectation** (ADR 0091 option c): pi^e pulled toward 0. Addresses the
  momentum, not the stockout yield or the dead market.
- **D. Clamps**: cap z^-zeta or the order. Numerical, not from the literature.

## Decision

Option A (owner, 2026-10-31).

```
S*_og = s_g D_og max(0, 1 + pi^e_og / (zeta_g (k_g + delta_g)))
b_og  = max( delta_g S_og + lambda_g (S*_og - S_og),  -(1 - delta_g) S_og )
lambda_g = psi_g zeta_g (k_g + delta_g) / s_g
```

lambda is 0.144, 0.15, 0.05 and 0.067 a month for goods 0, 1, 2 and 5 at the approved values,
the persistence targets psi was derived from (DESIGN 17, P144). With no buyers S\* = 0, so the
merchants release lambda S a month, supply the price step sees. At pi^e = 0 and S = s D the
order is delta S, as before. The floor stays; at lambda below 1 it does not bind.

## Consequences

- The steady state, the Storage loop's map, J's storage factor, the psi limit and the block map
  are those of ADR 0091 to first order; nothing calibrated moves. Storage stays off by default,
  so no golden hash moves.
- `TradeParams::storage_return` (x) goes; `storage_adjustment` (lambda) and `storage_target`
  (S\*) replace it.
- The stock no longer keeps a pipeline floor through the convenience yield: a long cut draws it
  down geometrically at lambda (agriculture's 12-month 30% cut to about 0.13 of target).
- ADR 0091's gate for switching storage on is unchanged.

## As built (pre-P3 B8f, 2026-10-31)

- **Code** (31116a4): `Markets::store` orders delta S + lambda (S\* - S) floored at -(1 - delta)
  S, with `TradeParams::storage_adjustment` (lambda) and `storage_target` (S\*);
  `storage_return` and the order's Q_floor and D = 0 branches are gone. Storage stays off by
  default: every golden hash holds (core, geo and econ goldens and p2_acceptance at RAYON 1, 3
  and unset; workspace 787 passed, 0 failed, 16 ignored on the installed pack).
- **Tests.** `stockout_spike_stays_below_flow_clearing`: fuels at E 11.24, eps 0.9, eta 1,
  orders doubled for one tick, peak ln p 0.0574 against ln 2 / E = 0.0617 (0.600 under the first
  rule). `stock_without_supply_or_buyers_does_not_bid_the_price_up`: the dead cell's price stays
  at or below 1 (e^3 = 20.1 after 10 ticks under the first rule). `storage_never_sells_below_zero`
  expects the release delta S - lambda S at pi^e -10, and `storage_conserves_goods_and_money`
  (econ) checks the same release bit for bit on the fixture at tick 0 (the floor no longer binds
  there). `partial_adjustment_keeps_the_storage_loop` (calibrate): for goods 0, 1, 2, 5 the toy
  market's tangent map over 1 and 5 ticks, by central differences at 1e-6, equals
  `storage_matrix` and its fifth power to 1e-7; it passes under both rules. The pinned values of
  J's storage factor, the psi limits (43.5, 2.1, 22.35, 26.5), the kappa1 limits of the storage
  table and the block map with storage on (`block_limits_with_storage_on`) are unchanged, since
  they are computed from the linearisation. ADR 0091's harvest tests hold: 12-month cut rise
  0.356 against surplus fall 0.266, 3-month rise 0.086 against a bound of 0.297, lowest stock
  0.131 of target.
- **Storage-on calibration of the 2000 pack** (scratch export of 31116a4 with a storage-on
  switch, never committed; a copy of the installed pack, manifest 3c7895d1, B8e tables): settles
  in 93 passes (last change 3.05e-14, Sinkhorn residual 9.95e-14; 64 min wall beside other jobs),
  nu 2.5942, 0.7999, 0.3956, 0.0660 and 0 (clamped), the storage-off fit to the third decimal.
  GE reference residual 8.3e-14 at the calibrated point.
- **Storage-on V1, 6,000 ticks** (largest relative gap of price, wage, flow units and flow value;
  V1 tolerance 0.01; Pitcairn moves only the 1e-7 tail, so the values are the same without it
  where they matter):

  | Start | 1,200 | 3,600 | 4,800 | 6,000 | Worst cells |
  |---|---|---|---|---|---|
  | default, seed 20 | 9.7e-5 | 8.9e-8 | 1.2e-7 | 2.0e-7 | MWI local goods, then PCN |
  | default, seed 21 | 5.9e-5 | 9.5e-8 | 9.3e-8 | 2.3e-7 | MWI, MLT; PCN at 6,000 |
  | default, seed 22 | 4.4e-5 | 9.5e-8 | 1.6e-7 | 1.1e-7 | MWI, PCN |
  | default, data state | 3.2e-3 | 1.2e-7 | 1.1e-7 | 1.2e-7 | MWI, MLT |
  | frictionless, seed 20 | 0.478 | 0.391 | 0.477 | 0.254 | Switzerland, good 1 |
  | frictionless, seed 21 | 3.6e-5 | 1.3e-7 | 2.2e-7 | 8.8e-8 | MWI, PCN |
  | frictionless, seed 22 | 2.4e-3 | 1.2e-7 | 3.1e-7 | 1.1e-7 | MWI, CHE |
  | frictionless, data state | 2.79 | 1.13 | 0.885 | 6.99 | France, province 1094 |

  Every run exits 0 (the first rule panicked on ledger overflow at ticks 43 to 82 in five of
  these eight runs and left the default perturbed runs 2.2% to 3.3% off in flows at 6,000).
  Six of eight pass at 1,200 ticks and stay within 3.1e-7 from 3,600 on.
- **Misses (reported, not fixed).** (1) Frictionless seed 20: a good-1 cycle in Switzerland's
  19 to 20 good-1 cells persists to 6,000 (price gap 0.069, 0.306, 0.376, 0.168 at the four
  checkpoints; wages 0.216 down to 0.024; flows into Switzerland 0.45 down to 0.09 in value),
  bounded and not decaying. The other residual cycle countries of the diagnosis, Slovenia and
  Senegal, are below 2e-3 in every run and below 1e-9 at 6,000 outside the frictionless data
  state. (2) Frictionless data state: France's province 1094 has prices of its local goods 7, 8,
  9 off by 0.87 to 1.13 and its wage by 0.85 to 1.12 at every checkpoint, and its good-1 exports
  carry the flow gap (2.8 at 1,200, 7.0 at 6,000); at 3,600 84 good-1 cells in 17 countries
  (Afghanistan 34) are more than 1% off, at 4,800 and 6,000 one. Storage off, the frictionless
  data state fails too (B8e: 0.762 at 1,200, Taiwan; 0.295 at 6,000, Albania), with different
  leaders. Neither is diagnosed here.
