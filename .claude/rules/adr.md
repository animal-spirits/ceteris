---
paths:
  - "adr/**"
  - "specs/**"
---

# ADRs and specs

- Follow `adr/TEMPLATE.md`: Status, Date, Design sections, then Context, Options, Decision,
  Consequences. New ADRs start as `proposed`; only the owner marks one accepted.
- An ADR that changes another names it in its header (`amends NNNN`, `superseded by NNNN`) and
  the older ADR gets a line pointing back.
- Take the next free number from `main` immediately before pushing; renumber on collision.
- Specs follow `specs/TEMPLATE.md`. Every equation names its Section 17 parameter IDs; every
  invariant has a tolerance and the test that checks it.
