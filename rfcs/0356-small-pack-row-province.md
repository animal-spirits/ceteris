# RFC 0356. Rest of world in the small pack is one pseudo-province with weighted distance

Status: accepted
Date: 2026-11-16
Design sections: 5.1
Release: 1.0 (P4 and P5 gates)
Research: none; follows #588 and PR #602 (RFC 0353 D1)

## Summary

RFC 0353 D1 aggregates every country outside the fixed list to one rest-of-world economy (ROW)
with no provinces. The committed pack (`tests/fixtures/pack-ci`, PR #602) does not calibrate
with current code, for two reasons found on 2026-11-16 (#588):

- With epsilon on, `epsilon::pack_data` builds BACI flows and CEPII distances per real country
  and has no row for ROW: "31 pack countries, 32 in the economy".
- With `--no-epsilon-fit`, calibration fails after 52 passes: good 1 has final demand but its
  world sales (7.40e10) do not exceed its intermediate use (7.95e10). World sales are the sum of
  province sales; ROW has no provinces, so its production is missing while the 31 kept
  countries' provinces still use it. The full pack's use to output ratio for good 1 is
  reproduced by the column rules (0.8355 against 0.8421), so no single rule is wrong and no
  rule change fixes it.

This RFC gives ROW one pseudo-province and a weighted distance row, so the small pack
calibrates through the same path as the full pack.

## Decisions

### D1. How is the small pack's rest of world modelled?

Options:

- A. ROW is one pseudo-province carrying its summed value added, population and one gateway, so
  its sales and use enter the province sums. Its distance row and BACI flows are the
  GDP-weighted aggregate of the countries it absorbs. Changes `pack subset` only; calibration
  code is unchanged.
- B. Drop ROW: the small pack holds the kept countries only. Simple, but trade balances and the
  benchmark moments no longer resemble the full pack, which weakens the D3 prediction.
- C. Park the small pack: stop #588 and the #589 follow-ups until another design exists; the
  report-only CI job keeps failing in its calibrate step.

Recommendation: A, because both failures trace to ROW lacking a province and a distance row, one
change fixes both, and the pack stays comparable to the full pack.

Decision: A, Antonio, 2026-11-16 (question card; "ROW pseudo-province, weighted distance").
Row: ADR 0377

## Spec changes

- 5.1: the small pack's rest of world is one pseudo-province with GDP-weighted distance and flows (ADR 0377).

## Work

`pack subset` code and config change, regenerated `tests/fixtures/pack-ci` (a reviewed PR under
the 25 MB cap, ADR 0370), hash test and golden updated; then the #588 campaign. Regeneration
also needs a full uncalibrated pack built with current code.
