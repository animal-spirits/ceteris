# 0279. Leg paths minimise the full iceberg cost per good

Status: accepted (owner, 2026-11-01: "Paths minimise full cost"; spec approved, code after B8)
Date: 2026-11-01
Design sections: 5.3, 5.4 (generalized trade cost), 8.3 (two-level kernel), 17.2 (S05), 19.4;
amends the least-cost legs of S05 as built (geo SPEC, Equations; trade SPEC, Leg factors);
ADRs 0017, 0022, 0247, 0269, 0271, 0277

## Context

S05's legs take the least-cost path by freight cost per tonne c, then price the leg as
`tau = 1 + c w_g + lambda_g T`. The time cost lambda_g T enters the price but not the choice of
path. DESIGN 5.4 writes the minimum over paths of the whole cost, time included: carrying cost
`p r T` sits inside the minimum. Choosing on c alone has two effects:

- **Air is never chosen.** Sea costs 0.008 to 0.012 USD per tonne-km and air at least 0.10, so
  the cheapest-by-freight path between two airports goes to a port and sails. The first
  ADR 0277 run on the pack (US 2000 imports, 1,116 origin-good cells) found no cell with an air
  crossing at P04d = 8 times road. Air's share is zero at any plausible P04d, so neither gravity
  (ADR 0022) nor mode shares (ADR 0277) can identify it. The airport gateways duplicate ports.
- **One path for every good.** Paths are the same for coal and machinery, although their time
  cost per tonne differs by orders of magnitude (lambda_g / w_g).

## Options

- **A. Airport pairs fly.** Price airport-to-airport crossings on the air network only. A
  smaller change, but it fixes air alone and keeps choice on c elsewhere. Not picked.
- **B. Paths minimise the full iceberg cost per good** (owner pick). Per traded good, a leg's
  path minimises `sum_e (w_g c_e + lambda_g h_e / 24)`, which is `tau - 1`.
- **C. Keep S05 and defer air to P9.** Not picked.

## Decision

B. For each traded good g, every leg of the kernel takes the path that minimises its iceberg
factor:

```
tau_leg,g = 1 + min over paths of sum_e ( w_g c_e + lambda_g h_e / 24 )
```

c_e and h_e are the S05 edge price and hours (geo SPEC). Fees and handling hours are on access
edges, as now. Domestic, access and crossing legs all use it.

- **Trees per time-to-weight ratio.** The minimiser depends only on kappa_g = lambda_g /
  (24 w_g), the value of an hour per tonne in USD. So goods with equal kappa share trees. With 7
  traded goods that means 7 sets of trees. ADR 0271's 50 goods would multiply that. The kernel
  then groups goods on a log grid of kappa, if the build budget below is missed. That grouping
  is a later owner call, not part of this ADR.
- **What stays on S05.** The freight-cost map mode and `PathCache` keep the freight-cost trees
  (c only), because they are goods-agnostic. Payments are unchanged: freight is paid as c per
  tonne along the chosen path, and the time cost is iceberg.
- **Calibration follows.** `ceteris_calibrate::costs::country_costs` and `air_share` build the
  same per-good trees, so gravity (ADR 0022) and the air fit (ADR 0277) see the kernel's paths.
- **Equilibrium nesting is unchanged.** The reference model reads leg taus. The per-good paths
  only change which tau it reads.
- **Switch and ablation.** `TransportParams::path_choice` takes `Freight` (S05 as built) or
  `Iceberg` (this ADR). Ablation test (DESIGN 15.7): V1, H2 and the P2 gate on both settings, on
  the 10-goods pack, with the difference reported. `Iceberg` becomes the default only after it
  passes, because ADR 0277's P04d is fitted on it. Until then it stays off, and P04d stays at
  ADR 0269's value.

## Consequences

- Air becomes a real choice: light, time-sensitive goods on long legs fly where w_g c_air +
  lambda_g h_air / 24 is lowest. Then the ADR 0277 fit can run.
- Kernel build cost grows about 7 times (one tree set per traded good, 4,594 province and 600
  gateway trees each). Budget: `kernel_build` 2 s (trade SPEC). Measured before the default
  changes. If it is missed, the options are the kappa grid above or parallel trees, which keep
  the slots fixed for determinism.
- Goldens: `tests/golden/p1-geo.txt` (freight trees) does not move. Every econ golden that
  prices legs moves when the default changes. The commit and the CHANGELOG then carry
  `### BEHAVIOUR: leg paths minimise the iceberg cost per good (ADR 0279)`.
- Timing: the build touches `ceteris-geo` paths, `ceteris-econ` `network_from_graph` and
  `ceteris-calibrate` costs, which pre-P3 B8 is changing now. So code starts on main once B8
  lands, and the default and P04d land together at the next rebuild that re-blesses the goldens
  (ADR 0277, Build).
- Route choice over edges (ADR 0017) stays deferred. This is still a hard minimum, so shares
  move at thresholds. The gateway-pair sum smooths them across gateways, which is what the
  ADR 0277 fit uses.
