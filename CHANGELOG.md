# Changelog

## Unreleased

### BEHAVIOUR: trade elasticities follow the OECD key (ADR 0023 amendment)

`TradeParams::default()` moves from the classes of the WITS and UNSD route to those the ADR 0023 rule
draws from the OECD BTiGE key with the gold and books adaptations (owner decision, 2026-10-25):

| Good | theta before | theta now |
|---|---|---|
| 0 Agriculture, 1 Crude oil and gas, 2 Coal and other mining | 12.55 | 10.57 |
| 3 Food | 2.62 | 2.62 |
| 4 Light manufacturing | 5.88 | 10.57 |
| 5 Chemicals, refined fuels, metals | 12.55 | 10.57 |
| 6 Machinery, electronics, vehicles | 5.88 | 5.19 |

The class map is [2, 2, 2, 0, 2, 2, 1] (was [2, 2, 2, 0, 1, 2, 1]); the P01 prior is 2.62 to 10.57.
`python/tests/test_econ_pack.py::test_rust_theta_defaults_follow_the_estimator` ties the fixture export
of the defaults and `ceteris.calibrate.gravity.CLASSES` and `THETA_DEFAULT` to the estimator.

Effects:

- Re-blessed `tests/golden/p2-econ.txt` (state hashes of the 1200-tick fixture run) and the fixture
  export `tests/golden/p2-fixture` (`goods`, `params.json`, the three kernels and `provinces`, whose
  base prices and kernels depend on theta) with its GE reference, because every traded good except
  food moved.
- Stability region of the price rule (`specs/trade/SPEC.md`): kappa1 limits rise to 0.141 to 0.169 at
  theta 10.57, 0.250 to 0.307 at 5.19 (food unchanged). The default (kappa1 0.05) sits at 0.10 to 0.30
  of its limit (was 0.10 to 0.40). `price_rule_stability.rs` models the small origin at E = 11.57; three
  of its twenty probe points moved to keep the 10% margin from the boundary.
- V1 maxima: price 8.1e-6, wage 6.5e-6, flow units 1.5e-5, flow value 1.9e-5 (tolerance 1e-2). A small
  deviation decays by 0.99597 per tick frictionless and 0.99636 in the default mode. The uniqueness
  bound is 0.922 (was 0.932).

### BEHAVIOUR: P2 economic core migrated onto ADRs 0015 to 0023

Model changes (each with its ADR; the econ and trade specs carry the equations):

- 0015 Trade flows in value with a dimensionless gravity kernel: destinations spread expenditure
  over origins by Eaton-Kortum shares on K p_o^-theta, buyers pay the origin price (with tariff)
  at dispatch. Tariffs sit inside destination spending.
- 0016 Origin markets clear across ticks: orders are filled against output and merchant
  inventory; unfilled spending stays with the buyer, with no backlog. `route.rs`, `transit.rs` and
  `Kernel::access` are gone.
- 0017 Route-choice trade costs: deferred past P2 (owner, 2026-10-24). S05 least-cost legs stay
  the default and `Network.route_taus` stays `None`; the module stays in the tree, tested and off.
- 0018 and 0023 Sector trade elasticities: P01 is a theta per traded good, in classes food 2.62,
  goods 4 and 6 5.88, goods 0, 1, 2, 5 12.55 (Caliendo and Parro 2015, 99% sample column). ADR
  0023 (the classes and their values) is an agent default under 22.2, not an owner decision.
- 0019 Expected inflation in the price and wage rules (P45 gain 0.006).
- 0020 Putty-clay coefficients and a same-tick chain for local goods; rationing modes include
  S11 Mixed (Pichler and Farmer 2022, Algorithm 2).
- 0021 PIGL household demand over three categories (goods 0 and 3; 1, 2, 4, 5, 6; 7, 8, 9) with
  lognormal pop income (PIGL parameters P46, P47); the consumption rule (P37) is spend-all
  in P2 (m = 1, lambda = 0).
- 0022 Uniqueness check (AAL 2024 Theorem 1) before V1, static nesting gate, direct estimation of
  theta, border friction and mode costs (estimation lands with PLAN Task 18).

Owner decisions (2026-10-24):

- Order-up-to rule on inventory position (stock + on order) with target (1 + cover + L) months
  of use.
- Buyers use the delivered unit price q_d = 1/alpha_d in household demand, input orders and p_VA.
- Route choice deferred past P2 (ADR 0017).
- kappa1 0.05 (P03a) and gain 0.006 (P45); kappa2 0 unchanged.
- S11 Mixed rationing mode added; it fills as Priority until P4 adds essential consumption levels.

Goldens re-blessed (`CETERIS_BLESS=1`): `tests/golden/p2-econ.txt` (every state hash changes:
new state layout, sourcing, markets and pipelines) and `tests/golden/p2-fixture/` (export format
6, per-good theta, delivered-price inputs, PIGL parameters, new `pigl_check.arrow`, and the
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
