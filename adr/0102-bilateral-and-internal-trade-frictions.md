# 0102. Bilateral, transit and internal trade frictions

Status: proposed
Date: 2026-10-26
Design sections: 5.4, 8.3, 8.5, 17.1 (P04, P05); trade SPEC Kernel; amends ADRs 0014, 0022, 0092

## Context

The P2 trade kernel prices distance and the border, and nothing between:

- **Internal trade is free.** Inside a province K_oo = 1 (A2). Large countries then trade
  internally more cheaply per unit of distance than across borders, and the border friction nu
  absorbs the gap, inflating the measured border effect.
- **One border friction per class.** nu is the same for every country pair (0092 fits it per
  theta class). Contiguity, common language, colonial ties and agreements, which move
  bilateral trade by 30 to 100% in gravity estimates (Head and Mayer 2014), are absent, so
  linked pairs are understated and others overstated.
- **Transit is free at borders.** A path through a third country pays transport cost but no
  border cost. Landlocked economies, whose trade costs are dominated by transit (Limao and
  Venables 2001), get coastal access too cheaply.
- **Tariffs are understated.** WDI weighted applied rates give prohibitive tariffs zero weight,
  and agreements set the tariff to zero though preference use is below 100%.
- **Transport costs are not identified.** P04 per-mode unit costs and nu trade off in the
  gravity fit, so neither is pinned down.

## Options

1. Keep the current kernel; let nu absorb everything.
2. Add the standard gravity shifters and the missing costs from open data, with literature
   coefficients.
3. Estimate pair fixed effects from BACI.

## Decision

Option 2. Option 3 overfits the 2000 data and has no counterfactual content for new pairs.

- **Internal distance.** Domestic legs within a province pay the iceberg cost of an internal
  distance d_ii = 0.67 sqrt(area / pi) (Head and Mayer 2009) at road cost, so K_oo < 1.
- **Pair shifters.** (1 + nu_AB,c) = (1 + nu_c) exp(-b' X_AB / theta_c), with X from the
  CEPII Gravity dataset (contiguity, common official language, colonial link, common legal
  origin) and b the Head and Mayer 2014 meta-analysis medians (trade elasticities, so dividing
  by theta_c turns them into cost). nu_c keeps its 0092 fit, now conditional on X.
- **Transit wedge.** Each border crossed on a path between two other countries adds a transit
  cost: the World Bank Doing Business 2006 border-compliance time for the transit country,
  priced at lambda_g per day, plus a fee share (new register entry, prior 0 to 5% ad valorem).
  The wedge sits on the gateway-layer transit hop a to c to b that ADR 0095 adds for ADR 0045:
  one hop carries both this compliance cost and 0045's evasion wedge (P83).
- **Tariffs.** Simple-average applied rates (WDI TM.TAX.MRCH.SM.AR.ZS) replace weighted ones.
  Inside an agreement, a share u (new register entry, prior 0.6 to 0.9, Keck and Lendle 2012)
  of trade gets the preference and the rest pays MFN; the routing wedge uses the mixture.
- **P04.** Sea and road unit costs are fitted to the OECD Maritime Transport Costs database by
  weight class for 2000-2005 before nu is fitted, so 0092 fits nu conditional on P04. This
  amends the owner decision of 2026-10-24 in ADR 0022 (geo SPEC P04 values until P9), with the
  owner's agreement (2026-10-26): the fit uses freight-rate data, not trade flows, so H2's
  distance range stays an unfitted gate.

## Migration

- Pack: CEPII Gravity (Etalab open licence) and Doing Business border times join the manifest;
  `geo` gains province area for d_ii.
- Kernel: domestic legs and the crossing term change; the tariff and border wedge table becomes
  pair-specific (it already is indexed by importer, exporter and good).
- Path search records the countries crossed per gateway pair leg.

## Tests

- PPML on simulated 2000 flows recovers the CEPII shifter coefficients within their bands.
- A landlocked toy country's trade cost to the coast rises by the transit wedge exactly.
- H2 distance gate still passes.

## Consequences

- Border effects fall and become pair-specific; internal trade shares of large countries fall.
- Landlocked economies (Sahel, Central Asia) become poorer and less open at t = 0 for given
  A, which the inversion absorbs, but their response to transit shocks becomes visible.
- More data in the manifest, all openly licensed.
