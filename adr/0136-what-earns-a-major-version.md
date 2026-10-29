# 0136. What earns a major version

Status: accepted; amended 2026-10-29 (minor versions; P10 order per ADR 0239)
Date: 2026-10-28
Design sections: 2.4, 21, 22.1, 23.1, 23.9; ADRs 0121, 0129 to 0134

## Context

The release roadmap (23 Oct 2026) set 1.0 to P0-P5, P8 and P9 and made P6 and P7 into 2.0.
DESIGN 21 kept the phases in numeric order before P9, so "first release at the end of P9"
read as including politics and war, while 23.9 said "2.0 politics and war". ADRs 0116 to 0135
then gave features labels from 1.3 to 3.x with no stated rule for what a major version is.
23.1 also let a mechanism that passes ablation enter the lab profile "with a minor release",
while the roadmap's bump rules make a new default that invalidates the calibration a major.
Analysis: https://claude.ai/artifact/Gw9BKMrLK9aHNzL7eZwh6j

## Options

1. No rule; label releases by feature size.
2. Majors only for public API or file-format breaks (plain semver).
3. A major relaxes one ceteris paribus: a block the previous major held fixed becomes an
   endogenous mechanism in the lab profile's default.

## Decision

Option 3 (owner, 28 Oct 2026). A release is a major only if all three hold:

1. It answers a class of question the previous major could not pose, even by god-mode edits.
2. The lab default changes, so the shipped calibration and scorecard are invalid.
3. The new mechanism has its own scorecard targets, met and published before release.

Seats and player roles, editors, modding, views, extended-profile mechanisms, new start years
in the extended profile and UI rewrites are minors. A format break without a new question gets
a `ceteris migrate` path and waits for the next major.

| Version | Starts moving | Still held fixed |
| --- | --- | --- |
| 1.0 | The economy: P0-P5, P8, P9 | Institutions as 2000 data, borders, policy from reaction functions or the tape, war |
| 2.0 | Politics and war: P6, P7, territory change (ADR 0115) | Technology as sector productivity; starts 1995-2022 |
| 3.0 | Time and technology: pre-1995 starts in the lab via the reanalysis hindcast (ADR 0131); capability milestones and emerging sectors in the lab | Leaders as stochastic attributes |
| 4.0 (candidate) | Characters and leaders in the lab, only if they pass ablation (15.7) | - |

Phase order is P0-P5, P8, P9 (1.0), then P6 (2.0-alpha), P7 (2.0-beta), P10 (1.3 Steward for
economic levers, 2.x full Steward and Tier 1b). Phase numbers are unchanged.

Promotion: a minor may add a mechanism that passed ablation to the lab profile as opt-in; the
lab default changes only at the next major.

## Migration

- DESIGN 21, 23, 23.1 and 23.9 updated to this order and rule.
- ADRs 0129 and 0130: P6 items are 2.0-alpha, P7 items 2.0-beta.
- Labels that are not majors become dependencies: 0133 variants, importers and LLM-drafted
  packs, 0134 sandboxed modules, 0130 citizen seat, 0121 thesis packs and 0124 era themes ship
  as minors once their dependencies land. Pre-1995 packs ship as extended-profile minors in
  2.x; 3.0 is their move into the lab. Nested zoom (0133) joins 3.0 if it breaks the pack
  schema.

## Consequences

The first release is smaller and comes sooner, with politics and borders held fixed and
labelled as such. Each major pairs a new kind of question with a new calibration, so a version
number tells a reader which results are comparable.

## Amendment (owner, 2026-10-29): what earns a minor version

Patches and the bump mechanics stay as in the release roadmap: identical golden hashes make a
patch, and any output change or new feature is at least a minor. A release ships as a minor only
if all three hold:

1. Same lab default and same question class. New data, start years, editors, views, extended
   mechanisms and opt-in lab mechanisms qualify. Seats and other game items follow ADRs 0239
   and 0240 and start at 2.0.
2. The scorecard does not get worse. Each minor recalibrates the same model (a calibration is
   valid only for its minor, per the roadmap) and reruns the scorecard. A loss on any scored
   target beyond the Monte Carlo error of ADR 0107 is a bug, or the change waits for the next
   major.
3. One theme a user can name. BEHAVIOUR entries say what moved and by how much, and the
   living-papers rerun (ADR 0127) lists published results that changed.

Minors are ordered by dependency and theme, not by date. An item not shipped in its planned
minor rides a later one; a 1.x item not shipped before 2.0-alpha becomes a 2.x minor. The
planned sequence from 1.1 to 5.x:
https://claude.ai/artifact/NwyGjzAgZtjXF6XfJwCm7K (proposed placement)

Phase order note: ADR 0239 replaces "P10 (1.3 Steward)" above. Steward for economic levers lands
with 2.0-alpha, and full Steward and Tier 1b ship as 2.x minors.

Proposed, not yet decided: 5.0 is cities (the province stops being the smallest unit; an urban layer at nested-zoom
grain becomes the lab default). Minds (ADR 0139) is the 6.0 candidate; its parts ship as opt-in
minors before then.
