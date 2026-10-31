# 0278. Storage order by partial adjustment toward a target stock

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

## As built

To be filled after the storage-on V1 runs.
