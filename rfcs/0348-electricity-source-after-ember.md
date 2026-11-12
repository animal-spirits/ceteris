# RFC 0348. EIA electricity series replace Ember

Status: accepted
Date: 2026-11-12
Design sections: 7.7, 15.4
Release: 1.0 (P2 data; P8 energy rows)
Research: [Electricity generation by country after Ember](../docs/research/electricity-source-after-ember-2026-11-12.md)

## Summary

Ember yearly electricity data was added as the primary source for generation by fuel (#505,
#510) and ruled out by Antonio on 2026-11-12: Ember compiles Energy Institute series, which carry
no open licence. The manifest marks `ember-yearly` rejected and drops its fetch entry. This RFC
picks the replacement for the power mix in 7.7 and the P8 energy rows. ADR 0357 already makes EIA
International Energy Statistics the energy calibration source; its electricity series (net
generation and capacity by fuel, consumption, trade, losses, about 220 countries from 1980) come
in the same bulk file, under the same public-domain terms.

## Decisions

### D1. Which source replaces Ember for electricity generation by fuel?

Options:

- A. EIA International Energy Statistics electricity series, from the `INTL.zip` bulk file ADR
  0357 already needs; Eurostat `nrg_bal_peh` (CC BY 4.0) as the European cross-check; power-sector
  emissions are a model output checked against PRIMAP-hist 1.A.
- B. UNSD Energy Statistics Database: longer history, but the full database is not an open
  download and UNdata terms bar modification of the supplied data.
- C. No external power-mix source in packs: derive the mix from the ICIO energy rows and EIA fuel
  inputs only.

Recommendation: A, because it is public domain, already accepted for all packs, needs no new
licence or fetch entry (`INTL.zip` pinned by #537), and covers every Ember series except power-sector emissions.

Decision: A, Antonio, 2026-11-12 (question card, #505).
Row: ADR 0358

## Spec changes

- 7.7: power generation by fuel and capacity are calibrated to EIA International Energy
  Statistics electricity series; Eurostat `nrg_bal_peh` is the European cross-check (ADR 0358).
