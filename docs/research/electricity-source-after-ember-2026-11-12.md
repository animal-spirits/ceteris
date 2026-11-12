# Electricity generation by country after Ember

Date: 2026-11-12. Issue: #505. Decision: RFC 0348.

## Question

Antonio ruled Ember out on 12 November (question card). Ember supplied generation by fuel,
capacity, demand and power-sector emissions by country, 2000 onward, for the spec 07 power mix
and the P8 energy rows. Which open source replaces it?

Constraints: the licence must allow redistribution in a release pack (or the source is
validation-only, user-fetched); coverage from 2000 for about 200 countries; a flat file the pack
build turns into Arrow IPC; consistent with ADR 0357 (EIA calibrates the energy rows, IEA held
out).

## Why Ember fails a chain-of-title check

Ember's methodology (files.ember-energy.org/public-downloads/ember_electricity_data_methodology.pdf,
read 2026-11-12) takes annual generation for most large power sectors from the Energy Institute
Statistical Review, for Europe from Eurostat, and for most other countries from EIA, with IRENA
and national statistics for gaps; IEA monthly ratios disaggregate fossil fuels. Ember's CC BY 4.0
statement covers Ember's compilation; the methodology states nothing about the upstream terms,
and the Energy Institute publishes no open licence for the Statistical Review data (about page,
read 2026-11-12: "free for users to access", no licence). The same holds for any compilation that
carries EI series (Our World in Data energy dataset).

## Candidates

| Source | Licence | Coverage | Format | Fit |
| --- | --- | --- | --- | --- |
| EIA International Energy Statistics, electricity | US public domain (data.gov: Public Domain Mark 1.0) | about 220 countries, 1980 onward, annual; net generation by fuel (nuclear, coal, gas, oil, hydro, wind, solar, geothermal, biomass and waste, tide and wave), capacity by fuel, consumption, imports, exports, losses | `INTL.zip` bulk file (JSON lines), no key; same file ADR 0357 already needs | already the calibration source (ADR 0357); no new licence, no new fetch entry |
| UNSD Energy Statistics Database | UNdata terms: "may be copied freely, duplicated and further distributed provided that UNdata is cited" | about 220 countries, 1950 onward; electricity production by plant type | UNdata exposes a subset; the full database is a separate UNSD product | terms forbid nothing commercial but say data are supplied "without any addition, subtraction, amendment, or modification", which a pack transform may breach (**unverified** reading); full download not open |
| Eurostat `nrg_bal_peh` (gross electricity production by fuel) | CC BY 4.0 (Commission Decision 2011/833/EU) | EU, EFTA, candidates, 1990 onward | TSV bulk | Europe only; a cross-check, not a replacement |
| Energy Institute Statistical Review | no open licence published | 80+ countries plus regions, 1985 onward | xlsx | redistribution not granted; ruled out for packs |
| Our World in Data energy dataset | CC BY 4.0 for OWID's own work; carries EI and Ember series | world, 1900 onward | CSV | inherits the EI and Ember problem |
| IRENA statistics | IRENA terms: reuse with attribution (**verify** at fetch) | renewables only, 2000 onward | xlsx | renewables cross-check only |

Power-sector emissions: no open country series from 2000 was found that separates IPCC 1.A.1
(energy industries) without IEA or EI inputs. PRIMAP-hist v2.6 (already pinned, CC BY 4.0) gives
1.A (fuel combustion) totals. The simulator computes emissions from input flows times emission
factors (7.7), so power-sector emissions are an output to validate against PRIMAP-hist 1.A, not a
calibration input.

## Finding

EIA International Energy Statistics covers every series Ember supplied except power-sector
emissions, at the same country and year coverage, under a licence already accepted for all packs.
Differences from Ember: EIA reports net generation where Ember used gross for Europe and EI
countries (gross is about 3 to 7 percent higher, own use of plants; **estimate**, not measured
here); EIA's latest year lags Ember's by about a year. Neither matters at T0 (2000).

Fetch: `INTL.zip` is already pinned in `fetch.toml` under the `eia` source (#537, Mac fetch of
2026-11-11); this source needs no new fetch entry.
