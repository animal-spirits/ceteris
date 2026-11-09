# RFC 0333. Prebuilt redistributable pack at first run (amends ADR 0215)

Status: proposed
Date: 2026-11-09
Design sections: 19 (first-run data, pack profiles)
Release: 1.0, R
Research: `reports/pre-1.0-polish-plan-2026-11-09.md` (P1, P14)

## Summary

ADR 0215 has every user fetch about 6 GB over 50 files, 13 of them manual downloads from three
providers, then build the pack on first run. For a free Steam 1.0 that is the weakest step of the
install-fork-verify loop. This RFC changes the default: the download carries a prebuilt pack built
from sources whose licence allows redistribution (the prebuilt profile of ADR 0217), and the user
fetches the remaining sources only to upgrade to the fuller pack. Everything else in ADR 0215
(archive, mirror, drift job, setup screen, schema rebuild) stands. The prebuilt pack's hash is
pinned in the release and reproducible from the same sources, so ADR 0128 is unaffected.
Estimate: 1 to 2 owner-weeks (issue #408).

## Decisions

### D1. How does first run get data to a Steam user?

Options:

- A. Fetch everything (ADR 0215 as written): no extra work, worst first impression.
- B. Prebuilt redistributable pack plus fetch for the rest: click-and-play first run, fuller pack optional.
- C. Prebuilt pack only for 1.0: no fetch step; drops non-redistributable sources from 1.0.

Recommendation: B, because it removes the 13 manual downloads from the first five minutes and keeps the fuller pack available.

Decision: B, Antonio, 2026-11-09 (decision card in the polish-plan thread).
Row: to be taken when this PR writes the spec.

## Spec changes

- 19, pack profiles: the prebuilt profile is the default install; fetch upgrades it. Row to be taken.
- ADR 0215 status line notes the amendment.

## Open points for the review

- Which sources are redistributable is settled by the licence audit (#231, #174), not here.
- Calibration targets that need non-redistributable sources: the prebuilt pack must state which
  scorecard rows it cannot reproduce.
- Release size of the prebuilt pack, to be measured on the Mac.
