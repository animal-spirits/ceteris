# RFC 0308. Trade imbalances: balanced final demand, current-account shares and the observed path (amends ADR 0100)

Status: accepted
Date: 2026-11-03
Design sections: 9.5, 15.4, 16, 17.2; econ SPEC (Pack additions, Income, Calibration, Invariants, Parameters)
Research: none separate; the measurements are the C2 probes of 2026-11-03 quoted below (WDI
BN.CAB.XOKA.CD and NY.GDP.MKTP.CD as downloaded that day, the fixture and the installed 2000
pack).

## Summary

ADR 0100 replaces the per-good rescale r_g of ADR 0089 with exogenous transfers D_c, a country's
current-account deficit as a share of world GDP times world GDP. Building it left six choices
open. Once r_g is gone, nothing makes world spending on a good equal world sales of it, so
stage 1 needs a balancing rule (D1). The 2000 current accounts are not complete (D2), the
spending of D_c has no demander to go to before ADR 0099 (D3), "world GDP" has two candidate
sources (D4), the fixture has no current accounts and is far from balanced (D5), and the observed
path needs D2 and D4 for every year (D6). D2 also replaces ADR 0100's rule that the residual of
the shares goes to the largest economy.

## Decisions

### D1. What balances world spending and sales of each good once r_g is removed?

Found (C2, 2026-11-03): with V and VA at the data, a world good's sales and its intermediate
use are fixed, so households' traded spending is what has to move. A data-state probe on the
installed pack put the column factors that would balance it at 0.48 to 3.24 (coal and other
mining 3.24); r_g on the same pack is 0.966 to 1.019.

Options:

- A. Keep V, VA and incomes at the data; balance each country's traded final demand
  biproportionally (RAS, as ADR 0286): rows each country's traded absorption at income + D_c,
  columns each good's world sales less its intermediate use; report the column factors as the
  residual of the old r_g. Moves stage 4's spending targets, not output.
- B. Keep r_g with D_c on top: output stays rescaled per good, which is what ADR 0100 removes.
- C. Rescale traded spending by 1 / r_g and send the difference to local goods: balances each
  good, but moves the traded and local split of every country.

Recommendation: A, because output and value added stay at the data, the RAS keeps each
country's traded total, and the factors show where the data's final demand and its trade
disagree.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0318

### D2. What happens to missing current accounts and to the world discrepancy?

Found: WDI BN.CAB.XOKA.CD has 2000 values for 148 of 217 economies; the missing ones are 3.03%
of world GDP, and the reported current accounts sum to -0.55% of world GDP. The US deficit is
3.78% of its GDP in the data.

Options:

- A. ADR 0100 as written: the residual goes to the largest economy (US -1.98% of GDP).
- B. Missing current accounts at 0 and flagged; the discrepancy spread pro rata to GDP over
  every country (US -3.23%).
- C. Fill missing ones from the nearest year up to 2005, then spread the rest pro rata (US
  -3.29%).

Recommendation: B, because it leaves no country's position to a residual and needs no other
year's data.

Decision: B, Antonio, 2026-11-03. It replaces ADR 0100's residual rule.
Row: ADR 0319

### D3. Who spends D_c before the government and investment demanders of ADR 0099?

Options:

- A. Households, at their PIGL shares, until ADR 0099's three demanders are built (Task C3).
- B. Hold the transfers until ADR 0099 is built.

Recommendation: A, because the imbalance enters the model now and moves to the three
demanders with ADR 0099 without changing its source.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0320

### D4. Which world GDP divides the current accounts?

Options:

- A. The pack's own: the sum of `econ_countries.gdp` over the pack's countries.
- B. WDI's world aggregate (WLD) of NY.GDP.MKTP.CD.

Recommendation: A, because the shares then sum over the economies the model has, at the GDP it
uses.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0321

### D5. How does the fixture, which has no current accounts, meet D1?

Found: D1's RAS is infeasible on the fixture. Crude oil has no final demand there but a column
target of +1.16e8, coal and other mining has a negative column target, and the fixture's r_g
runs from 0.316 to 1.789: its value added was never balanced.

Options:

- A. Scale the fixture's traded value added VA_g by its current r_g, so that its data state is
  balanced at D_c = 0; the RAS factors are then 1 and the calibrated state is today's to
  rounding. The fixture goldens re-blessed once.
- B. Keep r_g as a fallback where the RAS is infeasible: two calibrations to maintain.
- C. Run the fixture with r_g and the pack with the RAS: two calibration modes.

Recommendation: A, because the fixture is generated data and only its balance is changed.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0299

### D6. How does the observed path (ADR 0100, 2000 to 2024) treat each year?

Options:

- A. D2 and D4 per year: missing at 0 and flagged, the residual spread pro rata to that year's
  GDP, shares over that year's world GDP, the sum of NY.GDP.MKTP.CD over the pack's countries
  that report it.
- B. Every year's current accounts over 2000 world GDP.

Recommendation: A, because a share of the year's own world GDP is what the fixed-share mode
holds constant.

Decision: A, Antonio, 2026-11-03.
Row: ADR 0300

## Spec changes

- 9.5: P2 to P4 carry the observed imbalances as exogenous transfers through one world account
  (ADRs 0100, 0299, 0300, 0318 to 0321).
- 15.4: scores of runs on the observed path are conditional forecasts (ADR 0100).
- 16: WDI BN.CAB.XOKA.CD, current accounts 2000 to 2024 (ADRs 0100, 0319, 0300).
- 17.2: S22, the transfer switch (ADR 0100).
