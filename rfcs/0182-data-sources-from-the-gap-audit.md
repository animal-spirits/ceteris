# 0182. Data sources to add from the gap audit

Status: proposed
Date: 2026-10-28
Design sections: 2.3, 15.4, 15.5; ADRs 0005, 0167-0181
Phase: per source, as listed

## Context

The gap audit found datasets that the manifest (sources.toml, fetch.toml) does not use, and
series already in the manifest that no module reads. Ceteris is non-commercial and open source,
and each user fetches their own data, so non-commercial licences are acceptable inputs. CC BY-SA
(V-Dem) and CC BY-NC-SA (EPR/GeoEPR) cannot be combined in one derived table, and the same
holds for any NC source against V-Dem.
Gap audit (2026-10-28)

## Options

1. Add sources ad hoc as modules need them.
2. Record the candidates now, with their licence state and the ADR that uses each, and gate
   each on a provider-page licence check before it enters the manifest.

## Decision

Option 2. A source enters sources.toml only after its provider page has been checked; "to
check" below means it has not been. Tables derived from NC sources are stored apart from
tables derived from V-Dem.

Prices and within-country trade:

| Source | Use | Licence |
|---|---|---|
| ICP category price levels | PIGL prices by category, 15.4 | CC BY 4.0 |
| WFP market food prices (HDX) | Province food prices | CC BY 3.0 IGO (checked) |
| EUREGIO (PBL) | Interregional IO, EU | CC BY 4.0 (checked) |
| US Commodity Flow Survey | Internal trade, US | Public domain |
| CEADs China MRIO | Internal trade, China | To check |

Trade and policy shocks:

| Source | Use | Licence |
|---|---|---|
| IMF IMTS monthly | 15.5 cases, monthly | IMF terms; fetch only |
| US Census monthly HS10 imports | 2018-19 tariffs | Public domain |
| USTR Section 301 and 232 lists; China retaliation lists; Fajgelbaum et al. files | 2018-19 tariffs | USTR public domain; files to check |
| Global Trade Alert | Policy interventions | CC BY-NC 4.0 (checked); full download needs an account |
| WTO BaTiS | Services trade, 0168 | WTO terms; no licence named |
| OxCGRT | 2020 closures, 0168 | CC BY 4.0 (checked) |
| JODI | Monthly energy | Terms not found |
| NY Fed GSCPI; IMF PortWatch | Supply-chain validation, 0170 | To check |

Health, population and migration:

| Source | Use | Licence |
|---|---|---|
| UNAIDS estimates | 0167 | To check |
| Malaria Atlas Project | 0167 | CC BY 4.0 |
| WHO TB estimates | 0167 | CC BY-NC-SA 3.0 IGO |
| Niva et al. 2023 net migration | Subnational migration | No licence stated; ask the authors |
| KNOMAD remittance matrices | 0174 | To check |

Energy, water and logistics:

| Source | Use | Licence |
|---|---|---|
| WRI Global Power Plant Database v1.3 | Grid layer | CC BY 4.0 (checked) |
| GloFAS | Runoff, 0180 | Copernicus licence |
| GRanD | Reservoirs, 0180 | CC BY 4.0, to verify |
| Weiss et al. travel time | Market access | CC BY 4.0 |
| NGA ASAM | Piracy, 0181 | Public domain |
| Global Fishing Watch | Fisheries effort | CC BY-NC 4.0 (checked) |

Firms, innovation and money:

| Source | Use | Licence |
|---|---|---|
| World Bank Enterprise Surveys | Derived parameters only | Restricted microdata; no redistribution |
| PatentsView / USPTO | Spillovers, 0179 | Public domain, to verify |
| OECD REGPAT | Spillovers, 0179 | CC BY 4.0 |
| UNESCO UIS | R&D, education | CC BY-SA 3.0 IGO |
| Holston-Laubach-Williams r* | 0176 | To check |
| Boz et al. invoicing shares | 0181 | To check |

Manifest series no module reads yet: PWT average hours (0169), WDI tourism receipts and
arrivals (0168), UNCTADstat LSCI and port throughput (0180). These need no licence work.

## Migration

- Each source is added to sources.toml and fetch.toml by the ADR that first uses it, after the
  licence check, with its licence string recorded.
- Share-alike sources (V-Dem, UNESCO UIS, WHO TB, EPR) are each kept in their own derived
  tables, and no NC source is merged with V-Dem or UIS.

## Tests

- A new manifest check (extending ADR 0005's audit) fails if a source has no recorded licence string.
- A derived-table lint fails if one table lists both a share-alike source and an NC or
  differently share-alike source.

## Consequences

Data work for ADRs 0167-0181 is planned in one place, with each licence state explicit.
Register: none.
