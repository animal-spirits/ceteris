# 0029. Food system: competitive storage, export restrictions and famine

Status: proposed
Date: 2026-10-24
Design sections: 7.1, 8.2, 8.5, 6.2, 13, 15.7
Phase: P2 (storage and export rules), P4 (famine mortality)

## Context

The 2007-08 and 2010-11 food price spikes and the 2022 grain shock were amplified by insulating
trade policy: Martin and Anderson (2012) attribute a large part of the 2006-08 rice price rise
to export restrictions by exporters. Speculative storage with a non-negativity constraint
produces the skewed, spiky price distributions of storable commodities (Deaton and Laroque
1992; Wright and Williams 1982), which inventory-coverage pricing alone does not. Famine is a
core long-horizon counterfactual (climate, conflict, sanctions) and has a well-understood
trigger: entitlement collapse among the poor rather than aggregate food shortage (Sen 1981).
Section 15.7 already lists competitive storage as a targeted addition; this ADR specifies it.

## Options

1. Status quo: merchant inventories with target coverage only.
2. Competitive storage for storable goods, a rule-based export restriction, and a famine hazard
   tied to consumption of the poorest income quantiles.
3. A full agricultural sector model with crop calendars and stock-to-use by crop.

## Decision

Option 2.

- Competitive storage for grains, other storable crops, crude oil and metals: speculators hold
  stocks when the expected price rise exceeds storage cost plus interest, and stocks cannot go
  negative. Storage cost and convenience yield by good are register entry P56. Stocks are an
  asset in the balance sheets of 9.1.
- Export restrictions: a state restricts exports of a food good when its domestic price rises
  above trend by more than a threshold (P57), weighted by the food share of the poorest pops
  and by the state's net-exporter status. The restriction enters the trade kernel as a wedge on
  outbound gateway links, like a sanction.
- Famine: when food consumption of a pop's bottom income quantile (from its lognormal) falls
  below subsistence for consecutive months, a mortality hazard rises on the Brass level (P58)
  and displacement flows start (6.5). Thresholds follow IPC phase definitions.
- Harvest timing: annual crops realize once a year (4.3) from P4 (ADR 0081); storage carries
  the year. P2 spreads crop output evenly across months, so its data state stays the steady
  state of ADR 0089 (owner, 2026-10-25).

## Migration

- P2: storage and export rules on the food and energy goods.
- P4: famine mortality and displacement, which need the demography module.
- Data: FAOSTAT production, trade and food balances (CC BY 4.0, to add to the manifest); AMIS
  stock-to-use; FAO food price index (already listed, fao-ffpi).

## Tests

- With storage off, results equal the status quo.
- Storage produces positive skew and price spikes in a toy single-good market, matching the
  Deaton-Laroque closed form for the storage threshold.
- A simulated 2007-08 shock with and without export rules: the rules raise the world price
  peak, with the direction and rough magnitude reported by Martin and Anderson.

## Consequences

Food and commodity price spikes become endogenous, as do policy-amplified crises. Famine
becomes a measured outcome (excess mortality, displacement) rather than an absent one.
