# 0271. Goods at the ICIO industry level

Status: proposed
Date: 2026-10-31
Design sections: 7.1, 7.2, 8.3, 8.6, 16.1, 19.2; amends ADRs 0023, 0092, 0256

## Context

DESIGN 7.1 makes the ICIO industries the goods. P2 built ten goods, aggregated from them
(`crates/ceteris-pack/src/econ/goods.rs`), so the core could be built and calibrated at a size
that can be read by hand. No phase brings the list back to the industries: the P3 plan (task
G2, ADR 0235) benchmarks a synthetic 45-goods pack, but no ADR or plan builds the real one.

Ten goods cost fidelity in two ways:

- **Recipes.** A merged good gives every producer the output-weighted recipe of all its
  industries, so an economy that runs only one of them gets the wrong one. Refining in good 1 is
  the case found (ADR 0270 draft): the merged world column has a_VA 0.418 against 0.666 for
  extraction, and gives rent economies 1.6 times their gross output and five times their own use
  of good 1. Good 5 (refining, chemicals, pharmaceuticals, plastics, minerals, basic and
  fabricated metals) and good 9 (twenty service industries) average more.
- **Trade elasticities.** theta is estimated per industry. Good 5 averaged refined fuel (theta
  64.85) with chemicals and metals (1.7 to 3.3) into 12.64 (ADR 0092), and every merged good
  carries some such average.

The ICIO 2025 edition has 50 industries: 3 agriculture, 5 mining, 19 manufacturing, D, E, F and
20 services. 27 are traded goods under the P2 rule, 23 local.

## Options

1. Keep ten goods to 1.0.
2. An intermediate list of about 20, splitting the worst-mixed goods (5, 6, 9).
3. The 50 ICIO industries, one good each, built in P4; DESIGN 7.1's physical splits (grains,
   crops and livestock; crude oil and gas; generation by fuel) stay with their own ADRs.
4. The 50 industries together with the physical splits, in P8.

## Decision

Option 3.

- **Goods.** One good per ICIO 2025 industry, mapped one to one in `icio_industry`; older
  edition names keep mapping to their parts as now. Traded: A01 to C31T33 (27). Local: D, E, F
  and G to T (23). DESIGN 7.1 is restated at 50.
- **Recipes.** `econ_io` becomes countries x 50 x 50, the country's ICIO 2000 columns without
  aggregation. The VA convention and the identity a_VA = 1 - sum_k a_kg are unchanged.
- **Economies outside ICIO.** Unchanged rules: WDI group shares split within each group by the
  donor's proportions, now over more goods. The rents rule of ADR 0249 applies to B06 (crude oil
  and gas) and ADR 0256's world column is the B06 column; with refining its own good (C19) this
  is the extraction column, so ADR 0270 becomes interim for the ten-goods pack.
- **Downscaling.** Each industry takes its group's proxy (cropland, GEM assets, mines and
  mineral sites, DOSE manufacturing GVA, population). DOSE has one manufacturing total, so all
  19 manufacturing industries share its province shares within a country. Finer proxies are
  later work, each by its own ADR.
- **theta.** Caliendo-Parro per good by the ADR 0023 rule, classes recomputed; ADR 0092's nu
  per class refitted; H2 rerun as the gate.
- **Weights and demand.** w_g per good from BACI 2000 through the OECD HS to CPA key; the PIGL
  categories of ADR 0021 by the same group rule (A agriculture and food, S D to T, M the rest).
- **Gate.** Starts after P3 task G2 passes. Trade dispatch scales with traded goods (27, inside
  the synthetic 45); local goods add to the same-tick chain, measured on the real pack. If the
  real pack exceeds 160 ms, ADR 0235's levers apply in order.
- **Phase.** P4, first task, before 0243. Estimate (a guess, not measured): 2 to 4 owner-weeks
  for the pack, recalibration, goldens and the benchmark rerun.

## Consequences

- Every econ golden, the p2-pack golden and the calibration move once, in one commit.
- The oil-rent problem goes away structurally: refining and extraction have their own recipes.
- Pack and state grow about 25 times in IO cells (100 to 2,500 per country) and 5 times in
  enterprises at most (about 4,594 x 50); the same-tick local chain runs over 23 local goods.
- Manufacturing location within a country stays one proxy until a finer source is found, so
  province specialization inside manufacturing is not data at t0.
- ADR 0248's P4 estimate grows by the cost above.
