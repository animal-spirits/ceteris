# RFC 0357. The small pack records rest of world's member countries and GDP weights

Status: accepted
Date: 2026-11-16
Design sections: 5.1
Release: 1.0 (P4 and P5 gates)
Research: none; follows RFC 0356 and #588

## Summary

RFC 0356 gives the small pack's rest of world (ROW) GDP-weighted distances and flows. The
subset command keeps only ROW's summed GDP; the absorbed countries and their GDP are dropped.
`epsilon::pack_data` reads BACI and CEPII by ISO code from the data cache, so it cannot weight
ROW without knowing who ROW absorbed and what each weighed.

## Decisions

### D1. Where do the absorbed countries and their weights live?

Options:

- A. `pack subset` writes one more table, `row_members` (country code, GDP), and the pack
  format gains it. `epsilon::pack_data` stays a pure function of the pack plus BACI and CEPII.
  A file-format change, hence this RFC; a pack without ROW has no such table and is read as
  before.
- B. `pack_data` derives the absorbed set as every BACI/CEPII country not in the pack's codes
  and takes GDP from the cache's WDI file. No format change, but a new calibration data
  dependency, and CI needs WDI in its cache.
- C. Weight ROW by BACI trade instead of GDP. No new table or data, but deviates from RFC 0356
  D1 A's wording.

Recommendation: A, because it keeps calibration a pure function of the pack and the cache, and
CI holds the pack but not the full cache.

Decision: A, Antonio, 2026-11-16 (question card; "New row_members table + RFC").
Row: ADR 0378

## Spec changes

- 5.1: the small pack carries a `row_members` table of ROW's absorbed countries and GDP (ADR 0378).

## Work

`pack subset` writes `row_members`; `epsilon::pack_data` uses it when present and is inert
otherwise (full-pack results and goldens bit-identical); fixture regenerated; then the small-pack
epsilon fit is diagnosed (#588). The CI small-pack job needs the BACI and CEPII cache to run
epsilon on; that is a separate issue if the cache is not available there.
