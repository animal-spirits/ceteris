# Changelog

## Unreleased

### BEHAVIOUR: P2 economic core migrated onto ADRs 0015 to 0023

Model changes (each with its ADR; the econ and trade specs carry the equations):

- 0015 Trade flows in value with a dimensionless gravity kernel: destinations spread expenditure
  over origins by Eaton-Kortum shares on K p_o^-theta, buyers pay the origin price (with tariff)
  at dispatch. Tariffs sit inside destination spending.
- 0016 Origin markets clear across ticks: orders are filled against output and merchant
  inventory, the unfilled part carries as demand_int / demand_hh. `route.rs`, `transit.rs` and
  `Kernel::access` are gone.
- 0017 Route-choice trade costs: deferred past P2 (owner, 2026-10-24). S05 least-cost legs stay
  the default and `Network.route_taus` stays `None`; the module stays in the tree, tested and off.
- 0018 and 0023 Sector trade elasticities: P01 is a theta per traded good, in classes food 2.62,
  goods 4 and 6 5.88, goods 0, 1, 2, 5 12.55 (Caliendo and Parro 2015, 99% sample column).
- 0019 Expected inflation in the price and wage rules (P45 gain 0.006).
- 0020 Putty-clay coefficients and a same-tick chain for local goods; rationing modes include
  S11 Mixed (Pichler and Farmer 2022, Algorithm 2).
- 0021 PIGL household demand over three categories (goods 0 and 3; 1, 2, 4, 5, 6; 7, 8, 9) with
  lognormal pop income, and a consumption rule (P46, P47).
- 0022 Uniqueness check (AAL 2024 Theorem 1) before V1, static nesting gate, direct estimation of
  theta, border friction and mode costs (estimation lands with PLAN Task 18).

Owner decisions (2026-10-24):

- Order-up-to rule on inventory position (stock + on order) with target (1 + cover + L) months
  of use.
- Buyers use the delivered unit price q_d = 1/alpha_d in household demand, input orders and p_VA.
- Route choice deferred past P2 (ADR 0017).
- kappa1 0.05 (P03a) and gain 0.006 (P45); kappa2 0 unchanged.
- S11 Mixed rationing mode added.

Goldens re-blessed (`CETERIS_BLESS=1`): `tests/golden/p2-econ.txt` (every state hash changes:
new state layout, sourcing, markets and pipelines) and `tests/golden/p2-fixture/` (export format
2, per-good theta, delivered-price inputs, PIGL parameters, new `pigl_check.arrow`, and the
reference solution of the new model). The three tests held back since Task 1
(`p2_fixture_matches_golden_hashes`, `fixture_export_is_current`,
`test_fixture_reference_is_current`) run again.

V1 maxima from the blessed run (fixture, frictionless vs reference): price 3.14e-5, wage 2.54e-5,
flow units 5.80e-5, flow value 7.57e-5 (tolerance 1e-2). A small deviation from the equilibrium
decays by 0.99644 per tick frictionless and 0.99676 in the default mode.

Stability of the price rule (specs/trade/SPEC.md, Stability region): first kappa2 that fails, with
the kappa1 limit as kappa2 goes to 0, default mode / frictionless. The default (kappa1 0.05,
kappa2 0) lies on the kappa2 = 0 edge, which counts as inside.

| Good | theta_g | s_g | kappa1 limit | kappa2 limit at kappa1 0.05 (default) | at kappa1 0.02 |
|---|---|---|---|---|---|
| 0 Agriculture | 12.55 | 3 | 0.144 / 0.130 | 0.083 / 0.077 | 0.109 / 0.105 |
| 1 Crude oil and gas | 12.55 | 1 | 0.145 / 0.132 | 0.049 / 0.059 | 0.016 / 0.080 |
| 2 Coal and other mining | 12.55 | 2 | 0.144 / 0.130 | 0.077 / 0.071 | 0.040 / 0.097 |
| 3 Food | 2.62 | 2 | 0.507 / 0.368 | 0.392 / 0.347 | 0.040 / 0.380 |
| 4 Light manufacturing | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |
| 5 Chemicals, refined fuels, metals | 12.55 | 2 | 0.143 / 0.124 | 0.077 / 0.068 | 0.031 / 0.095 |
| 6 Machinery, electronics, vehicles | 5.88 | 2 | 0.278 / 0.230 | 0.189 / 0.173 | 0.040 / 0.202 |
