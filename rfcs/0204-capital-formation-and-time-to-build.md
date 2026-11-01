# 0204. Capital formation by asset class and time to build

Status: proposed
Date: 2026-10-28
Design sections: 4.4, 4.6, 7.2, 7.4, 17, 19.4; ADRs 0020, 0040, 0042, 0089, 0112, 0170, 0201
Phase: P5

## Context

DESIGN 7.4 allocates new capital by a quarterly logit and adds it the quarter it is paid for. There
are no asset classes, no build time and no half-built state, yet ADRs 0034, 0040, 0042, 0075, 0086
and 0165 all assume a construction step. Time to build is what gives investment its persistence and
makes stimulus, construction booms and the energy transition behave as observed (Kydland and
Prescott 1982; Koeva 2000 finds structures take about two years). Register P10 gives 2 to 8%
depreciation, but PWT uses about 31.5% for computers and software and about 19% for transport
equipment.

Design note: https://claude.ai/artifact/XmuftWfuBvehjp3ou7SYYd

## Options

1. Status quo: instant capital.
2. Building levels per province, as in Victoria 3.
3. Capital stocks by national-accounts asset class per enterprise cell, built through a staged
   pipeline; lumpy map-visible assets as named projects (ADR 0206).

## Decision

Option 3 (owner, 28 Oct 2026, all sub-choices below).

- **Asset classes.** Machinery and equipment, non-residential structures, ICT and software,
  dwellings (0040), public capital core and other (0042). Each has a bundle of goods per unit from
  the country's ICIO investment column by product, so the cost of capital is priced on province
  markets.
- **Orders.** The investor (7.4; owner and rule in 0201-0202) emits
  `Order { owner, province, asset, size, coefficients, schedule, soe_override }`. In 1.0 the owner
  is an enterprise cell, so every order is single-province. SOE orders flagged by 0171's lever are
  accepted even at negative expected value.
- **Coefficients lock at order** (owner). The vintage's input mix (0020) is chosen by cost
  minimization at expected prices when the order is placed, not at completion, so long projects
  can be stranded by price changes during construction.
- **Schedule.** A permit lag per country, then k_a stages with mean time T_a per asset class
  (linear chain, Erlang completion time, O(1) per stage as in 19.4):

  ```
  W[s](t+1) = W[s](t) + flow[s-1](t) - flow[s](t)
  flow[s](t) = (k_a / T_a) W[s](t) min(1, delivered_a(t) / ordered_a(t))
  K(t+1) = (1 - delta_a) K(t) + flow[k_a](t)
  ```

  The min term ties progress to rationing of the asset's bundle (0020, 0205): a project that cannot
  buy its inputs slows. More money cannot beat T_a / k_a per stage.
- **Work in progress** is a stock on the owner's balance sheet, counted in the goods invariant of
  4.6 as 0170's input stocks are. Spending on it is investment final demand. Cash calls fall due
  each quarter; an unmet call pauses the project and paused work in progress depreciates at
  delta_a; abandonment writes it off through 0112.
- **Depreciation** is geometric by asset class at PWT rates; P10's range is widened to 1 to 35%.
- **Initial state.** The 2000 pipeline is the steady state of 0089: work in progress per stage
  equals the calibrated investment flow times the stage's mean time, so the data state stays a
  fixed point.
- New section 17 entries: T_a and k_a per class (priors from Koeva 2000 and national accounts
  build-time studies), the permit lag mapping (World Bank Doing Business permit days, 2004-2019).

## Migration

- P5, in `ceteris-econ`, with finance (cash calls need credit). Pack: ICIO GFCF by product per
  country, PWT 10 capital by asset and depreciation rates (CC BY 4.0), Doing Business permit days
  (CC BY 4.0).
- Memory: two staged groups (structures, equipment) times three stages per active enterprise cell,
  about 14 MB at 300,000 cells; the update is quarterly.

## Tests

- With T_a at one quarter, k_a at one and no permit lag, results equal the instant 7.4 rule bit for
  bit.
- Goods and money invariants close every tick with work in progress.
- The steady-state 2000 pipeline holds for 100 years without shocks.
- A toy price shock after an order leaves the new vintage with the pre-shock coefficients.
- Investment-to-GDP and capital-output paths 2000-2010 within tolerance of PWT (P9).

## Consequences

Investment gains its lags, persistence and half-built state; stranded assets and unfinished
projects become possible outcomes. Adds per-class build-time parameters and widens P10.
