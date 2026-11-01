---
paths:
  - "rfcs/**"
  - "spec/**"
  - "specs/**"
---

# Decisions and specs

- RFCs 0001 to 0281 (the former ADRs) are a record: do not rewrite them. A change to a decision,
  or acceptance of a proposed one, is a new RFC from `rfcs/0000-template.md`, following
  "Decisions, specs and research" in CLAUDE.md. Regenerate the index in `rfcs/README.md`.
- Never write a decision number into an RFC, study or memory before the PR that registers it in
  `spec/rows.toml`; take it from `cargo run -p ceteris-rows -- next N`.
- After editing `rows.toml` run `cargo run -p ceteris-rows -- render` and `-- check`.
- Specs follow `specs/TEMPLATE.md`. Every equation names its Section 17 parameter IDs; every
  invariant has a tolerance and the test that checks it.
