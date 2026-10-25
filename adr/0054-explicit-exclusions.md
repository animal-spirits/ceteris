# 0054. Explicit exclusions

Status: proposed
Date: 2026-10-24
Design sections: 2.4, 15.7, 23.1
Phase: all

## Context

The gap analysis of 2026-10-24 reviewed the most complete existing models and the literature
for what is missing. It also confirmed what should stay out, because each item adds parameters
or risk faster than out-of-sample skill, breaks determinism, or rests on contested or
non-shippable evidence. Recording them prevents scope creep by future tasks and agents.

## Decision

Excluded from the lab profile, with the reason:

- Named firms, sub-province geography, daily financial markets, power-grid dispatch, per-agent
  optimization, more sectors than the input-output tables: confirmed as in 15.7. Residual load
  duration curves (0034) and cell distributions (0026) give what dispatch and named firms
  would.
- LLM agents in the economic or political loop: not validatable, prompt-sensitive, and they
  break bit-for-bit determinism. LLM text is allowed only as cached presentation (0049).
- Whole-model differentiability as an architectural requirement: forward-mode derivatives serve
  smooth blocks only (0023, 0053).
- A single headline climate damage function: replaced by lanes (0036).
- Crypto-assets as a full asset class: only as a dollarization or capital-flight channel in
  high-inflation states, if ever.
- Calibrated AI growth effects: scenario lane only (0039).
- Scripted mass climate migration: the evidence shows mostly internal, short-distance moves and
  trapped populations; migration responds through the existing discrete-choice model.
- Abstract game currencies (mana, war score) and deterministic event or focus trees: not
  empirically grounded; also excluded from the extended profile.
- Tactical battle maps: as in 2.4.
- ACLED, IHME GBD, EM-DAT, JST, GLORIA and GTD in release packs (0050); ACLED in any pack.
- Embedding AGPL code (BeforeIT.jl): run as an external benchmark only.
- Per-individual social network simulation: opinion dynamics at pop level suffice (0071).
- Pollinator, invasive species and plastic pollution modules: no validation targets at this
  scale.
- Human trafficking: unreliable data, little systemic effect at this resolution.
- A collapse switch or scripted civilizational cycles: collapse must emerge from capacity,
  fiscal stress and conflict (0067).
- Storytellers that inject events: they break validity; storytellers only select and pace what
  the UI shows (0078).
- Large hard-wired persistence elasticities from the historical-persistence literature:
  contested; initial conditions carry persistence instead.
- Strong social-media polarization effects: contradicted by experimental evidence (Allcott et
  al. 2024); effects stay small and ensembled (0071).

## Consequences

Each exclusion can be revisited only by a new ADR showing a holdout-skill gain or a core
counterfactual that needs it.
