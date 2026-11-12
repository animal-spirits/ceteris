# RFC 0344. Game licence and CLA process

Status: accepted
Date: 2026-11-12
Design sections: 19.1, 19.3, 22.1, 23
Release: 1.0 (CLA, before the repository goes public); 2.0 for the game
Research: reports/licensing-options-2026-11-11.md (not legal advice)

## Summary

RFC 0343 put the 2.0 game in a separate repository under a non-commercial licence, named
PolyForm Noncommercial 1.0.0 as the default. The owner has since chosen a BUSL-style
source-available licence for the paid game instead (D1). This RFC records that choice, replaces the
draft CLA with separate individual and corporate terms that name the owner by GitHub account
only (D2), and fixes how a contributor signs without any change to `.github/` (D3). The BUSL
parameters that need the owner's judgment are placeholders, not guesses. A lawyer reviews the
game licence and the CLA before 1.0, as RFC 0343 requires.

## Decisions

### D1. Which source-available licence should the paid 2.0 game use?

Options:

- A. BUSL-style (Business Source License 1.1 structure): source readable and modifiable; production
  use needs the owner's licence except as the Additional Use Grant allows; converts to the
  Change License on the Change Date.
- B. PolyForm Noncommercial 1.0.0 (RFC 0343 default): all commercial use barred forever; no
  conversion.
- C. Closed game under the Steam EULA.

Recommendation: A, because it keeps the source readable, bars competing commercial copies of the
paid game while the paid release sells, and the Change Date is a fixed, public commitment.

Decision: A, Antonio, 2026-11-12 (supersedes the licence text named in ADR 0347; the repository
split, the art terms and the crate dependency direction stand).
Row: ADR 0349

The game repository's `LICENSE` fills in these parameters. Rows marked OWNER TO SET are
placeholders until the owner sets them; nothing there is a default:

| Parameter | Value |
| --- | --- |
| Licensor | OWNER TO SET (legal name or entity) |
| Licensed Work | OWNER TO SET (game name and version) |
| Additional Use Grant | Non-production use, mods, and streamed or recorded play, free of charge (D4) |
| Change Date | Four years after each release (D4) |
| Change License | OWNER TO SET (must be GPL-compatible under BUSL; for example AGPL-3.0-or-later or Apache-2.0) |

Art, music and logos stay all rights reserved and outside the BUSL grant (ADR 0347).

### D2. What CLA text should outside contributors agree to?

Options:

- A. Two forms in one `CLA.md`: individual, and corporate for a contributor whose employer holds
  rights. Both grant the owner a copyright and patent licence including the right to relicense and
  sell commercially; the contributor keeps copyright. The owner is named by GitHub account, with
  no personal email; questions go through a GitHub issue.
- B. Individual form only; contributors whose employer holds rights are refused.

Recommendation: A, because employed contributors are the likely outside contributors and an
individual-only form leaves the employer's rights unlicensed.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0350

`CLA.md` in this PR is the text of A. It stays a draft pending lawyer review.

### D3. How does a contributor sign, without changing `.github/`?

Options:

- A. Signature by pull request comment with the exact sentence in `CLA.md`; the owner checks for it
  before merging (CODEOWNERS already routes every PR to the owner). No file or workflow.
- B. A CLA-assistant GitHub Action that records signatures and blocks the PR check until signed.
  Needs a new workflow under `.github/`, which agents do not edit; the owner would add it by a PR
  from a non-`claude/*` branch.
- C. A signature log file in the repository, one line per signer.

Recommendation: A now, B when outside PRs become frequent, because A is what RFC 0343 already
describes and costs nothing. C adds a commit per signer and holds GitHub account names in history
that cannot be purged.

Decision: A, Antonio, 2026-11-12.
Row: ADR 0351

If B is chosen later, the workflow is `contributor-assistant/github-action` (pinned by commit)
with `path-to-document` set to `CLA.md`, signatures stored in a branch other than `main`, and the
signing sentence from `CLA.md` as `custom-pr-sign-comment`. That workflow is listed in the PR
description for the owner; this RFC does not add it.

### D4. Which BUSL Change Date and Additional Use Grant?

Change Date options: A. four years after each release (BUSL maximum); B. three years; C. two years.
Recommendation: A, because it protects paid sales longest.

Additional Use Grant options: A. non-production use only; B. non-production use plus mods and
streamed or recorded play, free of charge; C. no additional grant.
Recommendation: B, because mods and streaming grow the game and do not compete with sales.

Decision: Change Date A, Additional Use Grant B, Antonio, 2026-11-12. Licensor, Licensed Work and
Change License remain OWNER TO SET.
Row: ADR 0352

## Spec changes

- 19.1 licence row: the game is source-available under a BUSL-style licence (ADR 0349) in place of a
  non-commercial one (ADR 0347).
- `CLA.md`: individual and corporate terms, owner named by GitHub account, signing sentences.
- `CONTRIBUTING.md`: signing instructions for both forms.
