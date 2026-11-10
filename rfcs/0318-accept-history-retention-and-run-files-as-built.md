# RFC 0318. Accept ADR 0212 and the built parts of ADRs 0211 and 0128 as built

Status: accepted
Date: 2026-11-09
Design sections: 14.1, 14.3, 14.5, 19.1, 19.2
Release: 1.0, P3
Research: issue #202 (G3 chunk-store measurements), `specs/p3/PLAN.md` Branches A and G

## Summary

Antonio asked for one spec PR marking ADRs 0027, 0116, 0128, 0211 and 0212 accepted as built.
An audit against `main` (2026-11-09) finds that two have code and measurements behind their
decisions and three do not, so this RFC accepts two, plus the built part of a third (D3 B), and leaves
ADRs 0027 and 0116 proposed with the missing pieces named. ADRs 0001 to 0281 are a record (rfcs/README.md), so acceptance is this new RFC
and a status line, not an edit to their files.

| ADR | Decision as written | Built on main | Verdict |
|---|---|---|---|
| 0212 | Retention tiers: dense live history for 25 years, then monthly; snapshots every 12 ticks then 60; budgets 20 GB per run, 50 GB per ensemble of 32; recompute from snapshots equals dense | `ceteris-run` `retention.rs` (tiers, `recompute`), chunk store; measured 1.59 GB live run to 2150 and 13.0 GB ensemble of 32 to 2050 (issue #202, 2026-11-07); test `recomputed_series_equals_dense_series` | accept as built |
| 0211 | Scenario file as the save; run hash over (scenario, pack hash, engine); caches keyed by run hash; `DecodeError` for another engine's cache; read-only open with banner and "re-simulate" as a sibling branch; `ceteris migrate` only at a major | Scenario file, run hash (`share`), `fetch`, engine-keyed chunk store refusing another engine (RFC 0315). The banner, re-simulate branch and compatibility promise tests are app work (Branch W) and not built | accept the file family, run hash and cache rules as built; the banner and re-simulate stay proposed |
| 0116 | `bisect`, `rebase`, `cherry-pick`, `blame` | `blame` only, as leave-one-out over channels (C4). The other three verbs and their tests are not built | stay proposed |
| 0128 | Run hashes, reproducible builds, annual report, engine crate | Run hash and `fetch` recompute built (F3); reproducible cross-platform job is Branch H, unbuilt; annual report and engine crate are 1.0 and later | accept the run hash and `fetch` part (D3); the rest stays proposed |
| 0027 | Short- and long-run trade elasticities, fitted rho | Branch E is in progress; the fitted-rho placebos failed twice and are parked (#351) | stay proposed |

## Decisions

### D1. Accept ADR 0212 as built?

Options:

- A. Accept. Status line "accepted as built (RFC 0318)"; 19.2 already carries the measured disk
  row (PR #348).
- B. Wait for the quiet-host 100-year run of Branch J (J1 full pack) before accepting.

Recommendation: A, because both disk budgets were measured on the pack and hold with margin,
and the tests the ADR names pass.

Decision: A, Antonio, 2026-11-09.
Row: none new; ADR 0212 keeps its row

### D2. Accept the built part of ADR 0211?

Options:

- A. Accept the file family, run hash, cache keying and engine refusal as built; the banner,
  re-simulate branch and compatibility-promise tests remain proposed under the same ADR.
- B. Leave 0211 proposed until the app implements the banner and re-simulate.

Recommendation: A, because the data-side rules gate every other P3 branch and are tested; the
app items depend on Branch W, which waits for the app shell.

Decision: A, Antonio, 2026-11-09.
Row: none new; ADR 0211 keeps its row (banner and re-simulate stay proposed)

### D3. Accept the built part of ADR 0128, and leave ADRs 0027 and 0116 proposed?

Options:

- A. Leave all three proposed, with the unbuilt pieces listed in the table above.
- B. Accept 0128's built part (run hash, `fetch`) as 0211 does, and leave 0027, 0116 and the
  rest of 0128 proposed.

Recommendation: A, because 0128's decision is mostly reproducible builds and distribution, which
are unbuilt; its built part is already covered by 0211's run hash.

Decision: B, Antonio, 2026-11-09.
Row: none new; ADR 0128 keeps its row (reproducible builds, annual report and engine crate stay proposed)

## Spec changes

- `rfcs/README.md`: ADR 0212 shows `accepted (RFC 0318)`; ADRs 0211 and 0128 show
  `accepted in part (RFC 0318)`.
- ADR files are not rewritten.
