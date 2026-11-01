# 0187. Leadership dossiers instead of portraits

Status: accepted
Date: 2026-10-28
Design sections: 20, 23.5; ADRs 0137, 0141, 0142, 0153, 0183, 0184, 0185, 0186
Phase: P6 (2.0-alpha), with the character record

## Context

Characters (ADRs 0183-0186) need a face in the UI. The owner rejected procedural portraits in
the atlas style and in a CK2 style, and a set of alternatives (halftone press photos,
silhouettes, signatures, epithets, personal effects). He chose the intelligence dossier.
Mocks: https://claude.ai/artifact/NaioKTnQY5V1vsBWQdtvWc (portraits, rejected),
https://claude.ai/artifact/LrARgmWSbvfLtpSJsJe3f6 (alternatives),
https://claude.ai/artifact/5uPxcrucstS3fZQ7UViyDs (dossier, chosen).

## Options

1. Procedural portraits from a genome (code-drawn or painted parts).
2. AI-generated portraits with the user's own key.
3. No pictures: a text leadership profile kept by the seat's intelligence service.

## Decision

Option 3. No character has a portrait, real or generated.

- **Form.** A leadership profile in the seat's service style: classification banners, a
  portion marking on each paragraph, file number, issue number, date and reason for issue.
- **Sections.** Biographic data, career, formative experience, key judgments, recorded
  positions, associates, intelligence gaps, sources.
- **Facts** come from the character record (ADR 0183): Wikidata and WhoGov for real people,
  the pop-cell biography for generated ones (ADR 0186). Formative experience comes from the
  memory kernel and episodes (ADR 0185); associates from relations.
- **Judgments.** Each hidden trait (ADR 0184) appears only as the seat's belief (ADR 0153),
  worded by probability band: almost certainly (above 0.95), very likely (0.8-0.95), likely
  (0.55-0.8), about as likely as not (0.45-0.55), unlikely (0.2-0.45), very unlikely
  (0.05-0.2), almost no chance (below 0.05).
- **Grades.** Each judgment and position carries a source grade, reliability A-F and
  credibility 1-6, from the seat's intelligence capacity and the source type.
- **Withheld.** Content beyond the seat's collection reach is shown as redacted paragraphs
  with a count; better intelligence or an event (a defection, a leak) fills them in.
- **Reissue.** A new issue on appointment, on a major event involving the subject, or yearly.
  Changes since the last issue are marked, and earlier issues stay readable.
- **Model values.** A toggle shows the posterior and the underlying trait behind each
  judgment, for lab use and for players who want the numbers.
- **Determinism.** The dossier is a view over state and the belief layer; it changes no
  state hash. Wording comes from templates (ADR 0078), not a model.
- **Real people.** Same form, same rules: facts only from public pre-2000 data, judgments
  only from the belief layer (ADR 0183).
- Replaces the portrait section of the proposal page; the signature, epithet and effects
  mocks are not adopted.

## Tests

- No dossier view changes a state hash.
- The judgment wording for a fixed posterior is stable across runs and machines.
- A seat with no collection on a subject sees only open-source sections.

## Consequences

Characters are read rather than looked at, which fits the hidden-trait design and costs no
art. The look depends on writing quality in the templates. Cost about 2-3 part-time weeks
on top of ADRs 0183-0186.
