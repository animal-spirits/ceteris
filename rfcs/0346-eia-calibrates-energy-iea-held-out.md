# RFC 0346. EIA calibrates the energy rows; IEA balances stay held out

Status: accepted
Date: 2026-11-12
Design sections: 7.7, 15.4
Release: 1.0 (P2 data)
Research: reports/data-sources-research-2026-11-11.md (#495)

## Summary

RFC 0314 lists IEA energy balances as a first-tier held-out source. IEA data is not
redistributable, so the energy rows need an open calibration source. EIA International Energy
Statistics (US public domain, EIA-collected series only) gives country coal, oil and gas
production and use from 1980. This RFC amends the energy-source choice of RFC 0314 and the
source notes of RFC 0005: EIA calibrates in all packs, IEA balances are scored only, fetched by
the user, never in the pack build or the paid build.

## Decisions

### D1. Which source calibrates the energy rows?

Options:

- A. EIA International Energy Statistics calibrates; IEA stays held out.
- B. Keep IEA as the calibration source.

Recommendation: A, because EIA is public domain and keeps IEA independent for validation.

Decision: A, Antonio, 2026-11-11 (question card, #499).
Row: ADR 0356

## Spec changes

- 7.7: country energy production and use are calibrated to EIA International Energy Statistics (ADR 0356).
