# 0218. Repository opening and contributions

Status: proposed (owner chose the timing, 2026-10-28)
Date: 2026-10-28
Design sections: 1, 2.5 (principle 9), 22.1 (Licence, Name); ADRs 0001, 0088, 0127, 0128, 0215, 0216
Phase: R

## Context

Ceteris is declared open source (DESIGN 1, 22.1), but the repository is private and has no
CONTRIBUTING, code of conduct, security policy, issue templates, or rule on DCO versus CLA. ADR
0088 speaks of "when the code opens" with no date. DESIGN 22.1 asks for a name check on crates.io
and GitHub that nothing records as done. From February 2027 the owner is part time, so review load
is the binding cost. Audit gap 11.

## Options

1. Public now; issues open, code contributions after 1.0.
2. Public at 1.0.
3. Public now with contributions open.

## Decision

Option 2 (owner, 2026-10-28). The repository stays private until the 1.0 release and opens with
it.

At R, before opening:
- CONTRIBUTING (how to build, test and propose; ADRs are written by the maintainer from issues),
  CODE_OF_CONDUCT (Contributor Covenant), SECURITY.md (private reporting), issue and PR templates.
- Contribution terms chosen then; the default proposal is DCO sign-off, no CLA.
- The name check of 22.1 on crates.io and GitHub, and crate names reserved.
- A history review: no secrets, no restricted data, no files that may not be redistributed.

## Migration

- DESIGN 1 and 22.1: "open source" reads as from the 1.0 release.
- ADR 0128: the annual report and the engine crate start after 1.0, as already scheduled.
- ADR 0215: the source archive's first upload is the 1.0 release.

## Tests

- A secret scan and a data-file audit of the full history pass before the repository opens.

## Consequences

No outside review load before 1.0, and no early feedback or contributors. GitHub Actions minutes
stay limited while the repository is private (they ran out on 27 October 2026), so the nightly
self-hosted runner carries more of CI until 1.0. Issue reports with repro bundles (ADR 0214) start
at 1.0.
