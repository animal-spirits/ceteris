---
paths:
  - "rfcs/**"
  - "spec/**"
  - "specs/**"
  - "docs/research/**"
---

# Decisions and specs

Procedure (the repository is the single record; a decision gets its number only in the PR that
writes it into the spec):

1. Research: write the study and add a row to `docs/research/README.md` the same day.
2. Proposal: copy `rfcs/0000-template.md` to `rfcs/NNNN-slug.md` (next RFC number) and open a
   draft PR. Label the items D1, D2, and so on. RFCs 0001 to 0281 are the former ADR files, so
   RFC n decided ADR n up to 0281; the counters run apart after that.
3. Decision: put each item to Antonio as a multiple-choice question with one recommendation.
   Record his answer and the date in the RFC.
4. Rows: in the same PR, take ids from `cargo run -p ceteris-rows -- next N`, add entries to
   `spec/rows.toml`, edit only the sections they change, then run `render` and `check`.
   Fill in the research index outcome and the RFC's `Row:` lines and index row.
   Codes in spec tables (P, S, A, H, V) come from `cargo run -p ceteris-rows -- next-code P`.
5. Merge: Antonio merges spec PRs. Rejected and deferred RFCs are merged too.
6. Collision: if another PR merged the same ids first, the append conflicts or `check` fails;
   merge main and renumber.

- RFCs 0001 to 0281 are a record: do not rewrite them. A change to a decision, or acceptance of
  a proposed one, is a new RFC. Regenerate the index in `rfcs/README.md`.
- `spec/appendix-a.md` is generated from `rows.toml`.
- Specs follow `specs/TEMPLATE.md`. Every equation names its Section 17 parameter IDs; every
  invariant has a tolerance and the test that checks it. CODEOWNERS routes spec, RFC and research
  PRs to Antonio.
