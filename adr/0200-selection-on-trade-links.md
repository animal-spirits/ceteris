# 0200. Selection on trade links

Status: accepted (owner, 2026-10-28; built in pre-P3 branch B with ADR 0092, ADR 0207)
Date: 2026-10-28
Design sections: 4.6, 7.2, 8.3, 15.3, 15.5; ADRs 0015, 0016, 0026, 0089, 0095; trade SPEC (buyer price row)
Amended: 2026-10-30 (normalization per cell; errata on the pow form), below
Phase: P2 fix, ahead of the P3 shock cases

## Context

Destinations spread spending over origins with Eaton-Kortum shares
pi_od = K_od c_od^-theta / Phi_d (ADR 0015), but buyers pay q_d = 1 / alpha_d with
alpha_d = sum_o pi_od / c_od, a spending-weighted harmonic mean of delivered costs (trade
SPEC, owner decision 2026-10-24). The goods bought from each origin are priced as if they were
that origin's average goods. In Eaton-Kortum a destination buys from a far origin only the goods
that origin is best at, so the price paid is the same for every origin and equals the index
P_d = Gamma Phi_d^(-1/theta). Melitz-Chaney (ADR 0026) gives the same result through firms:
only a cell's most productive firms serve distant markets.

Without selection, a rise in a border cost can lower q_d. The P2 benchmark of 2026-10-28
(https://claude.ai/artifact/RKzmS4xciyidJXTp1y1sja) found the consequences: the ACR welfare
formula (Arkolakis, Costinot and Rodriguez-Clare 2012) fails, Japan gains when its borders get
costlier, and the EU gains from Brexit. The trade SPEC rejected P_d because "a unit bought at
P_d delivers fewer than one unit"; that is the missing selection term, not a reason to drop the
index.
Deep dive: https://claude.ai/artifact/J7jLsKygA6qyLfLmvNXhNa

## Options

1. Status quo: q_d = 1 / alpha_d.
2. Buyers pay P_d and the quantity gap is absorbed as an unbooked residual.
3. A selection multiplier per link, booked as origin productivity, so buyers pay P_d and goods
   are conserved.

## Decision

Option 3. Option 2 breaks the goods invariant of 4.6.

Each active kernel leg carries

```latex
s_{od} = \frac{c_{od}}{P_d} = \Gamma^{-1} \left( \frac{K_{od}}{\pi_{od}} \right)^{1/\theta_g}
```

computed from the shares the kernel already holds. A unit shipped from o in base units arrives
in d as s_od delivered units, so alpha_d = sum_o pi_od s_od / c_od = 1 / P_d and q_d = P_d
exactly. The gain is real output, not a transfer: the goods o sells to d are made by the firms
of o that are best at them, which need 1 / s_od of the cell's average inputs per delivered
unit. The goods invariant counts selection as one explicit term beside iceberg melt.

With ADR 0026 built, the same multiplier has a firm reading. The share of cell o's firms that
serve d is n_od = (phi_min / phi*_od)^k, and the mean efficiency of those firms rises with the
cutoff, so s_od is proportional to n_od^(-1/k). Fixed export costs (P51) enter through the
cutoff.

Normalization: Gamma and the base unit are set so that s_dd = 1 at the 2000 data state. The
ADR 0089 steady state is then unchanged, and only counterfactuals move.

## Migration

- Trade SPEC: replace the buyer price row; s_od per active leg (one powf per leg per tick).
- Econ SPEC: enterprise input use per unit sold to d scales by 1 / s_od; the goods invariant
  gains the selection term.
- Calibration: check that stage 1 of the 0089 calibration reproduces the data state with
  s_dd = 1.
- Rerun the benchmark harness (branch claude/project-thread-cy0p06) once this lands.

## Tests

- Two-country toy: welfare change equals lambda_dd'^(-1/theta) to machine precision.
- A rise in tau on one link never lowers P_d in the destination.
- Benchmark: Japan no longer gains from costlier borders, and the EU no longer gains from Brexit.
- With all links at s = 1 (autarky toy), hashes equal the status quo.
- Goods invariant holds every tick with the selection term.

## Consequences

Welfare results of trade counterfactuals become comparable with the quantitative trade
literature. The 2000 calibration is unchanged. One float per active leg. No register entry.

## Amendment (2026-10-30): Gamma per cell, and errata

Owner ruling 2026-10-30: option A.

**Why.** The Normalization above asks for two things that one Gamma per good cannot give together.
On the calibrated fixture, at the calibrated prices, the Gamma that keeps today's buyer price,
q_d / P_d, varies across destinations by up to 30% within a good (good 0: 1.305 to 1.323, and
1.023 at province 24; good 3: 2.218 to 2.238, and 1.969), and s_dd = p_d / (Gamma P_d) varies by
a factor up to 4 (good 2: 0.35 to 1.43 at Gamma 1). A Gamma per cell chosen for s_dd = 1 sets the
data state's buyer price to 1 rather than 1 / alpha_d, which moves stage 3's coefficients, so it
recalibrates rather than leaves the steady state unchanged.

**Decision, as amended.** Buyers pay q_d = Gamma_d,g P_d with one constant per (province, traded
good), fixed at the calibration point so that q_d there equals the buyer price before this ADR,
the delivered unit price of the physical arrivals 1 / alpha_d. In the data state Gamma is
1 / (alpha_d P_d) at prices 1; a calibrated world reads it from the calibration tables (column
`gamma` of `econ_calib_cells.arrow`, calibration format 5), where it is q_d / P_d at the calibrated
prices. Gamma is 1 where no origin is reachable. s_dd is not 1. Away from the calibration point q_d
moves with P_d alone, so d ln q_d = d ln P_d, and the ACR welfare formula holds exactly.

The selection multiplier of a route r from o to d is

```latex
s_r = \frac{c_r}{q_d}, \qquad c_r = p_o (1 + t_r) \tau_r
```

with c_r the physical delivered cost of the route (tariff and physical loss). A shipped unit
arrives as s_r delivered units, so every route delivers X_r / q_d units: a unit of spending buys
1 / q_d units whatever its origin. The kernel cost also carries the time cost, the border
friction nu and the gateway masses, which melt nothing, so s_r is below 1 on a route whose
physical cost is below q_d, and the selection term there is negative.

**Normalization, as amended.** The 2000 calibration is unchanged in values: stage 1 (output value,
spending, r, rebates) does not depend on Gamma, since the calibration prices buyers at 1 / alpha_d
at every pass and Gamma follows. What moves is the split of arrivals over slots: by value rather
than by physical units, so the lead L_d and the steady-state pipeline move and the fixture
calibration is re-blessed (amounts in As built).

**Errata.** (1) The pow form s_od = Gamma^-1 (K_od / pi_od)^(1/theta) is not c_od / P_d: with
pi_od = K_od p_o^-theta / Phi_d it is p_o / (Gamma P_d), which leaves out the trade cost. The
identity alpha_d = sum_o pi_od s_od / c_od = 1 / q_d holds for s = c / q with c the physical
delivered cost. (2) No pow is needed, per leg or per route: the dispatch carries value by arrival
slot through the gateways and divides by q_d at the destination, and the cost per active leg is
one multiply-add for the value by lag. Migration's "one powf per leg per tick" does not apply.

**Tests, as amended.** "Two-country toy": with Gamma fixed per cell, the change in q_d equals
lambda_dd'^(-1/theta) times the change in the home price (the ACR identity), to 1e-12. The
autarky toy (no legs, every s = 1) cannot give hashes equal to the status quo, since the inputs
digest carries Gamma; it is a property instead: every tick, each buyer pays its own market's
price to 4 ulp, Gamma is 1 to 4 ulp and selection adds nothing.
"steady_state_unchanged_by_selection" checks stage 1's values at 1e-12; the moves of the lead and
the pipeline are recorded in As built.

## As built (2026-10-30, pre-P3 B2a)

- `ceteris_trade::source` takes Gamma per province and returns the buyer price
  `Sourced::buyer_price = Gamma P_d`; `dispatch` carries value by arrival slot beside the units
  that survive the melt and delivers `X / q_d` per route, with `Sourced::selected` (delivered
  units less surviving units, all routes), `Sourced::survived` (surviving units per
  destination and arrival slot, summed in the order the buyer price used before, so the base
  prices are bit for bit unchanged) and `Sourced::home` (origin units bought at home). No pow:
  one extra multiply-add per leg and slot.
- `ProvinceData::gamma` (inputs digest): the data state's `(1 / alpha_d) / P_d` at prices 1
  (`world::base_delivered_prices`), else the calibration tables' `gamma` (calibration format 5).
  Export format 13 carries it in `provinces`; the reference model reads it and prices buyers at
  Gamma P_d (`Model::new`), while the calibration keeps 1 / alpha_d at every pass
  (`Model::physical`) and writes Gamma = q_d / P_d where it settles.
- T2b books selection in the ledger (home purchases at the buyer, transit accounts brought to
  the pipeline), and `EconWorld::end_tick` checks every tick that what T2b creates less what it
  destroys is `selected - lost` (1e-9 of the good's shipped and delivered units plus the
  transit rounding).
- Production is unchanged: an origin's output leaves in shipped units at its coefficients, so
  input use per delivered unit is 1 / s_r of the cell's average, as the Decision reads it.
- Tests: `two_country_welfare_equals_acr` (1e-12 at border costs x1.1, x2 and x10),
  `raising_tau_never_lowers_the_buyer_price` (proptest, 200 cases),
  `inactive_leg_has_no_selection_term`, `goods_invariant_holds_with_selection` (120 ticks),
  `buyers_pay_the_ces_index` (1e-12), `steady_state_unchanged_by_selection` (fixture: buyer
  prices, spending, incomes, value added, orders, tariffs and arrivals at the calibrated point
  with and without selection agree to 1e-12), the reference oracle in both pricings. The
  autarky test `autarky_has_no_selection` checks that buyers pay their own
  market's price and selection adds nothing every tick; the status quo's state hash is not
  reproduced bit for bit (the inputs digest now carries Gamma, and Gamma P_d differs from
  1 / (1 / p_d) in the last bits: the states agree to 3.4e-12 at tick 120).
- Gamma P_d reproduces 1 / alpha_d to two roundings: with b = 1 / alpha_d the base price,
  Gamma = fl(b / P_d) and q_d = fl(Gamma P_d) each round by at most u = 2^-53 relative, so q_d
  is within about 2u = `f64::EPSILON` (2.2e-16) of b.
  `coefficients_are_cost_shares_at_base_prices` allows `2 * f64::EPSILON` (4.4e-16), twice the
  bound.
- Fixture calibration: stage 1's values are bit for bit unchanged; the lead moves by up to 0.165
  months and the pipeline by up to 52% in one entry (+1.4% in total units), from splitting
  arrivals over slots by value.

## As built (2026-10-30, pre-P3 B2b): benchmark

The harness of the 28 October benchmark is in the repository: `ceteris_calibrate::shock` (shock
files name countries by ISO3 code), `ceteris_calibrate::benchmark` (frictionless run from the
calibrated steady state, 1,200 ticks; welfare = d ln pop income less the Omega-weighted d ln buyer
prices; ACR = -sum over traded g of Omega_cg / theta_g d ln lambda_cg; lambda by value at origin
prices over the last 12 ticks) and `crates/ceteris-calibrate/tests/benchmark.rs` with the shock files
of `tests/data/benchmark/`. Soft Brexit is crossing tau x 1.0277 both ways between GBR and each
EU27 member; hard Brexit x 1.0831 and each side's 2000 EU MFN tariff on the other (0.0314 on group
0, 0.0214 on group 1), reported only. Each side's tariff becomes max(2000 tariff, CET): CYP, HRV,
MLT and ROU had no agreement with GBR in 2000 and already met the CET both ways, so their pairs
take the iceberg lines only (the hard-Brexit values below: GBR -0.631%, EU27 -0.107%, IRL -0.192%
with the rule; -0.630%, -0.106%, -0.193% with the tariffs stacked).

Welfare on the scratch pack `k263/pack` (pre-P3 branch at ADR 0263, calibration format 6; it
predates ADR 0256 and the unsplit provinces, so B8 reruns on the final pack), beside the
28 October values (main 6423389, `data/packs/2000` of 26 October, buyer price 1 / alpha_d, run at
the default border friction P05 nu 0.3 rather than the calibrated one, so the columns differ in nu
as well as in the buyer price and the pack):

| Case | 28 October | k263/pack |
|---|---|---|
| Border x1.1, JPN | +0.6% | -0.53% |
| Border x1.1, GBR | -4.0% | -1.20% |
| Border x1.1, EU27 (income-weighted) | | -1.92% |
| Soft Brexit, GBR | -0.95% | -0.24% |
| Soft Brexit, EU27 | +0.06% | -0.043% |
| Hard Brexit, GBR | -2.2% | -0.63% |
| Hard Brexit, EU27 | +0.47% | -0.11% |
| Hard Brexit, IRL | +0.18% | -0.19% |

`japan_loses_from_costlier_borders` and `eu_loses_from_soft_brexit` pass. The base run is not at
rest on this pack: the largest |d ln p| over the last 50 of 1,200 ticks is 0.17 (28 October:
2e-7), all of the top markets in Sudan (province 3474 and its neighbours; 588 markets above 1e-3),
the known drift that ADR 0256 removes on main. The ADR 0207 criteria are C7's; on this pack, for
the record: foreign share per theta class 1.11, 1.04, 1.13 and 1.00 times ICIO 2000's; under
x1.1, 80 ICIO countries with |ACR| above 1e-4, none of the wrong sign, median model / ACR 2.45,
correlation 0.85; under x10 the run panics in the dividend write-off (`systems.rs`, "write-off
within i64", an i64 overflow) before tick 1,200.

The ACR value of ADR 0207's criterion (Omega_cg / theta_g d ln lambda_cg summed) has no
input-output linkages; the model has Leontief intermediates (median column sum of A about 0.55
on this pack), for which the literature's prediction is d ln W_c = -Omega_c' (I - A_c')^-1
diag(1/theta) d ln lambda_c (Costinot and Rodriguez-Clare 2014, section 3.4, eq. (28) of NBER
WP 18896 under perfect competition; Caliendo and Parro 2015), A_c the country's cost shares.
`benchmark::acr_io` computes it. Under x1.1 on `k263/pack`, the 80 ICIO countries with |ACR|
above 1e-4, model / ACR:

| ACR form | median | p10 | p90 | correlation | wrong sign |
|---|---|---|---|---|---|
| plain (`acr`) | 2.453 | 1.849 | 3.025 | 0.847 | 0 |
| input-output (`acr_io`) | 1.064 | 0.941 | 1.306 | 0.865 | 0 |

The 2.45 is consistent with the input-output prediction (the B2b review's amplification
estimate on this pack, 2.34 at the median, 1.71 to 2.83 at p10 to p90). ADR 0265 makes the
input-output form ADR 0207's criterion (owner, 2026-10-30); C7 checks it at B8.
