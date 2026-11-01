# 0217. Pack profiles as prebuilt and user-built, and the credits screen

Status: proposed
Date: 2026-10-28
Design sections: 2.5 (principle 9), 16, 19.1, 22.1 (Licence); amends ADR 0050; ADRs 0005, 0088, 0113, 0213, 0215
Phase: App shell (About screen, deny exception), P3 (profiles), R (credits screen)

## Context

ADR 0050 builds the release profile on `commercial_use` and cites a Steam plan. Both were
overtaken on 28 October 2026: Ceteris is open source and non-commercial, and each user downloads
the data. The split that now matters is between artifacts that ship prebuilt (map tiles from
redistributable sources, ADR 0113) and tables each user builds. CC BY sources need credit where
their data is shown; IFPRI asks for a set sentence on adaptations; ODbL needs a notice; Slint's
royalty-free licence needs its `AboutSlint` widget in an About screen (ADR 0088). No screen holds
any of this. `deny.toml` also allows only permissive licences, so it should reject Slint's
royalty-free `LicenseRef` the day Slint enters the workspace. Audit gaps 6 and 9.

## Options

1. Keep ADR 0050's research and release profiles.
2. Redefine the profiles by where an artifact is built, and generate credits from the manifest.

## Decision

Option 2.

- **Profiles.** `prebuilt`: artifacts shipped with a release; every source must allow
  redistribution. `user`: tables built on the user's machine; every source must allow the user's
  own non-commercial use. `check-sources --profile prebuilt|user` replaces `research|release`.
  `commercial_use` stays recorded and gates nothing. Validation-only files keep ADR 0050's
  amendment.
- **Credits.** `sources.toml` gains `attribution`, the exact text a source requires (IFPRI's
  sentence, ODbL's notice). A Credits and licences screen lists every source of the user's pack
  with licence, link and attribution, generated from the manifest; the same text feeds export
  notices (ADR 0213) and the pedia.
- **About.** An About screen reachable from the top-level menu holds the version, the code licences
  (Apache-2.0 / MIT), `AboutSlint`, and a link to Credits.
- **Dependencies.** The App shell's first PR adds a `deny.toml` exception for Slint's royalty-free
  licence and records it in the third-party notices; no other copyleft is admitted.

## Migration

- ADR 0050: status "amended by 0217"; its Steam and commercial-use consequences no longer apply.
- CI: the `research` profile check becomes `user` and a `prebuilt` check is added.
- App shell: About screen and deny exception. R: Credits screen.

## Tests

- A prebuilt artifact derived from a no-redistribution source fails the `prebuilt` check.
- Every source in a built pack with a required `attribution` appears on the Credits screen.
- `cargo deny check licenses` passes with Slint present and fails on any other non-allowed licence.

## Consequences

The licence machinery matches the project as decided. Credit obligations are met mechanically.
